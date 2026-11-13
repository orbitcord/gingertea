"""Verify rendered colors, focus, editing and dropdowns through a real PTY."""

import json
import os
import re
import subprocess
import tempfile
from pathlib import Path

import tomllib
from compare import ROOT, RUST, Terminal
from motion_check import BACKEND
from terminal_cells import styles


def at_text(app, text):
    for y, row in enumerate(app.screen().splitlines()):
        if text in row and not (text == "Save" and "Save downloads" in row):
            return styles(app)[y, row.index(text)]
    raise AssertionError(f"Missing {text}")


def click(app, text):
    for y, row in enumerate(app.screen().splitlines()):
        if text in row:
            x = row.index(text)
            app.send(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")
            app.settle(0.15)
            return
    raise AssertionError(f"Missing {text}")


def main():
    passed = []
    with tempfile.TemporaryDirectory(prefix="gingertea-controls-") as tmp:
        folder = Path(tmp)
        config = folder / "config"
        config.mkdir()
        path = config / "config.toml"
        path.write_text('theme = "gingertea"\nanimations = false\n')
        backend = folder / "yt-dlp-stub"
        backend.write_text(BACKEND.replace("'duration':90", "'height':720,'duration':90"))
        backend.chmod(0o755)
        env = {
            **os.environ,
            "GINGERTEA_CONFIG_DIR": str(config),
            "GINGERTEA_DATA_DIR": str(folder / "data"),
            "GINGERTEA_YT_DLP": str(backend),
            "NO_COLOR": "1",
            "TERM_PROGRAM": "Apple_Terminal",
            "COLORTERM": "",
            "TERM": "xterm-256color",
        }
        app = Terminal(RUST, env)
        try:
            app.wait("Paste a video link")
            app.settle(0.15)
            assert at_text(app, "Paste a video link")[2] == 233
            sgr = re.findall(rb"\x1b\[([0-9;]*)m", app.buf)
            assert any(b"38;5;208" in code for code in sgr), "No accent color emitted"
            assert not any(b"38;2;" in code or b"48;2;" in code for code in sgr)
            app.send("\tt\x1b[B")  # Gingertea -> Gingertea light.
            app.settle(0.2)
            assert at_text(app, "Paste a video link")[2] == 255
            app.send("\r")
            app.settle(0.15)
            assert tomllib.loads(path.read_text())["theme"] == "gingertea-light"
            passed.append("Dark/light colors render under NO_COLOR=1 in Apple Terminal")

            app.send("s")
            app.wait("Save downloads to")
            app.send("\x15abcd\x1b[DX")
            app.settle(0.15)
            assert "abcXd" in app.screen(), "Middle insertion failed"
            app.send("\x15" + str(folder / "downloads"))
            app.send("\t\r")
            app.wait("Chrome")
            app.send("\x1b[B\r")
            app.settle(0.15)
            normal_save = at_text(app, "Save")
            # cookies -> parallel -> filename -> animations -> Save.
            app.send("\t\t\t\t")
            app.settle(0.15)
            focused_save = at_text(app, "Save")
            assert normal_save[1:3] != focused_save[1:3], "Save has no focus feedback"
            app.send(" ")
            app.settle(0.15)
            saved = tomllib.loads(path.read_text())
            assert saved["cookies_browser"] == "chrome"
            assert saved["download_dir"] == str(folder / "downloads")
            assert "Downloads at once" not in app.screen()
            passed.extend(
                [
                    "Caret editing inserts in the middle",
                    "Settings dropdown selects with arrows/Enter",
                    "Tab focuses Save with visible color feedback; Space saves",
                ]
            )

            app.send("s\t\t\x1b[C")  # Change parallel, then Cancel.
            app.settle(0.15)
            app.send("\t\t\t\t")
            app.settle(0.15)
            cancel = at_text(app, "Cancel")
            assert cancel[2] != focused_save[2], "Cancel focus lacks error fill"
            app.send("\r")
            app.settle(0.15)
            assert tomllib.loads(path.read_text())["parallel"] == 1
            passed.append("Focused Cancel discards edits")

            # Typed URLs debounce like the original, without needing Enter.
            app.send("\x1b")
            app.settle(0.15)
            app.send("https://example.test/tea")
            app.wait("A cup of ginger tea")
            app.send("\t")
            app.settle(0.15)
            download_style = at_text(app, "[d] Download")
            app.send("\t")
            app.settle(0.15)
            assert at_text(app, "[d] Download")[1:3] != download_style[1:3]
            assert at_text(app, "[c] Convert")[1] == 166
            app.send("\r")
            app.wait("Format")
            app.settle(0.15)
            assert "2160p" not in app.screen() and "1080p" not in app.screen()
            assert "720p" in app.screen()
            passed.append("Quality menu offers only resolutions the source has")
            assert at_text(app, "MP4")[2] == 166
            app.send("\x1b[B")
            app.settle(0.15)
            assert at_text(app, "MKV")[2] == 166
            assert at_text(app, "MP4")[2] != 166
            passed.extend(
                [
                    "Typed URL loads after debounce",
                    "Main buttons focus with Tab and activate with Enter",
                    "Format arrow selection moves the accent fill",
                ]
            )
            app.send("\x1b")
            app.settle(0.15)
            click(app, "[c] Convert")
            app.wait("Format")
            click(app, "WEBM")
            assert at_text(app, "WEBM")[2] == 166
            app.send("\t")
            app.settle(0.15)
            assert at_text(app, "WEBM")[2] != 166
            passed.append("Mouse selection and inactive selection use distinct fills")
            app.send("\x1b")
            app.settle(0.15)
            clip, picture = folder / "clip.mp4", folder / "picture.png"
            subprocess.run(
                [
                    "ffmpeg",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=red:s=64x64:d=0.2",
                    "-c:v",
                    "libx264",
                    str(clip),
                ],
                check=True,
            )
            subprocess.run(
                ["ffmpeg", "-v", "error", "-i", str(clip), "-frames:v", "1", str(picture)],
                check=True,
            )
            app.send(f"\x1b[200~{clip} {picture}\x1b[201~")
            app.wait("picture.png")
            click(app, "clip.mp4")
            assert at_text(app, "clip.mp4")[2] == 166
            app.send("\x1b[B")
            app.settle(0.15)
            assert at_text(app, "picture.png")[2] == 166
            app.send("\x1b[A")
            app.settle(0.15)
            assert at_text(app, "clip.mp4")[2] == 166
            app.send("c")
            app.wait("Format")
            app.settle(0.15)
            assert "Image" in app.screen(), "Mixed batch is missing image controls"
            app.send("\x13")
            app.wait("Queue finished")
            app.settle(0.15)
            assert len(list(folder.glob("*.mp4"))) == 2
            assert len(list(folder.glob("*.png"))) == 1
            passed.extend(
                [
                    "File selection gets a visible focused fill",
                    "Mixed batches offer all applicable kinds and convert only compatible files",
                ]
            )
        finally:
            app.close()
        app = Terminal(RUST, env)
        try:
            app.wait("Paste a video link")
            app.settle(0.15)
            assert at_text(app, "Paste a video link")[2] == 255
            passed.append("Saved theme renders after restart")
        finally:
            app.close()
    result = {"passed": passed, "count": len(passed)}
    (ROOT / "benchmarks/results/ui-controls.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
