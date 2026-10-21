"""Tea-inspired color themes."""

from textual.theme import Theme

TEA_HOUSE = Theme(
    name="tea-house",
    primary="#5f5f00",
    secondary="#af8700",
    accent="#5f5f00",
    warning="#af8700",
    error="#af0000",
    success="#5f8700",
    foreground="#262626",
    background="#eeeeee",
    surface="#eeeeee",
    panel="#d7d7d7",
    dark=False,
)
# Tea House after dark: the same olive and amber, lifted to read on a warm charcoal.
TEA_HOUSE_DARK = Theme(
    name="tea-house-dark",
    primary="#afaf5f",
    secondary="#d7af5f",
    accent="#afaf5f",
    warning="#d7af5f",
    error="#ff8787",
    success="#87af5f",
    foreground="#e4e4d7",
    background="#1c1c17",
    surface="#1c1c17",
    panel="#3a3a2c",
    dark=True,
)
MATCHA = Theme(
    name="matcha",
    primary="#afd787",
    secondary="#d7af5f",
    accent="#87af87",
    warning="#d7af5f",
    error="#ff8787",
    success="#87d787",
    foreground="#d7dfd7",
    background="#1c2621",
    surface="#1c2621",
    panel="#344239",
    dark=True,
)
EARL_GREY = Theme(
    name="earl-grey",
    primary="#afafd7",
    secondary="#87afd7",
    accent="#afafd7",
    warning="#d7af5f",
    error="#ff8787",
    success="#87af87",
    foreground="#d7d7df",
    background="#1c1c26",
    surface="#1c1c26",
    panel="#363642",
    dark=True,
)
MASALA_CHAI = Theme(
    name="masala-chai",
    primary="#d7af5f",
    secondary="#d7875f",
    accent="#d7af87",
    warning="#ffd787",
    error="#ff8787",
    success="#87af87",
    foreground="#e4e4e4",
    background="#121212",
    surface="#121212",
    panel="#303030",
    dark=True,
)
HIBISCUS = Theme(
    name="hibiscus",
    primary="#d787af",
    secondary="#d7afaf",
    accent="#d787af",
    warning="#d7af5f",
    error="#ff8787",
    success="#87af87",
    foreground="#e4d7df",
    background="#261c26",
    surface="#261c26",
    panel="#443044",
    dark=True,
)
GINGERTEA_DARK = Theme(
    name="gingertea",
    primary="#ff8700",
    secondary="#ffaf5f",
    accent="#ffaf5f",
    warning="#ffd700",
    error="#ff5f5f",
    success="#87d75f",
    foreground="#e4e4e4",
    background="#121212",
    surface="#121212",
    panel="#303030",
    dark=True,
)
GINGERTEA_LIGHT = Theme(
    name="gingertea-light",
    primary="#d75f00",
    secondary="#af5f00",
    accent="#af5f00",
    warning="#af8700",
    error="#d70000",
    success="#5f8700",
    foreground="#1c1c1c",
    background="#eeeeee",
    surface="#eeeeee",
    panel="#d0d0d0",
    dark=False,
)
DUSK = Theme(
    name="dusk",
    primary="#af87ff",
    secondary="#ff87d7",
    accent="#d7afff",
    warning="#ffd787",
    error="#ff5f87",
    success="#87d7af",
    foreground="#e8e4f0",
    background="#16131f",
    surface="#16131f",
    panel="#352e4d",
    dark=True,
)
TIDE = Theme(
    name="tide",
    primary="#5fafff",
    secondary="#87d7ff",
    accent="#5fd7d7",
    warning="#ffd787",
    error="#ff5f87",
    success="#87d7af",
    foreground="#e1e9f2",
    background="#0e1620",
    surface="#0e1620",
    panel="#263a52",
    dark=True,
)

THEMES = (
    GINGERTEA_DARK,
    GINGERTEA_LIGHT,
    DUSK,
    TIDE,
    TEA_HOUSE,
    TEA_HOUSE_DARK,
    MATCHA,
    EARL_GREY,
    MASALA_CHAI,
    HIBISCUS,
)

# Each look pairs a header, a border style and a theme. `v` cycles through them.
LOOKS = {
    "ginger": "masala-chai",
    "dusk": "dusk",
    "tide": "tide",
}

# Textual's ANSI themes name terminal palette slots instead of colours, so the header can't
# blend them and panels lose their fill. They are left out of the theme picker.
HIDDEN_THEMES = ("ansi-dark", "ansi-light")
