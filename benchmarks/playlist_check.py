"""Exercise playlist bulk controls, manual toggles and cancellation through a PTY."""

import fcntl
import json
import os
import struct
import tempfile
import termios
from pathlib import Path

from compare import ROOT, RUST, Terminal
from terminal_cells import styles
from ui_controls_check import click

BACKEND = r"""#!/usr/bin/env python3
import json,sys
if '--version' in sys.argv:
    print('playlist-fixture')
else:
    print(json.dumps({'id':'list','title':'Playlist fixture','entries':[{'id':str(i),'title':f'Tea video {i:02}'+(' — '+'a very long title '*12 if i==1 else ''),'webpage_url':f'https://example.test/{i}','channel':'Tea house','duration':60} for i in range(1,31)]}))
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
            "TERM_PROGRAM": "Apple_Terminal",
        }
        app = Terminal(RUST, env)
        try:
            app.wait("Paste a video link")
            app.send("\x1b[200~https://example.test/list\x1b[201~")
            app.wait("Playlist fixture")
            app.send("\te")
            app.wait("Pick videos")
            app.wait("n None")
            app.settle(0.15)
            rows = app.screen().splitlines()
            video_rows = [row for row in rows if "Tea video" in row]
            assert len({row.index("1:00") for row in video_rows}) == 1
            assert "30 min" in app.screen()
            assert "…" in video_rows[0]
            row_index = next(i for i, row in enumerate(rows) if "Tea video 01" in row)
            x = rows[row_index].index("Tea video 01")
            cells = styles(app)
            assert cells[row_index, x][2] == 236, "Active row should have a quiet panel fill"
            assert cells[row_index, x - 5][0] == "▎" and cells[row_index, x - 5][1] == 208
            app.send("0")
            app.wait("Download 0")
            app.send("\x13")
            app.settle(0.15)
            assert "Pick videos" in app.screen()
            app.send("a")
            app.wait("Download 30")
            app.send(" ")
            app.wait("Download 29")
            click(app, "n None")
            app.wait("Download 0")
            assert "✓" not in "\n".join(
                row for row in app.screen().splitlines() if "Tea video" in row
            )
            click(app, "a All")
            app.wait("Download 30")
            app.send("2")
            app.wait("Download 2")
            assert "2 min" in app.screen()
            app.send("i")
            app.wait("Download 28")
            click(app, "i Invert")
            app.wait("Download 2")
            app.send("n")  # First N must not override an explicit bulk action.
            app.wait("Download 0")
            app.send("a")
            app.wait("Download 30")
            app.send("n")
            app.wait("Download 0")
            app.send("\x13")
            app.settle(0.15)
            assert "Pick videos" in app.screen(), "Empty selection started a download"
            app.send("\x1b")
            app.settle(0.15)
            app.send("e")
            app.wait("Pick videos")
            app.wait("Download 30")
            app.send("n ")
            app.wait("Download 1")
            app.send("\x1b[F")
            app.settle(0.15)
            assert "Tea video 30" in app.screen(), "End did not scroll the list"
            app.send("\x1b[H")
            app.settle(0.15)
            assert "Tea video 01" in app.screen()
            (ROOT / "benchmarks/results/playlist-picker.txt").write_text(
                "\n".join(row[:120].rstrip() for row in app.screen().splitlines()[:46]) + "\n"
            )
            app.send("\t ")  # Row -> Cancel, then Space.
            app.settle(0.15)
            assert "Pick videos" not in app.screen()
            app.send("e")
            app.wait("Pick videos")
            app.wait("Download 30")
            fcntl.ioctl(app.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 60, 0, 0))
            app.settle(0.15)
            compact = "\n".join(row[:60] for row in app.screen().splitlines()[:24])
            assert all(
                label in compact
                for label in ["a All", "n None", "i Invert", "first", "Cancel", "Download 30"]
            )
            result = {
                "passed": [
                    "Mouse selects and deselects all 30 including offscreen rows",
                    "a/n keyboard bulk controls",
                    "Bulk actions clear First N override",
                    "Manual row toggle still works",
                    "Zero selection does not download",
                    "Cancel restores original selection",
                    "Invert by keyboard and mouse",
                    "First N previews count and duration immediately",
                    "Durations align on the right",
                    "Home/End scroll and compact controls stay visible",
                    "Long titles truncate before right-aligned duration",
                    "Quiet row fill with accent edge",
                    "First zero previews and downloads zero items",
                    "Tab/Space activates Cancel",
                ],
                "count": 14,
            }
            (ROOT / "benchmarks/results/playlist-controls.json").write_text(
                json.dumps(result, indent=2) + "\n"
            )
            print(json.dumps(result, indent=2))
        finally:
            app.close()


if __name__ == "__main__":
    main()
