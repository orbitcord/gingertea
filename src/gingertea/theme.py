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
    foreground="#e4dfd7",
    background="#26201c",
    surface="#26201c",
    panel="#44382c",
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

THEMES = (TEA_HOUSE, MATCHA, EARL_GREY, MASALA_CHAI, HIBISCUS, GINGERTEA_DARK, GINGERTEA_LIGHT)
