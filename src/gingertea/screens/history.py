"""Past downloads: open one, show it, copy the link, download it again, or forget it."""

from __future__ import annotations

from textual import on
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Vertical
from textual.screen import ModalScreen
from textual.widgets import Input, OptionList, Static
from textual.widgets.option_list import Option

from ..engine import history
from ..engine.system import open_path, reveal, write_clipboard


class HistoryScreen(ModalScreen[str | None]):
    """Dismisses with a URL when the user wants that video again, otherwise None."""

    BINDINGS = [
        Binding("escape", "close", "Close"),
        Binding("s", "show", "Show"),
        Binding("g", "again", "Again"),
        Binding("c", "copy", "Copy"),
        Binding("x", "forget", "Forget"),
    ]

    def __init__(self) -> None:
        super().__init__()
        self._items: list[dict] = []

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog", classes="history"):
            yield Static(
                "[b]History[/b]  [dim]enter open  ·  s folder  ·  g again  ·  c copy  ·  x forget[/dim]",
                id="dialog-title",
            )
            yield Input(placeholder="Search titles and links", id="history-search")
            yield OptionList(id="history")

    def on_mount(self) -> None:
        self._show("")
        self.query_one("#history-search", Input).focus()

    @on(Input.Changed, "#history-search")
    def _search(self, event: Input.Changed) -> None:
        self._show(event.value)

    def _show(self, query: str) -> None:
        self._items = history.search(query)
        options = self.query_one("#history", OptionList)
        highlighted = options.highlighted
        options.clear_options()
        for item in self._items:
            options.add_option(
                Option(item.get("title") or item.get("url") or "Untitled", id=item["id"])
            )
        if options.option_count and highlighted is not None:
            options.highlighted = min(highlighted, options.option_count - 1)

    def _current(self) -> dict | None:
        options = self.query_one("#history", OptionList)
        if options.highlighted is None or not self._items:
            return None
        option = options.get_option_at_index(options.highlighted)
        return next((item for item in self._items if item["id"] == option.id), None)

    def action_open(self) -> None:
        item = self._current()
        if item and item.get("path"):
            open_path(item["path"])

    def action_show(self) -> None:
        item = self._current()
        if item and item.get("path"):
            reveal(item["path"])

    def action_again(self) -> None:
        item = self._current()
        if item and item.get("url"):
            self.dismiss(item["url"])

    def action_copy(self) -> None:
        item = self._current()
        if not item or not item.get("url"):
            self.app.bell()
            return
        write_clipboard(item["url"])
        self.notify("Link copied")

    def action_forget(self) -> None:
        item = self._current()
        if not item:
            self.app.bell()
            return
        history.forget(item["id"])
        self._show(self.query_one("#history-search", Input).value)

    def action_close(self) -> None:
        self.dismiss(None)

    @on(OptionList.OptionSelected, "#history")
    def _open_selected(self) -> None:
        self.action_open()
