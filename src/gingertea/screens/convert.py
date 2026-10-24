"""Pick a format and quality, then download (a link) or convert (files on disk)."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, replace

from rich.markup import escape
from textual import events, on
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Grid, Horizontal, Vertical
from textual.message import Message
from textual.screen import ModalScreen
from textual.widget import Widget
from textual.widgets import Button, Input, Label, OptionList, Static, TabbedContent, TabPane
from textual.widgets.option_list import Option

from ..config import Settings
from ..engine.names import playlist_folder
from ..engine.options import (
    AUDIO_FORMATS,
    FRAME_RATE_LABELS,
    H264_FIRST,
    VIDEO_FORMATS,
    Choice,
    audio_qualities,
    parse_langs,
    video_qualities,
)
from ..engine.probe import VideoInfo
from ..engine.times import clip_error, format_seconds, parse_time

TAB_NAMES = {"video": "Video", "audio": "Audio", "image": "Image", "frames": "Frames"}

FORMAT_NOTES = {
    "mp4": "widely supported",
    "mkv": "keeps every stream",
    "webm": "open web format",
    "mov": "Apple / editing apps",
    "gif": "short clips, no sound",
    "mp3": "widely supported",
    "m4a": "AAC, Apple devices",
    "opus": "compressed audio",
    "flac": "lossless",
    "wav": "uncompressed",
    "ogg": "Vorbis",
}


def quality_label(kind: str, q: str, fmt: str = "", local: bool = False) -> str:
    if kind == "frames":
        return FRAME_RATE_LABELS.get(q, q).capitalize()
    if kind == "image":
        if q == "best":
            return "Original size"
        return "Half size" if q == "50" else f"Up to {q}px"
    if q == "best" and local:
        return "Same as original" if kind == "video" else "Best"
    if q == "best":
        # See options.H264_FIRST: "best" MP4/MOV is the best H.264, so nothing is re-encoded.
        return "Best H.264 copy" if fmt in H264_FIRST else "Best available"
    label = f"{q}p" if kind == "video" else f"{q} kbps"
    if fmt == "mp4" and kind == "video" and int(q) > 1080 and not local:
        label += "  [dim]VP9/AV1, newer players[/dim]"
    return label


def format_note(kind: str, fmt: str) -> str:
    return FORMAT_NOTES.get(fmt, "")


def format_option(kind: str, fmt: str, note: str, missing: str) -> Option:
    if missing:
        # Shown, not hidden, so people can see the format exists and what unlocks it.
        return Option(f"[dim]{fmt.upper():<5} needs {missing}[/dim]", id=fmt, disabled=True)
    return Option(f"[b]{fmt.upper():<5}[/b] [dim]{escape(note)}[/dim]", id=fmt)


@dataclass
class ConvertOptions:
    """What the dialog offers. `for_video` builds it for a link; the Files tab for files."""

    title: str
    kinds: list[str]
    formats: dict[str, tuple[str, ...]]
    qualities: Callable[[str, str], list[str]]
    where: str  # shown after the arrow in the summary
    action: str = "download"
    local: bool = False  # files on disk, not a link: "best" keeps the original's quality
    note: Callable[[Choice], str] = lambda _choice: ""  # extra summary text for a choice
    note_for: Callable[[str, str], str] = format_note  # one-line note beside each format
    # The encoder a format needs and this ffmpeg lacks, or "". Such formats show greyed out.
    missing: Callable[[str, str], str] = lambda _kind, _fmt: ""
    sponsor: bool = False  # YouTube downloads can cut or mark SponsorBlock segments


def for_video(info: VideoInfo, where: str) -> ConvertOptions:
    if info.entries:
        where = f"{where}/{playlist_folder(info.title, info.channel)}"
    return ConvertOptions(
        title=info.title,
        kinds=["video", "audio"],
        formats={"video": VIDEO_FORMATS, "audio": AUDIO_FORMATS},
        qualities=lambda kind, fmt: (
            video_qualities(fmt, info.max_height) if kind == "video" else audio_qualities(fmt)
        ),
        where=where,
        sponsor=info.extractor.lower() in {"", "youtube", "youtubetab"},
    )


def remembered(settings: Settings, choice: Choice) -> Settings:
    """Settings with `choice` saved, so the dialog reopens where the user left it."""
    saved = replace(
        settings,
        last_kind=choice.kind,
        **{f"last_{choice.kind}_format": choice.fmt, f"last_{choice.kind}_quality": choice.quality},
    )
    if choice.kind == "video":
        saved = replace(
            saved,
            subtitle_mode=choice.subtitles,
            subtitle_langs=", ".join(choice.languages),
            sponsorblock=choice.sponsorblock,
        )
    if choice.kind in {"video", "audio"}:
        saved = replace(saved, split_chapters=choice.split_chapters, loudnorm=choice.loudnorm)
    if choice.kind == "image":
        saved = replace(
            saved,
            image_max_kb="" if choice.max_kb is None else str(choice.max_kb),
            strip_exif=choice.strip_metadata,
        )
    return saved


SUBS = (("off", "Off"), ("embed", "Inside the video"), ("srt", "Separate .srt"))
SPONSOR = (("off", "Off"), ("cut", "Cut segments"), ("chapters", "Mark as chapters"))
SWITCH = (("off", "Off"), ("on", "On"))
META = (("keep", "Keep"), ("strip", "Strip location"))


class CycleValue(Static):
    """A value on the right of an Extras row. Enter or click moves to the next one."""

    can_focus = True

    class Cycled(Message):
        pass

    def __init__(self, options: tuple[tuple[str, str], ...], value: str, **kwargs) -> None:
        super().__init__(**kwargs)
        self.options = options
        keys = [key for key, _label in options]
        self.value = value if value in keys else keys[0]

    def on_mount(self) -> None:
        self.update(self._label())

    def _label(self) -> str:
        return dict(self.options)[self.value]

    def cycle(self, step: int = 1) -> None:
        keys = [key for key, _label in self.options]
        self.value = keys[(keys.index(self.value) + step) % len(keys)]
        self.update(self._label())
        self.post_message(self.Cycled())

    def on_click(self) -> None:
        self.cycle()

    def on_key(self, event: events.Key) -> None:
        if event.key in {"enter", "space", "right"}:
            self.cycle(1)
            event.stop()
        elif event.key == "left":
            self.cycle(-1)
            event.stop()


class ConvertPanel(Vertical):
    """One cell of the convert grid. The name sits on the border, not on a second row inside."""

    def __init__(self, title: str) -> None:
        super().__init__(classes="convert-panel")
        self.border_title = title


class ExtraLine(Horizontal):
    def __init__(self, label: str, value: Widget) -> None:
        super().__init__(classes="extra-line")
        self._label = label
        self._value = value

    def compose(self) -> ComposeResult:
        yield Static(self._label, classes="extra-label")
        yield self._value


class ConvertScreen(ModalScreen[Choice | None]):
    BINDINGS = [Binding("escape", "cancel", "Cancel")]

    def __init__(self, source: VideoInfo | ConvertOptions, settings: Settings) -> None:
        super().__init__()
        self.settings = settings
        if isinstance(source, ConvertOptions):
            self.options = source
        else:
            from ..widgets.queue import short_path

            self.options = for_video(source, short_path(str(settings.download_path)))

    def compose(self) -> ComposeResult:
        opts = self.options
        initial = self.settings.last_kind if self.settings.last_kind in opts.kinds else None
        with Vertical(id="dialog", classes="convert"):
            yield Static(f"[b]Convert[/b]  [dim]{escape(opts.title)}[/dim]", id="dialog-title")
            with TabbedContent(initial=initial or opts.kinds[0], id="kind"):
                for kind in opts.kinds:
                    with TabPane(TAB_NAMES[kind], id=kind):
                        yield from self._grid(kind)
            yield Static(id="summary")
            with Horizontal(id="dialog-actions"):
                yield Button(opts.action.capitalize(), id="go", variant="primary")
                yield Button("Cancel", id="cancel")

    def _grid(self, kind: str) -> ComposeResult:
        opts = self.options
        quality = "Rate" if kind == "frames" else "Size" if kind == "image" else "Quality"
        with Grid(classes="convert-grid"):
            with ConvertPanel("Format"):
                yield OptionList(
                    *(
                        format_option(kind, fmt, opts.note_for(kind, fmt), opts.missing(kind, fmt))
                        for fmt in opts.formats[kind]
                    ),
                    id=f"{kind}-format",
                )
            with ConvertPanel(quality):
                yield OptionList(id=f"{kind}-quality")
            yield from self._third(kind)
            yield from self._fourth(kind)

    def _third(self, kind: str) -> ComposeResult:
        if kind in {"video", "audio"}:
            with ConvertPanel("Clip"):
                with Horizontal(classes="clip-fields"):
                    with Vertical():
                        yield Label("Start", classes="picker-label")
                        yield Input(placeholder="optional", id=f"{kind}-start")
                    with Vertical():
                        yield Label("End", classes="picker-label")
                        yield Input(placeholder="optional", id=f"{kind}-end")
                yield Static("Whole video if blank\n1:30 · 90 · 1m30s", classes="clip-hint")
            return
        if kind == "image":
            with ConvertPanel("Max size"):
                yield Input(
                    self.settings.image_max_kb,
                    placeholder="KB, optional",
                    id=f"{kind}-maxkb",
                    classes="max-size",
                )
                yield Static("Blank keeps the current size.", classes="clip-hint")
            return
        with ConvertPanel("Clip"):
            yield Static("Frames use the whole video.", classes="panel-note")

    def _fourth(self, kind: str) -> ComposeResult:
        saved = self.settings
        with ConvertPanel("Extras"):
            if kind == "video" and not self.options.local:
                yield ExtraLine(
                    "Subtitles",
                    CycleValue(SUBS, saved.subtitle_mode, id=f"{kind}-subs"),
                )
                yield ExtraLine(
                    "Languages",
                    Input(saved.subtitle_langs, placeholder="en, ja", id=f"{kind}-langs"),
                )
                if self.options.sponsor:
                    yield ExtraLine(
                        "SponsorBlock",
                        CycleValue(SPONSOR, saved.sponsorblock, id=f"{kind}-sponsor"),
                    )
            if kind in {"video", "audio"}:
                yield ExtraLine(
                    "Chapters",
                    CycleValue(
                        SWITCH, "on" if saved.split_chapters else "off", id=f"{kind}-chapters"
                    ),
                )
                yield ExtraLine(
                    "Volume",
                    CycleValue(SWITCH, "on" if saved.loudnorm else "off", id=f"{kind}-volume"),
                )
            elif kind == "image":
                yield ExtraLine(
                    "Location",
                    CycleValue(META, "strip" if saved.strip_exif else "keep", id=f"{kind}-meta"),
                )
            elif kind == "frames":
                yield Static("Frames have no extras.", classes="panel-note")

    def on_mount(self) -> None:
        for kind in self.options.kinds:
            formats = self.options.formats[kind]
            last = getattr(self.settings, f"last_{kind}_format", "")
            fmt_list = self.query_one(f"#{kind}-format", OptionList)
            usable = [i for i, f in enumerate(formats) if not self.options.missing(kind, f)]
            index = formats.index(last) if last in formats else -1
            fmt_list.highlighted = index if index in usable else (usable or [0])[0]
            self._fill_qualities(kind)
        self.query_one(f"#{self.kind}-format", OptionList).focus()
        self._update_summary()

    @property
    def kind(self) -> str:
        return self.query_one("#kind", TabbedContent).active or self.options.kinds[0]

    def _selected(self, list_id: str) -> str | None:
        ol = self.query_one(f"#{list_id}", OptionList)
        if ol.highlighted is None:
            return None
        return ol.get_option_at_index(ol.highlighted).id

    def _fill_qualities(self, kind: str) -> None:
        fmt = self._selected(f"{kind}-format")
        if fmt is None:
            return
        qualities = self.options.qualities(kind, fmt)
        ql = self.query_one(f"#{kind}-quality", OptionList)
        previous = self._selected(f"{kind}-quality") or getattr(
            self.settings, f"last_{kind}_quality", ""
        )
        ql.clear_options()
        ql.add_options(
            [Option(quality_label(kind, q, fmt, self.options.local), id=q) for q in qualities]
        )
        ql.highlighted = qualities.index(previous) if previous in qualities else 0

    def _value(self, widget_id: str) -> str:
        found = self.query(f"#{widget_id}")
        if not found:
            return ""
        widget = found.first()
        if isinstance(widget, CycleValue):
            return widget.value
        if isinstance(widget, Input):
            return widget.value.strip()
        return ""

    def _clip(self, kind: str) -> tuple[float | None, float | None]:
        if kind not in {"video", "audio"}:
            return None, None
        return parse_time(self._value(f"{kind}-start")), parse_time(self._value(f"{kind}-end"))

    def choice(self) -> Choice | None:
        kind = self.kind
        fmt, quality = self._selected(f"{kind}-format"), self._selected(f"{kind}-quality")
        if not fmt or not quality:
            return None
        start, end = self._clip(kind) if kind in {"video", "audio"} else (None, None)
        subs = self._value(f"{kind}-subs") or "off"
        langs = parse_langs(self._value(f"{kind}-langs"))
        sponsor = self._value(f"{kind}-sponsor") or "off"
        chapters = self._value(f"{kind}-chapters") == "on"
        volume = self._value(f"{kind}-volume") == "on"
        raw_kb = self._value(f"{kind}-maxkb")
        max_kb = int(raw_kb) if raw_kb.isdigit() and int(raw_kb) > 0 else None
        strip = self._value(f"{kind}-meta") == "strip"
        return Choice(
            kind,
            fmt,
            quality,
            start=start,
            end=end,
            subtitles=subs if subs in {"off", "embed", "srt"} else "off",
            languages=langs,
            sponsorblock=sponsor if sponsor in {"off", "cut", "chapters"} else "off",
            split_chapters=chapters,
            loudnorm=volume,
            max_kb=max_kb,
            strip_metadata=strip,
        )

    def _problem(self) -> str:
        kind = self.kind
        if kind not in {"video", "audio"}:
            return ""
        try:
            start, end = self._clip(kind)
        except ValueError:
            return "Use a time like 1:30, 90, or 1m30s."
        if message := clip_error(start, end):
            return message
        if self._value(f"{kind}-subs") not in {"", "off"} and not parse_langs(
            self._value(f"{kind}-langs")
        ):
            return "List a language, such as en."
        return ""

    def _describe(self, choice: Choice) -> str:
        bits = [choice.label]
        if choice.start is not None or choice.end is not None:
            start = "0" if choice.start is None else format_seconds(choice.start)
            end = "end" if choice.end is None else format_seconds(choice.end)
            bits.append(f"{start}–{end}")
        if choice.subtitles != "off" and choice.languages:
            how = "inside" if choice.subtitles == "embed" else "srt"
            bits.append(f"{how} {', '.join(choice.languages)}")
        if choice.sponsorblock == "cut":
            bits.append("sponsors cut")
        elif choice.sponsorblock == "chapters":
            bits.append("sponsors as chapters")
        if choice.split_chapters:
            bits.append("one file per chapter")
        if choice.loudnorm:
            bits.append("even volume")
        if choice.max_kb:
            bits.append(f"≤ {choice.max_kb} KB")
        if choice.strip_metadata:
            bits.append("no location")
        return "  ·  ".join(bits)

    def _update_summary(self) -> None:
        problem = ""
        try:
            problem = self._problem()
        except Exception:
            problem = ""
        if problem:
            self.query_one("#summary", Static).update(f"[$error]{escape(problem)}[/]")
            return
        choice = None
        try:
            choice = self.choice()
        except ValueError:
            choice = None
        text = ""
        if choice:
            text = (
                f"[b]{escape(self._describe(choice))}[/b]"
                f"  [dim]→ {escape(self.options.where)}[/dim]"
            )
            if note := self.options.note(choice):
                text += f"\n[dim]{escape(note)}[/dim]"
        self.query_one("#summary", Static).update(text)

    @on(OptionList.OptionHighlighted)
    def _highlighted(self, event: OptionList.OptionHighlighted) -> None:
        list_id = event.option_list.id or ""
        if list_id.endswith("-format"):
            self._fill_qualities(list_id.split("-")[0])
        self._update_summary()

    @on(OptionList.OptionSelected)
    def _selected_option(self, event: OptionList.OptionSelected) -> None:
        # Enter on a format moves to quality; Enter on a quality downloads.
        list_id = event.option_list.id or ""
        if list_id.endswith("-format"):
            self.query_one(f"#{list_id.split('-')[0]}-quality", OptionList).focus()
        else:
            self.action_go()

    @on(TabbedContent.TabActivated)
    @on(Input.Changed)
    @on(CycleValue.Cycled)
    def _tab(self) -> None:
        self._update_summary()

    @on(Button.Pressed, "#go")
    def action_go(self) -> None:
        if self._problem():
            self._update_summary()
            return
        self.dismiss(self.choice())

    @on(Button.Pressed, "#cancel")
    def action_cancel(self) -> None:
        self.dismiss(None)
