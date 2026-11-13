"""Deterministic terminal acceptance and idle check for event-driven motion.

The external yt-dlp stub controls timing; all UI/input/process handling is real.
It writes a fixture output rather than contacting a video site.
"""

import json
import os
import tempfile
import time
from pathlib import Path

import tomllib
from compare import ROOT, RUST, Terminal, process_stats

BACKEND = r"""#!/usr/bin/env python3
import json, os, pathlib, sys, time
args=sys.argv[1:]
if '--dump-single-json' in args:
    time.sleep(.65)
    print(json.dumps({'id':'tea','title':'A cup of ginger tea','channel':'Tea house','duration':90,'webpage_url':'https://example.test/tea'}),flush=True)
else:
    folder=pathlib.Path(args[args.index('--paths')+1]);folder.mkdir(parents=True,exist_ok=True)
    print('Getting ready',flush=True);time.sleep(.3)
    print(' 25.0% 2MiB/s ETA 00:03',flush=True);time.sleep(.55)
    print(' 55.0% 2MiB/s ETA 00:01',flush=True);time.sleep(.55)
    path=folder/'A cup [tea].mp4';path.write_bytes(b'test media fixture')
    print('GINGERTEA_PATH:'+str(path),flush=True)
"""


def capture(app):
    return "\n".join(line.rstrip() for line in app.screen().splitlines()).rstrip()


def main():
    results = {}
    with tempfile.TemporaryDirectory(prefix="gingertea-motion-") as tmp:
        folder = Path(tmp)
        config = folder / "config"
        config.mkdir()
        (config / "config.toml").write_text(
            f"download_dir = {json.dumps(str(folder / 'downloads'))}\nanimations = true\n"
        )
        backend = folder / "yt-dlp-stub"
        backend.write_text(BACKEND)
        backend.chmod(0o755)
        env = {
            **os.environ,
            "GINGERTEA_CONFIG_DIR": str(config),
            "GINGERTEA_DATA_DIR": str(folder / "data"),
            "GINGERTEA_YT_DLP": str(backend),
            "TERM": "xterm-256color",
        }
        env.pop("GINGERTEA_NO_ANIMATION", None)
        app = Terminal(RUST, env)
        frames = []
        try:
            app.wait("No downloads yet.")
            early = capture(app).splitlines()[1]
            frames.append(("Startup", capture(app)))
            app.settle(0.25)
            later = capture(app).splitlines()[1]
            assert early != later, "Wordmark did not resolve between frames"
            app.settle(0.65)
            assert "gingertea" in capture(app).splitlines()[1]
            assert app.settle(0.4) == 0, "Startup did not settle to zero terminal output"
            app.send("\x1b[200~https://example.test/tea\x1b[201~")
            app.wait("Getting video details")
            assert "https://example.test/tea" in capture(app)
            before = capture(app).splitlines()[5]
            frames.append(("Pasted link", capture(app)))
            app.settle(0.18)
            after = capture(app).splitlines()[5]
            assert before != after, "Loading indicator did not advance"
            app.wait("A cup of ginger tea")
            app.settle(0.3)
            frames.append(("Details ready", capture(app)))
            app.send("d")
            app.wait("25.0%")
            before = capture(app)
            frames.append(("Downloading", before))
            app.settle(0.18)
            after = capture(app)
            assert before != after, "Active download indicator did not animate"
            assert "██▌" in after, "25% progress was not reflected in the bar"
            app.wait("Queue finished")
            frames.append(("Finished", capture(app)))
            app.settle(0.7)
            cpu0, _ = process_stats(app.pid)
            began = time.perf_counter()
            output = app.settle(3)
            cpu1, rss = process_stats(app.pid)
            elapsed = time.perf_counter() - began
            assert output == 0, "Finished queue kept rendering"
            results.update(
                idle_after_queue_bytes_per_s=output / elapsed,
                idle_after_queue_cpu_percent=(cpu1 - cpu0) / elapsed * 100,
                idle_after_queue_rss_mib=rss,
            )
            app.send("s")
            app.wait("Settings ·")
            app.send("\x1b[C\x13")  # first alphabetic setting: animations -> false, save
            app.wait("Settings saved")
            saved = tomllib.loads((config / "config.toml").read_text())
            assert saved["animations"] is False, saved
        finally:
            app.close()
        for mode in ["saved setting", "environment override"]:
            if mode == "environment override":
                (config / "config.toml").write_text("animations = true\n")
                env["GINGERTEA_NO_ANIMATION"] = "1"
            app = Terminal(RUST, env)
            try:
                app.wait("No downloads yet.")
                assert "gingertea" in capture(app).splitlines()[1]
                app.settle(0.1)  # drain the rest of the initial paint after the readiness row
                assert app.settle(0.9) == 0, f"{mode} did not suppress startup motion"
            finally:
                app.close()
        results["checks"] = [
            "character resolve",
            "paste acknowledgment",
            "metadata loading",
            "active download progress",
            "completion settles",
            "settings persistence",
            "motion disabled via settings",
            "motion disabled via environment",
        ]
        result_path = ROOT / "benchmarks/results/motion.json"
        result_path.write_text(json.dumps(results, indent=2) + "\n")
        (ROOT / "benchmarks/results/motion-frames.txt").write_text(
            "\n\n".join(name + "\n" + frame for name, frame in frames) + "\n"
        )
        print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
