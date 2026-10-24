"""Videos already saved, so a playlist run skips them the next time."""

from __future__ import annotations

from .. import config


def _key(extractor: str, video_id: str) -> str:
    return f"{extractor.casefold()} {video_id}"


def _read() -> set[str]:
    path = config.archive_path()
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return set()
    return {line.strip() for line in lines if line.strip()}


def contains(extractor: str, video_id: str) -> bool:
    if not extractor or not video_id:
        return False
    return _key(extractor, video_id) in _read()


def add(extractor: str, video_id: str) -> None:
    if not extractor or not video_id:
        return
    key = _key(extractor, video_id)
    if key in _read():
        return
    path = config.archive_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(key + "\n")
