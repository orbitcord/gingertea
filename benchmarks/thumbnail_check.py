"""Verify decoded video/playlist thumbnails render as colored terminal cells."""

import functools
import http.server
import json
import os
import subprocess
import tempfile
import threading
from pathlib import Path

from compare import ROOT, RUST, Terminal
from terminal_cells import styles

BACKEND = r"""#!/usr/bin/env python3
import os,pathlib,sys
if '--version' in sys.argv:
    print('thumbnail-fixture')
else:
    print(pathlib.Path(os.environ['GINGERTEA_THUMB_FIXTURE']).read_text())
"""


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


def main():
    passed = []
    with tempfile.TemporaryDirectory(prefix="gingertea-thumbnail-") as tmp:
        folder = Path(tmp)
        image = folder / "thumb.jpg"
        subprocess.run(
            [
                "ffmpeg",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180",
                "-frames:v",
                "1",
                str(image),
            ],
            check=True,
        )
        config = folder / "config"
        config.mkdir()
        (config / "config.toml").write_text('theme = "gingertea"\nanimations = false\n')
        backend = folder / "yt-dlp"
        backend.write_text(BACKEND)
        backend.chmod(0o755)
        fixture = folder / "info.json"
        server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(QuietHandler, directory=tmp)
        )
        threading.Thread(target=server.serve_forever, daemon=True).start()
        url = f"http://127.0.0.1:{server.server_port}/thumb.jpg"
        bad = f"http://127.0.0.1:{server.server_port}/missing.jpg"
        env = {
            **os.environ,
            "GINGERTEA_CONFIG_DIR": str(config),
            "GINGERTEA_DATA_DIR": tmp + "/data",
            "GINGERTEA_YT_DLP": str(backend),
            "GINGERTEA_THUMB_FIXTURE": str(fixture),
            "TERM": "xterm-256color",
            "TERM_PROGRAM": "Apple_Terminal",
            "NO_COLOR": "1",
        }
        cases = {
            "Playlist thumbnails list": {
                "thumbnails": [{"url": url, "width": 320, "height": 180}],
                "entries": [{"title": "First item", "url": "https://example.test/video"}],
            },
            "Playlist first-item fallback": {
                "entries": [
                    {
                        "title": "First item",
                        "url": "https://example.test/video",
                        "thumbnails": [{"url": url}],
                    }
                ]
            },
            "Unavailable image falls back": {
                "thumbnails": [{"url": bad, "width": 320, "height": 180}],
                "thumbnail": url,
            },
            "Single video thumbnail": {"thumbnail": url},
        }
        try:
            for name, metadata in cases.items():
                fixture.write_text(
                    json.dumps(
                        {
                            "id": "fixture",
                            "title": "Thumbnail fixture",
                            "webpage_url": "https://example.test/video",
                            **metadata,
                        }
                    )
                )
                app = Terminal(RUST, env)
                try:
                    app.wait("Paste a video link")
                    app.send("\x1b[200~https://example.test/video\x1b[201~")
                    app.wait("Thumbnail fixture")
                    app.wait("▀")
                    app.settle(0.15)
                    cells = styles(app)
                    pixels = [cell for cell in cells.values() if cell[0] == "▀"]
                    assert len(pixels) == 34 * 9, (name, len(pixels))
                    assert len({cell[1] for cell in pixels}) >= 6
                    assert any(cell[1] != cell[2] for cell in pixels)
                    passed.append(name)
                finally:
                    app.close()
        finally:
            server.shutdown()
            server.server_close()
    result = {"passed": passed, "count": len(passed), "rendered_cells_per_preview": 306}
    (ROOT / "benchmarks/results/thumbnails.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
