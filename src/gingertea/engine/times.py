"""Clip times typed the way people say them: 1:30, 90, or 1m30s."""

from __future__ import annotations

import re

_CLOCK = re.compile(r"^\d+:\d{1,2}(?::\d{1,2})?$")
_WORDS = re.compile(r"^(?:(\d+(?:\.\d+)?)h)?(?:(\d+(?:\.\d+)?)m)?(?:(\d+(?:\.\d+)?)s)?$")
_PLAIN = re.compile(r"^\d+(?:\.\d+)?s?$")


def parse_time(text: str) -> float | None:
    """Seconds, or None when `text` is blank. Raises ValueError when it isn't a time."""
    raw = text.strip().lower()
    if not raw:
        return None
    if _CLOCK.match(raw):
        parts = [float(part) for part in raw.split(":")]
        if any(part >= 60 for part in parts[1:]):
            raise ValueError(text)
        if len(parts) == 2:
            minutes, seconds = parts
            return minutes * 60 + seconds
        hours, minutes, seconds = parts
        return hours * 3600 + minutes * 60 + seconds
    if raw.endswith("s") or "h" in raw or "m" in raw:
        match = _WORDS.match(raw)
        if match is None or not any(match.groups()):
            raise ValueError(text)
        hours, minutes, seconds = (float(part) if part else 0.0 for part in match.groups())
        return hours * 3600 + minutes * 60 + seconds
    if _PLAIN.match(raw):
        return float(raw[:-1] if raw.endswith("s") else raw)
    raise ValueError(text)


def clip_error(start: float | None, end: float | None) -> str:
    """Why this range can't be used, or "" when it can."""
    if start is not None and end is not None and end <= start:
        return "The end time has to come after the start."
    return ""


def format_seconds(value: float) -> str:
    if value == int(value):
        return str(int(value))
    return f"{value:.3f}".rstrip("0").rstrip(".")
