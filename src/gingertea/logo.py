"""The gingertea text wordmark, rendered entirely in terminal cells."""

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


def render(width: int, gradient: list[str]) -> Text:
    """Use a block wordmark when it fits and a single text line in small terminals."""
    out = Text(no_wrap=True, overflow="crop")
    if width < len(BIG[0]):
        out.append(WORD[: max(0, width)], f"bold {gradient[0]}")
        return out
    for row, line in enumerate(BIG):
        if row:
            out.append("\n")
        out.append(line, f"bold {gradient[min(row, len(gradient) - 1)]}")
    return out
