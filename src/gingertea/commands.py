"""`gingertea get` and `gingertea convert` for scripts.

Progress is printed to stderr. Each saved path is printed to stdout, alone on its line.
"""

from __future__ import annotations

import argparse
import sys
import threading
from pathlib import Path

from . import config
from .engine import archive, deps, local, runner
from .engine.names import playlist_folder
from .engine.options import Choice, apply_saved, parse_langs
from .engine.probe import probe
from .engine.runner import Job, Progress
from .engine.times import clip_error, parse_time


def get_command(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="gingertea get", description="Download a video or playlist."
    )
    parser.add_argument("url")
    parser.add_argument("-f", "--format", default="")
    parser.add_argument("-q", "--quality", default="best")
    parser.add_argument("--audio", action="store_true")
    parser.add_argument("--start", default="")
    parser.add_argument("--end", default="")
    parser.add_argument("--subs", default="")
    parser.add_argument("--embed-subs", action="store_true")
    parser.add_argument("--srt", action="store_true")
    parser.add_argument("--sponsorblock", choices=("off", "cut", "chapters"), default="")
    parser.add_argument("--chapters", action="store_true")
    parser.add_argument("--loudnorm", action="store_true")
    parser.add_argument("--dir", default="")
    args = parser.parse_args(argv)
    settings = config.load()
    if args.dir:
        settings.download_dir = args.dir
    try:
        choice = _download_choice(args, settings)
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1
    try:
        info = probe(args.url, settings.cookies_browser)
    except Exception as exc:
        print(exc, file=sys.stderr)
        return 1
    targets = info.entries if info.entries else (info,)
    folder = playlist_folder(info.title, info.channel) if info.entries else ""
    saved = 0
    for index, item in enumerate(targets, 1):
        if info.entries and archive.contains(item.extractor or info.extractor, item.id):
            print(f"skip {item.title}", file=sys.stderr)
            continue
        job = Job(
            url=item.url,
            title=item.title,
            choice=choice,
            index=index if info.entries else None,
            folder=folder,
            template=settings.filename_template,
            video_id=item.id,
            extractor=item.extractor or info.extractor,
        )
        print(item.title, file=sys.stderr)
        try:
            path = runner.run_job(
                job, settings.download_path, _progress, cookies_browser=settings.cookies_browser
            )
        except Exception as exc:
            print(file=sys.stderr)
            print(f"{item.title}: {exc}", file=sys.stderr)
            return 1
        print(file=sys.stderr)
        print(path)
        archive.add(job.extractor, job.video_id)
        saved += 1
    return 0 if saved else 1


def convert_command(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="gingertea convert", description="Convert files on disk.")
    parser.add_argument("files", nargs="+")
    parser.add_argument("-f", "--format", required=True)
    parser.add_argument("-q", "--quality", default="best")
    parser.add_argument("--start", default="")
    parser.add_argument("--end", default="")
    parser.add_argument("--chapters", action="store_true")
    parser.add_argument("--loudnorm", action="store_true")
    parser.add_argument("--max-kb", type=int, default=0)
    parser.add_argument("--strip-exif", action="store_true")
    args = parser.parse_args(argv)
    try:
        choice = _convert_choice(args)
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1
    ffmpeg = next(iter(deps.ffmpeg_candidates()), None)
    ffmpeg_dir = ffmpeg.path if ffmpeg else None
    saved = 0
    for raw in args.files:
        path = Path(raw).expanduser()
        print(path.name, file=sys.stderr)
        try:
            info = local.probe_file(path, ffmpeg_dir)
            result = local.run_convert(info, choice, _progress, threading.Event(), ffmpeg_dir)
        except Exception as exc:
            print(file=sys.stderr)
            print(f"{path.name}: {exc}", file=sys.stderr)
            return 1
        print(file=sys.stderr)
        print(result)
        saved += 1
    return 0 if saved else 1


def _download_choice(args: argparse.Namespace, settings: config.Settings) -> Choice:
    kind = "audio" if args.audio else "video"
    fmt = args.format or ("mp3" if args.audio else "mp4")
    choice = apply_saved(Choice(kind, fmt, args.quality or "best"), settings)
    start, end = _clip(args.start, args.end)
    subs = "off"
    if args.embed_subs:
        subs = "embed"
    elif args.srt or args.subs:
        subs = "srt"
    langs = parse_langs(args.subs) or choice.languages
    sponsor = args.sponsorblock or choice.sponsorblock
    return Choice(
        kind,
        fmt,
        choice.quality,
        start=start,
        end=end,
        subtitles=subs if subs != "off" else choice.subtitles,
        languages=langs,
        sponsorblock=sponsor,
        split_chapters=args.chapters or choice.split_chapters,
        loudnorm=args.loudnorm or choice.loudnorm,
    )


def _convert_choice(args: argparse.Namespace) -> Choice:
    fmt = args.format.lower().lstrip(".")
    kind = _kind_for(fmt)
    start, end = _clip(args.start, args.end)
    return Choice(
        kind,
        fmt,
        args.quality or "best",
        start=start,
        end=end,
        split_chapters=args.chapters,
        loudnorm=args.loudnorm,
        max_kb=args.max_kb or None,
        strip_metadata=args.strip_exif,
    )


def _kind_for(fmt: str) -> str:
    if fmt in local.LOCAL_VIDEO_FORMATS:
        return "video"
    if fmt in local.LOCAL_AUDIO_FORMATS:
        return "audio"
    if fmt in local.LOCAL_IMAGE_FORMATS:
        return "image"
    raise ValueError(f"Unknown format: {fmt}")


def _clip(start: str, end: str) -> tuple[float | None, float | None]:
    parsed_start, parsed_end = parse_time(start), parse_time(end)
    if message := clip_error(parsed_start, parsed_end):
        raise ValueError(message)
    return parsed_start, parsed_end


def _progress(progress: Progress) -> None:
    if progress.fraction is None:
        line = progress.status
    else:
        line = f"{progress.status} {progress.fraction * 100:.0f}%"
    print(f"\r{line:<48}", file=sys.stderr, end="", flush=True)
