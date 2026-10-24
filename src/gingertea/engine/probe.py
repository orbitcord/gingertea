"""Fetch a video's details without downloading it."""

from __future__ import annotations

import io
import re
import urllib.request
from dataclasses import dataclass, field

from . import deps

# A channel home page lists tabs before it lists videos. Opening Videos directly is faster.
_BARE_CHANNEL = re.compile(
    r"^https?://(?:(?:www|m|music)\.)?youtube\.com/"
    r"(?:@[^/?#]+|channel/[^/?#]+|c/[^/?#]+|user/[^/?#]+)$",
    re.IGNORECASE,
)


@dataclass
class Entry:
    url: str
    id: str
    title: str
    duration: int | None
    channel: str
    extractor: str = ""


@dataclass
class VideoInfo:
    url: str
    id: str
    title: str
    channel: str
    duration: int | None
    upload_date: str | None  # YYYYMMDD
    thumbnail: str | None
    extractor: str
    max_height: int | None
    raw: dict = field(repr=False, default_factory=dict)
    entries: tuple[Entry, ...] = ()

    @property
    def is_playlist(self) -> bool:
        return bool(self.entries)

    @property
    def duration_text(self) -> str:
        if not self.duration:
            return "live" if self.raw.get("is_live") else "—"
        h, rem = divmod(int(self.duration), 3600)
        m, s = divmod(rem, 60)
        return f"{h}:{m:02}:{s:02}" if h else f"{m}:{s:02}"

    @property
    def date_text(self) -> str:
        d = self.upload_date
        if not d or len(d) != 8:
            return "—"
        return f"{d[:4]}-{d[4:6]}-{d[6:]}"


def max_video_height(formats: list[dict]) -> int | None:
    """Largest resolution on offer, using the short side so vertical videos count right."""
    best = 0
    for f in formats or []:
        if f.get("vcodec") in (None, "none") and not f.get("height"):
            continue
        w, h = f.get("width"), f.get("height")
        side = min(w, h) if w and h else h
        if side:
            best = max(best, int(side))
    return best or None


def pick_thumbnail(info: dict) -> str | None:
    """A small widescreen jpg is plenty for a terminal preview and loads fast.

    YouTube's default `hqdefault` is 4:3 with black bars, so prefer a ~16:9 one.
    """
    candidates = []
    for t in info.get("thumbnails") or []:
        w, h, url = t.get("width"), t.get("height"), t.get("url")
        if not (w and h and url) or w < 300:
            continue
        widescreen = abs(w / h - 16 / 9) < 0.1
        candidates.append((not widescreen, ".webp" in url, w, url))
    if candidates:
        return min(candidates)[3]
    return info.get("thumbnail")


def videos_tab(url: str) -> str:
    """A bare YouTube channel opens its Videos tab instead of the slower home page."""
    base = url.split("#", 1)[0].split("?", 1)[0].rstrip("/")
    if _BARE_CHANNEL.match(base):
        return base + "/videos"
    return url


def is_collection(url: str) -> bool:
    """A playlist, channel, or search, rather than one video."""
    if url.startswith("ytsearch"):
        return True
    lowered = url.lower()
    if "/watch" in lowered or "youtu.be/" in lowered:
        return False
    if "/playlist" in lowered:
        return True
    if re.search(r"[?&]list=", lowered) and not re.search(r"[?&]v=", lowered):
        return True
    return bool(re.search(r"youtube\.com/(?:@|channel/|c/|user/|feed/)", lowered))


def probe(url: str, cookies_browser: str = "") -> VideoInfo:
    if url.startswith("ytsearch") or is_collection(url):
        return probe_collection(videos_tab(url), cookies_browser)
    info = _extract(url, cookies_browser, flat=False, playlist=False)
    if info.get("_type") == "playlist":
        return _collection(info, url)
    return _video(info, url)


def probe_collection(url: str, cookies_browser: str = "") -> VideoInfo:
    return _collection(_extract(videos_tab(url), cookies_browser, flat=True, playlist=True), url)


def _extract(url: str, cookies_browser: str, *, flat: bool, playlist: bool) -> dict:
    from yt_dlp import YoutubeDL  # loaded with the first link, not at startup

    opts = {
        "quiet": True,
        "no_warnings": True,
        "noplaylist": not playlist,
        "extract_flat": flat,
        "skip_download": True,
        "js_runtimes": deps.js_runtimes(),
        "logger": deps.SilentLogger(),
    }
    if cookies_browser:
        opts["cookiesfrombrowser"] = (cookies_browser,)
    with YoutubeDL(opts) as ydl:
        info = ydl.extract_info(url, download=False)
    if info is None:
        raise ValueError("No video found at that link.")
    return info


def _video(info: dict, url: str) -> VideoInfo:
    return VideoInfo(
        url=info.get("webpage_url") or url,
        id=info.get("id", ""),
        title=info.get("title") or "Untitled",
        channel=info.get("channel") or info.get("uploader") or "—",
        duration=info.get("duration"),
        upload_date=info.get("upload_date"),
        thumbnail=pick_thumbnail(info),
        extractor=info.get("extractor_key") or "",
        max_height=max_video_height(info.get("formats") or []),
        raw=info,
    )


def _collection(info: dict, url: str) -> VideoInfo:
    if info.get("_type") != "playlist":
        return _video(info, url)
    parent_channel = info.get("channel") or info.get("uploader") or "—"
    parent_extractor = info.get("extractor_key") or ""
    entries = []
    for raw in info.get("entries") or []:
        if not raw:
            continue
        entry_url = _entry_url(raw)
        if not entry_url:
            continue
        duration = raw.get("duration")
        entries.append(
            Entry(
                url=entry_url,
                id=str(raw.get("id") or ""),
                title=raw.get("title") or "Untitled",
                duration=int(duration) if isinstance(duration, (int, float)) else None,
                channel=raw.get("channel") or raw.get("uploader") or parent_channel,
                extractor=(raw.get("ie_key") or raw.get("extractor_key") or parent_extractor),
            )
        )
    if not entries:
        raise ValueError("No videos in that playlist.")
    return VideoInfo(
        url=info.get("webpage_url") or url,
        id=info.get("id") or "",
        title=info.get("title") or "Playlist",
        channel=parent_channel,
        duration=None,
        upload_date=info.get("upload_date"),
        thumbnail=pick_thumbnail(info),
        extractor=parent_extractor,
        max_height=None,
        raw=info,
        entries=tuple(entries),
    )


def _entry_url(entry: dict) -> str:
    for key in ("webpage_url", "url"):
        value = entry.get(key) or ""
        if isinstance(value, str) and value.startswith("http"):
            return value
    video_id = str(entry.get("id") or "")
    extractor = (entry.get("ie_key") or entry.get("extractor_key") or "").lower()
    if video_id and extractor.startswith("youtube"):
        return f"https://www.youtube.com/watch?v={video_id}"
    return ""


def fetch_thumbnail(url: str, timeout: float = 8.0):
    """Download a thumbnail as a PIL image, or None if it can't be fetched."""
    from PIL import Image

    try:
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 gingertea"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = resp.read()
        return Image.open(io.BytesIO(data)).convert("RGB")
    except Exception:
        return None
