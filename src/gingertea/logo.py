"""The gingertea header, drawn entirely in terminal cells. Each look has its own."""

from rich.color import Color
from rich.text import Text

# Five-row block letters fit the full name in 53 columns.
GLYPHS = {
    "G": (" ████", "█    ", "█ ███", "█   █", " ███ "),
    "I": ("█████", "  █  ", "  █  ", "  █  ", "█████"),
    "N": ("█   █", "██  █", "█ █ █", "█  ██", "█   █"),
    "E": ("█████", "█    ", "████ ", "█    ", "█████"),
    "R": ("████ ", "█   █", "████ ", "█  █ ", "█   █"),
    "T": ("█████", "  █  ", "  █  ", "  █  ", "  █  "),
    "A": (" ███ ", "█   █", "█████", "█   █", "█   █"),
}
WORD = "GINGERTEA"
BIG = tuple(" ".join(GLYPHS[letter][row] for letter in WORD) for row in range(5))

# Six-row letters with box-drawing edges that read as a dropped shadow, 72 columns in all.
SHADOW_GLYPHS = {
    "G": (" ██████╗ ", "██╔════╝ ", "██║  ███╗", "██║   ██║", "╚██████╔╝", " ╚═════╝ "),
    "I": ("██╗", "██║", "██║", "██║", "██║", "╚═╝"),
    "N": ("███╗   ██╗", "████╗  ██║", "██╔██╗ ██║", "██║╚██╗██║", "██║ ╚████║", "╚═╝  ╚═══╝"),
    "E": ("███████╗", "██╔════╝", "█████╗  ", "██╔══╝  ", "███████╗", "╚══════╝"),
    "R": ("██████╗ ", "██╔══██╗", "██████╔╝", "██╔══██╗", "██║  ██║", "╚═╝  ╚═╝"),
    "T": ("████████╗", "╚══██╔══╝", "   ██║   ", "   ██║   ", "   ██║   ", "   ╚═╝   "),
    "A": (" █████╗ ", "██╔══██╗", "███████║", "██╔══██║", "██║  ██║", "╚═╝  ╚═╝"),
}
SHADOW = tuple("".join(SHADOW_GLYPHS[letter][row] for letter in WORD) for row in range(6))

# A steaming cup, twelve columns by six rows to stand beside SHADOW, for the tide look.
CUP = (
    "   )  )  )  ",
    "  (  (  (   ",
    " ╭────────╮ ",
    " │▓▓▓▓▓▓▓▓├╮",
    " │▓▓▓▓▓▓▓▓├╯",
    " ╰────────╯ ",
)
CUP_STEAM_ROWS = 2


def _rgb(color: str) -> tuple[int, int, int]:
    try:
        triplet = Color.parse(color).get_truecolor()
    except Exception:
        triplet = Color.parse("#ffffff").get_truecolor()
    return triplet.red, triplet.green, triplet.blue


def blend(start: str, end: str, t: float) -> str:
    """The colour `t` of the way from `start` to `end`, as a hex string."""
    a, b = _rgb(start), _rgb(end)
    mixed = (round(x + (y - x) * t) for x, y in zip(a, b, strict=True))
    return "#{:02x}{:02x}{:02x}".format(*mixed)


def _sweep(text: Text, line: str, start: str, end: str, bold: bool = True) -> None:
    """Append `line` with its colour moving from `start` to `end` across the width."""
    last = max(len(line) - 1, 1)
    for i, char in enumerate(line):
        style = blend(start, end, i / last)
        text.append(char, f"bold {style}" if bold else style)


def _extrude(lines: tuple[str, ...]) -> tuple[str, ...]:
    """Give flat block letters a shadow one cell down and to the right, drawn as `░`."""
    height, width = len(lines) + 1, len(lines[0]) + 1

    def solid(row: int, col: int) -> bool:
        return 0 <= row < len(lines) and 0 <= col < len(lines[row]) and lines[row][col] == "█"

    return tuple(
        "".join("█" if solid(r, c) else "░" if solid(r - 1, c - 1) else " " for c in range(width))
        for r in range(height)
    )


def _carve(text: Text, lines: tuple[str, ...], primary: str, secondary: str) -> None:
    """Paint letters as lit faces over darker edges, so the word stands off the screen."""
    for row, line in enumerate(lines):
        if row:
            text.append("\n")
        _carve_line(text, line, row, len(lines), primary, secondary)


def _carve_line(text: Text, line: str, row: int, rows: int, primary: str, secondary: str) -> None:
    """One row of `_carve`. Faces sweep from `primary` to `secondary` and catch light at the
    top; anything that is not a full block is the shadow side, a deep shade of the face."""
    last_col = max(len(line) - 1, 1)
    shine = 0.45 * max(0.0, 1 - row / max(rows - 2, 1))
    for col, char in enumerate(line):
        base = blend(primary, secondary, col / last_col)
        if char == "█":
            text.append(char, f"bold {blend(base, '#ffffff', shine)}")
        elif char == " ":
            text.append(char)
        else:
            text.append(char, blend(base, "#000000", 0.55))


def _plain(text: Text, width: int, color: str) -> Text:
    text.append(WORD[: max(0, width)], f"bold {color}")
    return text


def render(width: int, look: str, primary: str, secondary: str, muted: str) -> Text:
    """Draw the header for `look` in `width` cells, falling back to plain text in tiny terminals."""
    out = Text(no_wrap=True, overflow="crop", justify="center")
    if width < len(WORD):
        return _plain(out, width, primary)
    if look == "dusk":
        return _slim(out, width, primary, secondary, muted)
    if look == "tide":
        return _cup(out, width, primary, secondary, muted)
    return _banner(out, width, primary, secondary, muted)


def _banner(out: Text, width: int, primary: str, secondary: str, muted: str) -> Text:
    """Ginger: a 3D wordmark that lights up from the top."""
    if width >= len(SHADOW[0]):
        _carve(out, SHADOW, primary, secondary)
    elif width >= len(BIG[0]) + 1:
        _carve(out, _extrude(BIG), primary, secondary)
    else:
        return _slim(out, width, primary, secondary, muted)
    return out


def _slim(out: Text, width: int, primary: str, secondary: str, muted: str) -> Text:
    """Dusk: a spaced title between two rules that fade out, small enough for short terminals."""
    title = " ".join(WORD.lower())
    side = max(0, min(14, (width - len(title) - 4) // 2))
    if side:
        _sweep(out, "━" * side, muted, primary, bold=False)
        out.append("  ")
    _sweep(out, title, primary, secondary)
    if side:
        out.append("  ")
        _sweep(out, "━" * side, secondary, muted, bold=False)
    return out


def _cup(out: Text, width: int, primary: str, secondary: str, muted: str) -> Text:
    """Tide: a steaming cup beside the 3D wordmark."""
    gap = "   "
    if width < len(CUP[0]) + len(gap) + len(SHADOW[0]):
        return _banner(out, width, primary, secondary, muted)
    for row, line in enumerate(SHADOW):
        if row:
            out.append("\n")
        cup_style = muted if row < CUP_STEAM_ROWS else primary
        out.append(CUP[row], cup_style)
        out.append(gap)
        _carve_line(out, line, row, len(SHADOW), primary, secondary)
    return out
