"""Tick the videos in a playlist, or type a number to keep the first N."""

from __future__ import annotations

from textual import on
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Input, OptionList, Static
from textual.widgets.option_list import Option

from ..engine.probe import Entry, VideoInfo

TITLE_WIDTH = 48


def _clock(seconds: int | None) -> str:
    if not seconds:
        return ""
    minutes, secs = divmod(int(seconds), 60)
    hours, minutes = divmod(minutes, 60)
    return f"{hours}:{minutes:02}:{secs:02}" if hours else f"{minutes}:{secs:02}"


class PlaylistScreen(ModalScreen[tuple[Entry, ...] | None]):
    BINDINGS = [Binding("escape", "cancel", "Cancel"), Binding("space", "toggle", "Tick")]

    def __init__(self, info: VideoInfo) -> None:
        super().__init__()
        self.info = info
        self.picked = set(range(len(info.entries)))

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog", classes="playlist"):
            yield Static(id="dialog-title")
            with Horizontal(id="first-row"):
                yield Static("First", classes="first-label")
                yield Input(placeholder="", id="first-n")
                yield Static(id="first-note")
            yield Static(id="list-head")
            yield OptionList(id="entries")
            with Horizontal(id="dialog-actions"):
                yield Button("Download", id="go", variant="primary")
                yield Button("Cancel", id="cancel")

    def on_mount(self) -> None:
        count = len(self.info.entries)
        self.query_one("#dialog-title", Static).update(
            f"[b]Choose videos[/b]{' ' * 8}[dim]{count} videos[/dim]"
        )
        self.query_one("#first-note", Static).update(
            f"of {count}    [dim]empty keeps the ticked rows[/dim]"
        )
        self.query_one("#list-head", Static).update(
            f"[dim]{'#':<6}{'title':<{TITLE_WIDTH}}length[/dim]"
        )
        self._fill(0)
        self._label()
        self.query_one("#entries", OptionList).focus()

    def _fill(self, highlight: int | None) -> None:
        options = self.query_one("#entries", OptionList)
        options.clear_options()
        for index, entry in enumerate(self.info.entries):
            mark = "●" if index in self.picked else "○"
            title = entry.title
            if len(title) > TITLE_WIDTH:
                title = title[: TITLE_WIDTH - 1] + "…"
            length = _clock(entry.duration).rjust(7)
            style = "bold" if index in self.picked else "dim"
            options.add_option(
                Option(
                    f"[{style}]{mark}  {index + 1:02}  {title.ljust(TITLE_WIDTH)}{length}[/]",
                    id=str(index),
                )
            )
        if highlight is not None and options.option_count:
            options.highlighted = min(highlight, options.option_count - 1)

    def selection(self) -> tuple[Entry, ...]:
        raw = self.query_one("#first-n", Input).value.strip()
        if raw.isdigit() and int(raw) > 0:
            return tuple(self.info.entries[: int(raw)])
        return tuple(entry for index, entry in enumerate(self.info.entries) if index in self.picked)

    def _label(self) -> None:
        count = len(self.selection())
        word = "all" if count == len(self.info.entries) else str(count)
        self.query_one("#go", Button).label = f"Download {word}"

    @on(Input.Changed, "#first-n")
    def _typed(self) -> None:
        self._label()

    @on(Input.Submitted, "#first-n")
    def _typed_go(self) -> None:
        self.action_go()

    @on(OptionList.OptionSelected, "#entries")
    def _toggled(self, event: OptionList.OptionSelected) -> None:
        if event.option.id is not None:
            self.action_toggle(int(event.option.id))

    def action_toggle(self, index: int | None = None) -> None:
        options = self.query_one("#entries", OptionList)
        if index is None:
            index = options.highlighted
        if index is None:
            return
        if index in self.picked:
            self.picked.remove(index)
        else:
            self.picked.add(index)
        self._fill(index)
        self._label()

    @on(Button.Pressed, "#go")
    def action_go(self) -> None:
        chosen = self.selection()
        if not chosen:
            self.app.bell()
            return
        self.dismiss(chosen)

    @on(Button.Pressed, "#cancel")
    def action_cancel(self) -> None:
        self.dismiss(None)
