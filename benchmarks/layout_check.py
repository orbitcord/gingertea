"""Check shared gutters, responsive shortcut columns and theme color swatches."""

import fcntl
import json
import os
import struct
import tempfile
import termios
from pathlib import Path

from compare import ROOT, RUST, Terminal
from terminal_cells import styles


def main():
    results = []
    with tempfile.TemporaryDirectory(prefix="gingertea-layout-") as tmp:
        config = Path(tmp) / "config"
        config.mkdir()
        (config / "config.toml").write_text('theme = "gingertea"\nanimations = false\n')
        env = {
            **os.environ,
            "GINGERTEA_CONFIG_DIR": str(config),
            "GINGERTEA_DATA_DIR": tmp + "/data",
            "TERM": "xterm-256color",
            "TERM_PROGRAM": "Apple_Terminal",
            "NO_COLOR": "1",
        }
        for width, height in [(60, 24), (80, 24), (100, 30), (120, 46), (160, 40)]:
            app = Terminal(RUST, env)
            try:
                fcntl.ioctl(app.fd, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
                app.wait("Paste a video link")
                app.settle(0.2)
                rows = [row[:width] for row in app.screen().splitlines()[:height]]
                url = next(row for row in rows if " URL " in row)
                video = next(row for row in rows if "─ Video " in row)
                downloads = next(row for row in rows if "─ Downloads " in row)
                footer_y = max(
                    i for i, row in enumerate(rows) if row.strip() and set(row.strip()) == {"─"}
                )
                for row in [url, video, downloads, rows[footer_y]]:
                    assert len(row) - len(row.lstrip()) == 2, row
                    assert len(row) - len(row.rstrip()) == 2, row
                footer = rows[footer_y + 1 :]
                for key, label in [
                    ("d", "download"),
                    ("e", "choose"),
                    ("c", "convert"),
                    ("a", "audio"),
                    ("x", "cancel"),
                    ("p", "pause"),
                    ("h", "history"),
                    ("s", "settings"),
                    ("l", "sites"),
                    ("v", "paste"),
                    ("tab", "use keys"),
                    ("t", "themes"),
                    ("q", "quit"),
                ]:
                    assert any(f" {key}  {label}" in row for row in footer), (width, key, footer)
                cells = styles(app)
                columns = []
                counts = []
                for y in range(footer_y + 1, height):
                    starts = [
                        x
                        for x in range(width)
                        if cells.get((y, x), (None, None, None))[2] == 236
                        and (x == 0 or cells.get((y, x - 1), (None, None, None))[2] != 236)
                    ]
                    if starts:
                        columns.append(starts)
                        counts.append(len(starts))
                assert counts and max(counts) - min(counts) <= 1, (width, columns)
                assert all(row == columns[0][: len(row)] for row in columns), columns
                results.append(
                    {
                        "width": width,
                        "height": height,
                        "footer_rows": len(counts),
                        "columns": columns,
                    }
                )
                if width == 160:
                    fcntl.ioctl(app.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 60, 0, 0))
                    app.settle(0.2)
                    shrunk = [row[:60] for row in app.screen().splitlines()[:24]]
                    footer_y = max(
                        i
                        for i, row in enumerate(shrunk)
                        if row.strip() and set(row.strip()) == {"─"}
                    )
                    footer_text = "\n".join(shrunk[footer_y + 1 :])
                    for label in [
                        "download",
                        "choose",
                        "convert",
                        "audio",
                        "cancel",
                        "pause",
                        "history",
                        "settings",
                        "sites",
                        "paste",
                        "use keys",
                        "themes",
                        "quit",
                    ]:
                        assert label in footer_text, (label, footer_text)
                    results[-1]["live_shrink_to_60_columns"] = True
                if width == 120:
                    assert all(
                        columns[i]
                        and any(
                            not row.strip()
                            for row in rows[footer_y + 2 * i + 2 : footer_y + 2 * i + 3]
                        )
                        for i in range(len(columns) - 1)
                    )
                    x = columns[0][0]
                    y = footer_y + 1
                    app.send(f"\x1b[<35;{x + 6};{y + 1}M")
                    app.settle(0.15)
                    painted = styles(app)
                    assert all(painted[y, cell][2] == 208 for cell in range(x, x + 12)), (
                        "Hover must solid-fill key and label"
                    )
                    assert painted[y, columns[0][1]][2] != 208
                    app.send("\x1b[<35;1;1M")
                    app.settle(0.15)
                    assert styles(app)[y, x + 5][2] == 233, "Hover did not clear"
                    results[-1]["solid_whole_button_hover"] = True
                    app.send("\tt")
                    app.wait("Themes · live preview")
                    app.settle(0.2)
                    rows = app.screen().splitlines()
                    swatches = [(y, row.index("▪")) for y, row in enumerate(rows) if "▪" in row]
                    assert len(swatches) == 10
                    cells = styles(app)
                    assert len({cells[position][1] for position in swatches}) >= 6
                    labels = [row.split("▪", 1)[1][3:21].strip() for row in rows if "▪" in row]
                    assert labels == sorted(labels)
                    assert "tea-house-light" in labels and "tea-house" in labels
                    assert "tea-house-dark" not in labels
                    assert all(" " not in label for label in labels), labels
                    app.send("\x1b[B")  # Preview gingertea-light as well as the dark palette.
                    app.settle(0.15)
                    cells = styles(app)
                    assert all(cells[position][2] == 255 for position in swatches), (
                        f"Swatch backgrounds: {[cells[position] for position in swatches]}"
                    )
                    assert cells[swatches[3]][1] == 166, "Selected preview lost its own accent color"
                    rows = app.screen().splitlines()
                    results[-1]["light_theme_swatches_without_backing_strips"] = True
                    (ROOT / "benchmarks/results/themes-preview.txt").write_text(
                        "\n".join(row[:width].rstrip() for row in rows[:height]) + "\n"
                    )
            finally:
                app.close()
    output = {"layouts": results, "theme_swatches": 10}
    (ROOT / "benchmarks/results/layout.json").write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
