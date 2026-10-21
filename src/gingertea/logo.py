"""The gingertea wordmark and selected teacup asset, rendered in terminal cells."""

from functools import cache
from importlib.resources import files

from rich.text import Text
from textual.widget import Widget


def render(width: int, color: str) -> Text:
    return Text("gingertea"[: max(0, width)], style=f"bold {color}", no_wrap=True)


@cache
def _cup_image():
    from PIL import Image

    with files("gingertea").joinpath("assets/cup.png").open("rb") as source:
        image = Image.open(source).convert("RGBA")
    bounds = image.getbbox()
    return image.crop(bounds) if bounds else image


class TeaIcon(Widget):
    """Render the supplied raster icon with Unicode quadrant cells."""

    def render(self) -> Text:
        from PIL import Image
        from textual.color import Color

        width, height = self.size.width, self.size.height
        if width < 1 or height < 1:
            return Text("")
        # Quadrant cells keep the supplied silhouette filled at a small header size.
        image = _cup_image().resize((width * 2, height * 2), Image.Resampling.LANCZOS)
        theme = self.app.current_theme
        primary = Color.parse(theme.primary).rich_color
        secondary = Color.parse(theme.secondary or theme.primary).rich_color
        blocks = " ▘▝▀▖▌▞▛▗▚▐▜▄▙▟█"
        out = Text(no_wrap=True)
        for row in range(height):
            if row:
                out.append("\n")
            for column in range(width):
                mask, warm, count = 0, 0, 0
                for y in range(2):
                    for x in range(2):
                        r, g, _b, alpha = image.getpixel((column * 2 + x, row * 2 + y))
                        if alpha >= 75:
                            mask |= 1 << (y * 2 + x)
                            warm += r > g * 1.1
                            count += 1
                from rich.style import Style

                out.append(
                    blocks[mask],
                    Style(color=secondary if warm > count / 2 else primary),
                )
        return out
