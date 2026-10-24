"""Filename templates. `{channel}/{date} {title}` becomes folders; unknown fields are refused."""

from __future__ import annotations

import re
from pathlib import Path

KNOWN = ("title", "id", "channel", "date", "index", "ext")
DEFAULT_TEMPLATE = "{title} [{id}]"
_FIELD = re.compile(r"\{([A-Za-z0-9_]+)\}")
_BAD = re.compile(r'[<>:"\\|?*\x00-\x1f]')
_GENERIC_PLAYLIST = {"videos", "shorts", "streams", "uploads", "playlist", "featured"}

SAMPLE = {
    "title": "Tea and Ginger",
    "id": "dQw4w9Wg",
    "channel": "Ginger Tea",
    "date": "2024-01-02",
    "index": "03",
    "ext": "mp4",
}


class UnknownField(ValueError):
    def __init__(self, name: str) -> None:
        self.name = name
        super().__init__(name)


def sanitize(part: str) -> str:
    cleaned = _BAD.sub("", part).strip().strip(".")
    return cleaned[:180]


def format_date(value: str | None) -> str:
    if value and len(value) == 8 and value.isdigit():
        return f"{value[:4]}-{value[4:6]}-{value[6:]}"
    return value or ""


def render(template: str, fields: dict[str, str]) -> str:
    """`template` with fields filled in. `/` splits folders. Raises UnknownField."""

    def replace(match: re.Match[str]) -> str:
        key = match.group(1)
        if key not in KNOWN:
            raise UnknownField(key)
        return fields.get(key, "")

    text = _FIELD.sub(replace, template.strip())
    text = re.sub(r" {2,}", " ", text)
    parts = []
    for part in text.split("/"):
        clean = sanitize(part).strip(" .-_")
        if clean and clean != "..":
            parts.append(clean)
    return "/".join(parts) or "download"


def with_extension(rendered: str, ext: str) -> str:
    if rendered.lower().endswith("." + ext.lower()):
        return rendered
    return f"{rendered}.{ext}"


def example_name(template: str) -> str:
    return with_extension(render(template, SAMPLE), "mp4")


def fields_for(info: dict, index: int | None = None) -> dict[str, str]:
    return {
        "title": info.get("title") or "Untitled",
        "id": str(info.get("id") or ""),
        "channel": info.get("channel") or info.get("uploader") or "",
        "date": format_date(info.get("upload_date") or info.get("date")),
        "index": f"{index:02d}" if index else "",
        "ext": "",
    }


def output_path(
    root: Path, template: str, fields: dict[str, str], ext: str, folder: str = ""
) -> Path:
    name = with_extension(render(template or DEFAULT_TEMPLATE, {**fields, "ext": ext}), ext)
    base = root / sanitize(folder) if folder else root
    return base / name


def playlist_folder(title: str, channel: str) -> str:
    """A channel's Videos tab is named after the channel, not the word Videos."""
    if title.strip().lower() in _GENERIC_PLAYLIST and channel and channel != "—":
        return channel
    return title or channel or "playlist"
