"""Compare actual Python/Rust terminal cells against the original design."""

import fcntl
import json
import os
import struct
import tempfile
import termios

from compare import PYTHON, ROOT, RUST, Terminal


def capture(command, env, width=120, height=46):
    app = Terminal(command, env)
    fcntl.ioctl(app.fd, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
    app.wait("No downloads yet.")
    app.settle(1)
    return app


def screen(app, height=46):
    return [line[:120].rstrip() for line in app.screen().splitlines()[:height]]


def main():
    with tempfile.TemporaryDirectory(prefix="gingertea-design-") as tmp:
        env = {
            **os.environ,
            "HOME": tmp,
            "XDG_CONFIG_HOME": tmp + "/config",
            "XDG_DATA_HOME": tmp + "/data",
            "GINGERTEA_CONFIG_DIR": tmp + "/rust-config",
            "GINGERTEA_DATA_DIR": tmp + "/rust-data",
            "GINGERTEA_NO_ANIMATION": "1",
            "GINGERTEA_NO_UPDATE_CHECK": "1",
            "GINGERTEA_NO_AUTO_UPDATE": "1",
            "GINGERTEA_NO_FFMPEG_FETCH": "1",
            "TERM": "xterm-256color",
        }
        original = capture(PYTHON, env)
        rust = capture(RUST, env)
        try:
            before, after = screen(original), screen(rust)
            output = ROOT / "benchmarks/results"
            (output / "python-design-reference.txt").write_text("\n".join(before).rstrip() + "\n")
            (output / "rust-design-restored.txt").write_text("\n".join(after).rstrip() + "\n")
            # These rows intentionally change for the requested shared gutters,
            # padded rule labels and balanced footer grid. Preserve the rest.
            changed_rows = {8, 13, 14, 15, 17, 23, 43, 44, 45}
            preserved_rows = [i for i in range(46) if i not in changed_rows]
            for row in preserved_rows:
                assert before[row] == after[row], (
                    f"Row {row} differs:\nPython: {before[row]!r}\nRust:   {after[row]!r}"
                )

            # Mouse-click the original-style tabs and action/footer controls.
            def click(app, x, y):
                app.send(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")

            click(rust, 18, 10)
            rust.wait("Add files to convert.")
            click(rust, 5, 10)
            rust.wait("Paste a video link above.")
            rust.send("\ts")
            rust.wait("Save downloads to")
            rust.wait("Browser cookies for videos that require a login")
            assert "last_video_format" not in "\n".join(screen(rust))
            rust.send("\x1b")
            rust.settle(0.12)
            rust.send("t")
            rust.wait("Themes ·")
            rust.send("\x1b")
            rust.settle(0.12)
            rust.send("h")
            rust.wait("Search titles and links")
            rust.send("\x1b")
            rust.settle(0.12)
            rust.send("l")
            rust.wait("Search supported sites")
            rust.send("\x1b")
            rust.settle(0.12)
            results = {
                "reference": "retained Python app rendered at 120x46",
                "matched_rows": preserved_rows,
                "intentional_updates": [
                    "shared two-cell gutters",
                    "spaced rule labels",
                    "balanced aligned footer",
                ],
                "checks": [
                    "original block wordmark",
                    "original workspace spacing",
                    "rounded URL field",
                    "open section rules",
                    "empty-state wording",
                    "centered shortcut chips",
                    "clickable source tabs",
                    "user-facing settings",
                    "theme picker",
                    "modal history and sites",
                ],
            }
            (output / "design-parity.json").write_text(json.dumps(results, indent=2) + "\n")
            print(json.dumps(results, indent=2))
        finally:
            original.close()
            rust.close()
        for width, height in [(80, 24), (60, 36)]:
            app = capture(RUST, env, width, height)
            try:
                assert "URL" in app.screen()
                if height < 30:
                    assert "██████╗" not in app.screen()
            finally:
                app.close()
        print("PASS: compact terminal layouts")


if __name__ == "__main__":
    main()
