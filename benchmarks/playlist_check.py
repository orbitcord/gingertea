"""Exercise playlist bulk controls, manual toggles and cancellation through a PTY."""

import json
import os
import tempfile
from pathlib import Path

from compare import ROOT, RUST, Terminal
from ui_controls_check import click

BACKEND = r"""#!/usr/bin/env python3
import json,sys
if '--version' in sys.argv:
    print('playlist-fixture')
else:
    print(json.dumps({'id':'list','title':'Playlist fixture','entries':[{'id':str(i),'title':f'Tea video {i:02}','webpage_url':f'https://example.test/{i}','channel':'Tea house','duration':60} for i in range(1,31)]}))
"""


def main():
    with tempfile.TemporaryDirectory(prefix="gingertea-playlist-") as tmp:
        config = Path(tmp) / "config"
        config.mkdir()
        (config / "config.toml").write_text('theme = "gingertea"\nanimations = false\n')
        backend = Path(tmp) / "yt-dlp"
        backend.write_text(BACKEND)
        backend.chmod(0o755)
        env = {
            **os.environ,
            "GINGERTEA_CONFIG_DIR": str(config),
            "GINGERTEA_DATA_DIR": tmp + "/data",
            "GINGERTEA_YT_DLP": str(backend),
            "TERM": "xterm-256color",
        }
        app = Terminal(RUST, env)
        try:
            app.wait("Paste a video link")
            app.send("\x1b[200~https://example.test/list\x1b[201~")
            app.wait("Playlist fixture")
            app.send("\te")
            app.wait("Choose videos")
            app.wait("[n] Deselect all")
            app.send(" ")
            app.wait("Download 29")
            click(app, "[n] Deselect all")
            app.wait("Download 0")
            assert "●" not in "\n".join(
                row for row in app.screen().splitlines() if "Tea video" in row
            )
            click(app, "[a] Select all")
            app.wait("Download all")
            app.send("2n")  # First N must not override an explicit bulk action.
            app.wait("Download 0")
            app.send("a")
            app.wait("Download all")
            app.send("n")
            app.wait("Download 0")
            app.send("\x13")
            app.settle(0.15)
            assert "Choose videos" in app.screen(), "Empty selection started a download"
            app.send("\x1b")
            app.settle(0.15)
            app.send("e")
            app.wait("Choose videos")
            app.wait("Download all")
            app.send("n ")
            app.wait("Download 1")
            result = {
                "passed": [
                    "Mouse selects and deselects all 30 including offscreen rows",
                    "a/n keyboard bulk controls",
                    "Bulk actions clear First N override",
                    "Manual row toggle still works",
                    "Zero selection does not download",
                    "Cancel restores original selection",
                ],
                "count": 6,
            }
            (ROOT / "benchmarks/results/playlist-controls.json").write_text(
                json.dumps(result, indent=2) + "\n"
            )
            print(json.dumps(result, indent=2))
        finally:
            app.close()


if __name__ == "__main__":
    main()
