"""The Textual application."""

from __future__ import annotations

from dataclasses import replace

from textual.app import App

from . import config
from .screens.main import MainScreen
from .theme import HIDDEN_THEMES, THEMES


class GingerTeaApp(App):
    CSS_PATH = "styles.tcss"
    TITLE = "gingertea"
    ENABLE_COMMAND_PALETTE = True

    def __init__(self, initial_url: str = "") -> None:
        super().__init__()
        self.initial_url = initial_url
        # Set when PyPI has a newer gingertea; the CLI starts the upgrade after the app exits.
        self.pending_app_update: str | None = None
        self.settings = config.load()
        self.in_foreground = True

    def on_app_focus(self) -> None:
        self.in_foreground = True
        screen = self.screen
        if isinstance(screen, MainScreen):
            screen.offer_clipboard()

    def on_app_blur(self) -> None:
        self.in_foreground = False

    def on_mount(self) -> None:
        for theme in THEMES:
            self.register_theme(theme)
        for name in HIDDEN_THEMES:
            if name in self.available_themes:
                self.unregister_theme(name)
        saved = self.settings.theme
        self.theme = saved if saved in self.available_themes else "masala-chai"
        self.theme_changed_signal.subscribe(self, self._remember_theme)

    def _remember_theme(self, theme) -> None:
        if theme.name != self.settings.theme:
            self.save_settings(replace(self.settings, theme=theme.name))

    def get_default_screen(self) -> MainScreen:
        return MainScreen(self.initial_url)

    def save_settings(self, settings: config.Settings) -> None:
        self.settings = settings
        try:
            config.save(settings)
        except OSError as exc:
            self.notify(f"Couldn't save settings: {exc}", severity="error")
