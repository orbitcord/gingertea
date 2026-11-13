"""Theme-picker regression checks through real terminal key/mouse events."""

import fcntl
import json
import os
import struct
import tempfile
import termios
from pathlib import Path

import tomllib
from compare import ROOT, RUST, Terminal


def main():
    checks = []
    for height in [46, 12]:
        with tempfile.TemporaryDirectory(prefix="gingertea-theme-") as tmp:
            config = Path(tmp) / "config"
            config.mkdir()
            path = config / "config.toml"
            path.write_text('theme = "gingertea"\nanimations = false\n')
            env = {
                **os.environ,
                "GINGERTEA_CONFIG_DIR": str(config),
                "GINGERTEA_DATA_DIR": tmp + "/data",
                "TERM": "xterm-256color",
            }
            app = Terminal(RUST, env)
            fcntl.ioctl(app.fd, termios.TIOCSWINSZ, struct.pack("HHHH", height, 120, 0, 0))
            try:
                app.wait("Paste a video link")
                app.send("\tt")
                app.wait("Themes · live preview")
                app.send("\x1b[F")
                app.settle(0.15)
                assert any("›" in row and "tide" in row for row in app.screen().splitlines()), (
                    "\n".join(row[:120].rstrip() for row in app.screen().splitlines()[:46])
                )
                assert tomllib.loads(path.read_text())["theme"] == "gingertea", (
                    "Preview persisted prematurely"
                )
                app.send("\x1b")
                app.settle(0.15)
                app.send("t")
                app.wait("Themes · live preview")
                app.settle(0.15)
                assert any(
                    "›" in row and "✓ gingertea" in row for row in app.screen().splitlines()
                ), "Esc did not restore the previous theme"
                app.send("\x1b[F ")  # End then Space saves the last theme.
                app.settle(0.15)
                assert tomllib.loads(path.read_text())["theme"] == "tide"
                app.send("t")
                app.wait("Themes · live preview")
                app.send("\x1b[H")
                app.settle(0.15)
                rows = app.screen().splitlines()
                y, row = next(
                    (i, row) for i, row in enumerate(rows) if "›" in row and "dusk" in row
                )
                x = row.index("dusk") + 2
                app.send(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")
                app.settle(0.15)
                assert tomllib.loads(path.read_text())["theme"] == "dusk", (
                    "Mouse did not save theme"
                )
                assert "Themes · live preview" not in app.screen()
                app.send("t")
                app.wait("Themes · live preview")
                app.send("\t\r")
                app.settle(0.15)
                assert tomllib.loads(path.read_text())["theme"] == "earl-grey", (
                    "Tab/Enter did not save theme"
                )
                checks.append(
                    {
                        "height": height,
                        "passed": [
                            "selected row scrolls into view",
                            "preview does not save",
                            "Esc restores",
                            "Space saves",
                            "mouse saves",
                            "Tab/Enter saves",
                        ],
                    }
                )
            finally:
                app.close()
    (ROOT / "benchmarks/results/themes.json").write_text(json.dumps(checks, indent=2) + "\n")
    print(json.dumps(checks, indent=2))


if __name__ == "__main__":
    main()
