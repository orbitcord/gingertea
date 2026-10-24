"""Downloads remembered after gingertea quits."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from uuid import uuid4

from .. import config


def _load() -> list[dict]:
    path = config.history_path()
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    entries = []
    for line in lines:
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict) and item.get("id"):
            entries.append(item)
    return entries


def _write(entries: list[dict]) -> None:
    path = config.history_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    text = "".join(json.dumps(item, ensure_ascii=False) + "\n" for item in entries)
    path.write_text(text, encoding="utf-8")


def add(url: str, title: str, path: str, video_id: str = "") -> dict:
    item = {
        "id": uuid4().hex[:12],
        "url": url,
        "title": title,
        "path": path,
        "video_id": video_id,
        "saved": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    entries = _load()
    entries.append(item)
    _write(entries)
    return item


def all_entries() -> list[dict]:
    return list(reversed(_load()))


def search(query: str) -> list[dict]:
    needle = query.strip().casefold()
    entries = all_entries()
    if not needle:
        return entries
    return [
        item
        for item in entries
        if needle in item.get("title", "").casefold()
        or needle in item.get("url", "").casefold()
        or needle in item.get("path", "").casefold()
    ]


def forget(entry_id: str) -> None:
    _write([item for item in _load() if item.get("id") != entry_id])
