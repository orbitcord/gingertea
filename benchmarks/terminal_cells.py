"""Reconstruct foreground/background styles from captured terminal output."""

import re


def styles(app):
    """Reconstruct cell styles from real cursor-addressed SGR output."""
    cells = {}
    row = col = 0
    fg = bg = None
    bold = False
    for token in re.split(r"(\x1b\[[0-?]*[ -/]*[@-~])", app.buf.decode(errors="replace")):
        if token.startswith("\x1b["):
            move = re.fullmatch(r"\x1b\[(\d*);?(\d*)[Hf]", token)
            if move:
                row, col = int(move[1] or 1) - 1, int(move[2] or 1) - 1
            elif token in ("\x1b[2J", "\x1b[3J"):
                cells = {}
            elif re.fullmatch(r"\x1b\[[0-9;]*m", token):
                args = [int(v or 0) for v in token[2:-1].split(";")]
                i = 0
                while i < len(args):
                    n = args[i]
                    if n == 0:
                        fg = bg = None
                        bold = False
                    elif n == 1:
                        bold = True
                    elif n == 22:
                        bold = False
                    elif n in (38, 48) and i + 2 < len(args) and args[i + 1] == 5:
                        if n == 38:
                            fg = args[i + 2]
                        else:
                            bg = args[i + 2]
                        i += 2
                    elif n == 39:
                        fg = None
                    elif n == 49:
                        bg = None
                    i += 1
            continue
        for char in token:
            if char == "\r":
                col = 0
            elif char == "\n":
                row += 1
            elif char >= " ":
                cells[row, col] = (char, fg, bg, bold)
                col += 1
    return cells
