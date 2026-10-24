"""Run one download job with yt-dlp and report progress through callbacks."""

from __future__ import annotations

import itertools
import shutil
import tempfile
import threading
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import deps
from .names import DEFAULT_TEMPLATE, fields_for, output_path, sanitize
from .options import Choice, build_options

_ids = itertools.count(1)

QUEUED, PREPARING, DOWNLOADING, CONVERTING, DONE, FAILED, CANCELLED = (
    "queued",
    "preparing",
    "downloading",
    "converting",
    "done",
    "failed",
    "cancelled",
)
FINISHED = (DONE, FAILED, CANCELLED)

# yt-dlp reports postprocessors by short name (the class name minus "FFmpeg" and "PP").
_PP_LABELS = {
    "ExtractAudio": "Converting audio",
    "VideoConvertor": "Converting video",
    "VideoRemuxer": "Packaging",
    "Merger": "Merging video and audio",
    "Metadata": "Adding details",
    "EmbedThumbnail": "Adding cover art",
}


@dataclass
class Job:
    url: str
    title: str
    choice: Choice
    id: int = field(default_factory=lambda: next(_ids))
    state: str = QUEUED
    status: str = "Waiting"
    fraction: float | None = 0.0
    speed: float | None = None
    eta: float | None = None
    filepath: str | None = None
    error: Exception | None = None
    # A local.FileInfo when this job converts a file on disk rather than downloading `url`.
    file: Any = field(default=None, repr=False)
    # Set from the UI thread; the download thread checks it at every progress update.
    cancel_event: threading.Event = field(default_factory=threading.Event, repr=False)
    index: int | None = None
    folder: str = ""
    template: str = ""
    video_id: str = ""
    extractor: str = ""

    def cancel(self) -> None:
        self.cancel_event.set()

    @property
    def cancelled(self) -> bool:
        return self.cancel_event.is_set()


@dataclass
class Progress:
    state: str
    status: str
    fraction: float | None = None
    speed: float | None = None
    eta: float | None = None


def run_job(
    job: Job,
    download_dir: Path,
    on_progress: Callable[[Progress], None],
    cookies_browser: str = "",
) -> str:
    """Download and convert `job`. Blocks; returns the final file path.

    Raises DownloadCancelled soon after `job.cancel()` is called.
    """
    on_progress(Progress(PREPARING, "Getting ready"))
    ffmpeg = deps.find_ffmpeg(fetch=False)
    if ffmpeg is None:
        on_progress(Progress(PREPARING, "Fetching ffmpeg (one time, ~70 MB)"))
        ffmpeg = deps.find_ffmpeg(fetch=True)

    download_dir.mkdir(parents=True, exist_ok=True)
    # Work in a private folder so intermediate files never clash with (or delete)
    # files already in the user's download folder; move the result out at the end.
    work = Path(tempfile.mkdtemp(prefix=".gingertea-", dir=download_dir))
    try:
        return _run(job, work, download_dir, ffmpeg, on_progress, cookies_browser)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def _run(job, work, download_dir, ffmpeg, on_progress, cookies_browser) -> str:
    # yt-dlp takes a while to import, so it loads with the first download, not at startup.
    from yt_dlp import YoutubeDL
    from yt_dlp.utils import DownloadCancelled

    opts = build_options(
        job.choice,
        str(work),
        ffmpeg_location=ffmpeg.path if ffmpeg else None,
        js_runtimes=deps.js_runtimes(),
        cookies_browser=cookies_browser,
        native_vorbis=bool(ffmpeg) and not deps.ffmpeg_has_encoder(ffmpeg.path, "libvorbis"),
    )

    streams = {"n": 0, "seen": set()}

    def progress_hook(d: dict) -> None:
        if job.cancelled:
            raise DownloadCancelled("Cancelled")
        fmt = (d.get("info_dict") or {}).get("format_id")
        if fmt not in streams["seen"]:
            streams["seen"].add(fmt)
            streams["n"] += 1
        if d["status"] == "downloading":
            total = d.get("total_bytes") or d.get("total_bytes_estimate")
            done = d.get("downloaded_bytes") or 0
            label = "Downloading" if streams["n"] <= 1 else f"Downloading part {streams['n']}"
            on_progress(
                Progress(
                    DOWNLOADING,
                    label,
                    fraction=(done / total) if total else None,
                    speed=d.get("speed"),
                    eta=d.get("eta"),
                )
            )

    def pp_hook(d: dict) -> None:
        if job.cancelled:
            raise DownloadCancelled("Cancelled")
        if d["status"] == "started" and (label := _PP_LABELS.get(d.get("postprocessor"))):
            on_progress(Progress(CONVERTING, label, fraction=None))

    opts["logger"] = deps.SilentLogger()
    opts["progress_hooks"] = [progress_hook]
    opts["postprocessor_hooks"] = [pp_hook]

    with YoutubeDL(opts) as ydl:
        info = ydl.extract_info(job.url, download=True)
    if not info:
        raise RuntimeError("Download finished but no file was produced.")
    return str(_publish(job, info, work, download_dir))


def unique_path(path: Path) -> Path:
    """`path`, or `name (2).ext`, `name (3).ext`... if it is already taken."""
    if not path.exists():
        return path
    for n in itertools.count(2):
        candidate = path.with_name(f"{path.stem} ({n}){path.suffix}")
        if not candidate.exists():
            return candidate
    raise AssertionError("unreachable")


def _publish(job: Job, info: dict, work: Path, download_dir: Path) -> Path:
    files = _outputs(work, Path(final_path(info)) if final_path(info) else None, job)
    if not files:
        raise RuntimeError("Download finished but no file was produced.")
    fields = fields_for(info, job.index)
    template = job.template or DEFAULT_TEMPLATE
    videos = [path for path in files if path.suffix.lower() != ".srt"]
    subs = [path for path in files if path.suffix.lower() == ".srt"]
    root = download_dir / sanitize(job.folder) if job.folder else download_dir
    if len(videos) <= 1 and videos:
        ext = videos[0].suffix.lstrip(".")
        target = unique_path(output_path(download_dir, template, fields, ext, job.folder))
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(videos[0]), target)
        for sub in subs:
            # Keep the language yt-dlp put in the name when there is more than one.
            name = sub.name if len(subs) > 1 else f"{target.stem}.srt"
            shutil.move(str(sub), unique_path(target.parent / name))
        return target
    folder = unique_path(root / sanitize(fields["title"] or job.title))
    folder.mkdir(parents=True, exist_ok=True)
    for src in files:
        shutil.move(str(src), unique_path(folder / src.name))
    return folder


def _outputs(work: Path, produced: Path | None, job: Job) -> list[Path]:
    extras = job.choice.split_chapters or job.choice.subtitles != "off"
    if produced is not None and produced.is_file() and not extras:
        return [produced]
    skip = {".part", ".ytdl", ".json", ".jpg", ".jpeg", ".webp", ".png", ".temp"}
    found: list[Path] = []
    if produced is not None and produced.is_file():
        found.append(produced)
    for path in sorted(work.rglob("*")):
        if not path.is_file() or path.suffix.lower() in skip or path in found:
            continue
        found.append(path)
    return found


def final_path(info: dict) -> str:
    for d in info.get("requested_downloads") or []:
        if d.get("filepath"):
            return d["filepath"]
    return info.get("filepath") or info.get("_filename") or ""
