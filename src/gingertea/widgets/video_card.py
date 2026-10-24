"""The card that shows the looked-up video and its Download / Convert buttons."""

from __future__ import annotations

from textual.app import ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import Button, ContentSwitcher, LoadingIndicator, Static

from ..engine import errors
from ..engine.names import playlist_folder
from ..engine.probe import VideoInfo
from .thumbnail import Thumbnail

EMPTY_HINT = (
    "[b]Paste a video link above.[/b]\n\n"
    "[dim]YouTube, Vimeo, SoundCloud and 1,300+ sites. Press Enter to load a typed link.[/dim]"
)
LEADER = 10  # metadata values line up this many columns after the key


class VideoCard(Vertical):
    _info: VideoInfo | None = None

    def compose(self) -> ComposeResult:
        with ContentSwitcher(initial="card-empty", id="card-switcher"):
            yield Static(EMPTY_HINT, id="card-empty")
            with Horizontal(id="card-loading"):
                yield LoadingIndicator()
                yield Static("Looking up video…", id="loading-text")
            with Vertical(id="card-error"):
                yield Static(id="error-message")
                yield Static(id="error-hint")
                with Horizontal(id="error-actions"):
                    yield Button("Use browser cookies", id="use-cookies", variant="primary")
                    yield Button("Update yt-dlp", id="update-ytdlp", variant="primary")
                    yield Button("Try again", id="retry")
            with Horizontal(id="card-info"):
                yield Thumbnail(id="thumb")
                with Vertical(id="details"):
                    yield Static(id="video-title")
                    yield Static(id="video-meta")
                    with Horizontal(id="actions"):
                        yield Button("\\[d] Download", id="download")
                        yield Button("\\[e] Choose", id="choose")
                        yield Button("\\[c] Convert", id="convert")
                        yield Static("MP4 by default", id="action-hint")

    def on_mount(self) -> None:
        self.border_title = "Video"
        self.query_one("#choose").display = False

    def _show(self, name: str) -> None:
        self.query_one(ContentSwitcher).current = name

    def show_empty(self) -> None:
        self._show("card-empty")

    def show_loading(self, url: str) -> None:
        self.query_one("#loading-text", Static).update(f"Looking up [b]{_escape(url)}[/b]…")
        self._show("card-loading")

    def show_error(self, err: errors.FriendlyError) -> None:
        self.query_one("#error-message", Static).update(f"[b $error]✗ {_escape(err.message)}[/]")
        hint = _escape(err.hint)
        if err.detail and err.detail != err.message:
            hint += f"\n\n[dim]yt-dlp: {_escape(err.detail)}[/dim]"
        self.query_one("#error-hint", Static).update(hint)
        self.query_one("#use-cookies").display = err.kind == errors.LOGIN
        self.query_one("#update-ytdlp").display = err.kind == errors.OUTDATED
        self.query_one("#retry").display = err.kind in (
            errors.NETWORK,
            errors.OUTDATED,
            errors.GENERIC,
            errors.LOGIN,
        )
        self._show("card-error")

    def show_info(self, info: VideoInfo) -> None:
        self._info = info
        self.border_title = "Playlist" if info.is_playlist else "Video"
        self.query_one("#thumb", Thumbnail).image = None
        self.query_one("#video-title", Static).update(f"[b]{_escape(info.title)}[/b]")
        self.query_one("#choose").display = info.is_playlist
        self.query_one("#convert").display = not info.is_playlist
        download = "\\[d] Download all" if info.is_playlist else "\\[d] Download"
        self.query_one("#download", Button).label = download
        if info.is_playlist:
            folder = playlist_folder(info.title, info.channel)
            self.query_one("#action-hint", Static).update(f"MP4  ·  {_escape(folder)}/")
        else:
            self.query_one("#action-hint", Static).update("MP4 by default")
        self._render_meta()
        self._show("card-info")

    def on_resize(self) -> None:
        if self._info is not None:
            self._render_meta()

    def _render_meta(self) -> None:
        info = self._info
        if info is None:
            return
        if self.screen.size.height < 30:
            if info.is_playlist:
                bits = [info.channel, f"{len(info.entries)} videos", info.extractor]
            else:
                bits = [info.channel, info.duration_text, info.extractor]
            quality = (
                "" if info.is_playlist else (f"Up to {info.max_height}p" if info.max_height else "")
            )
            text = " · ".join(_escape(b) for b in bits if b)
            extra = [b for b in (() if info.is_playlist else (info.date_text, quality)) if b]
            if extra:
                text += "\n" + " · ".join(_escape(b) for b in extra)
            self.query_one("#video-meta", Static).update(text)
            return
        if info.is_playlist:
            rows = [
                ("channel", info.channel),
                ("videos", str(len(info.entries))),
                ("folder", playlist_folder(info.title, info.channel) + "/"),
            ]
        else:
            rows = [
                ("channel", info.channel),
                ("length", info.duration_text),
                ("uploaded", info.date_text),
            ]
            if info.max_height:
                rows.append(("quality", f"up to {info.max_height}p"))
        if info.extractor:
            rows.append(("site", info.extractor))
        self.query_one("#video-meta", Static).update(
            "\n".join(
                f"[dim]{key}[/dim] [$foreground-disabled]{'·' * (LEADER - len(key))}[/] {_escape(value)}"
                for key, value in rows
            )
        )

    def set_thumbnail(self, image) -> None:
        self.query_one("#thumb", Thumbnail).image = image


def _escape(text: str) -> str:
    from rich.markup import escape

    return escape(text or "")
