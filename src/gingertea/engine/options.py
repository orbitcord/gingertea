"""Turn a format/quality choice into yt-dlp options. Pure: no I/O."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass, replace

from ..config import Settings

VIDEO_FORMATS = ("mp4", "mkv", "webm", "mov", "gif")
AUDIO_FORMATS = ("mp3", "m4a", "opus", "flac", "wav", "ogg")

VIDEO_HEIGHTS = (2160, 1440, 1080, 720, 480, 360)
GIF_HEIGHTS = (480, 360, 240)
AUDIO_BITRATES = ("320", "192", "128")
# Image sizes: "best" keeps the original, "50" halves it, others cap the long side in pixels.
IMAGE_SIZES = ("3840", "1920", "1280")
# Frames per second to pull out of a video; "best" keeps every frame.
FRAME_RATES = ("best", "10", "1", "0.1")
FRAME_RATE_LABELS = {"best": "every frame", "10": "10/s", "1": "1/s", "0.1": "1 every 10s"}
LOSSLESS_AUDIO = ("flac", "wav")

# Containers that can carry embedded cover art.
THUMBNAIL_EXTS = {"mp4", "mov", "mkv", "m4a", "mp3", "flac", "opus", "ogg"}

# yt-dlp names the Vorbis codec, not the ogg container.
AUDIO_CODECS = {"ogg": "vorbis"}

# Prefer codecs each container plays well with; resolution still wins first.
VIDEO_SORT = {
    "mp4": ["vcodec:h264", "acodec:aac"],
    "mov": ["vcodec:h264", "acodec:aac"],
    "webm": ["vcodec:vp9", "acodec:opus"],
    "mkv": [],
}

# Containers whose "best" prefers H.264 so nothing has to be re-encoded to play everywhere.
H264_FIRST = ("mp4", "mov")

OUTPUT_TEMPLATE = "%(title).180B [%(id)s].%(ext)s"


# SponsorBlock categories worth cutting. Highlights and full-video chapters stay.
SPONSOR_CATEGORIES = (
    "sponsor",
    "intro",
    "outro",
    "selfpromo",
    "preview",
    "filler",
    "interaction",
)
EMBED_SUBS = {"mp4", "mkv", "webm"}
LOUDNORM = "loudnorm=I=-16:TP=-1.5:LRA=11"


@dataclass(frozen=True)
class Choice:
    kind: str  # "video", "audio", or for local files also "image" and "frames"
    fmt: str
    # "best", a height like "1080", a bitrate like "320", an image size like "1920" or "50"
    # (percent), or a frame rate like "1".
    quality: str = "best"
    start: float | None = None
    end: float | None = None
    subtitles: str = "off"  # off, embed, srt
    languages: tuple[str, ...] = ()
    sponsorblock: str = "off"  # off, cut, chapters
    split_chapters: bool = False
    loudnorm: bool = False
    max_kb: int | None = None
    strip_metadata: bool = False

    @property
    def label(self) -> str:
        if self.kind == "frames":
            return (
                f"{self.fmt.upper()} frames · {FRAME_RATE_LABELS.get(self.quality, self.quality)}"
            )
        if self.kind == "image":
            if self.quality == "best":
                q = "original"
            elif self.quality == "50":
                q = "50%"
            else:
                q = f"{self.quality}px"
        elif self.quality == "best":
            q = "best"
        elif self.kind == "video":
            q = f"{self.quality}p"
        else:
            q = f"{self.quality}k"
        return f"{self.fmt.upper()} · {q}"


DOWNLOAD = Choice("video", "mp4", "best")


def video_qualities(fmt: str, max_height: int | None) -> list[str]:
    """Quality options for a video format, limited to what the video has."""
    heights = GIF_HEIGHTS if fmt == "gif" else VIDEO_HEIGHTS
    if max_height:
        heights = tuple(h for h in heights if h <= max_height)
    if fmt == "gif":
        return [str(h) for h in heights] or ["240"]
    return ["best", *(str(h) for h in heights)]


def audio_qualities(fmt: str) -> list[str]:
    return ["best"] if fmt in LOSSLESS_AUDIO else ["best", *AUDIO_BITRATES]


def image_sizes(long_side: int | None) -> list[str]:
    """Size options for an image: original, half, and caps smaller than the image."""
    caps = [s for s in IMAGE_SIZES if not long_side or int(s) < long_side]
    return ["best", "50", *caps]


def gif_filter(height: str) -> str:
    """A palette pass makes GIFs look far better than ffmpeg's default dithering."""
    return (
        f"fps=10,scale=-2:{height}:flags=lanczos,split[a][b];"
        "[a]palettegen=stats_mode=diff[p];"
        "[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
    )


def build_options(
    choice: Choice,
    download_dir: str,
    *,
    ffmpeg_location: str | None = None,
    js_runtimes: dict | None = None,
    cookies_browser: str = "",
    native_vorbis: bool = False,
) -> dict:
    opts: dict = {
        "outtmpl": {"default": os.path.join(download_dir, OUTPUT_TEMPLATE)},
        "noplaylist": True,
        "quiet": True,
        "no_warnings": True,
        "noprogress": True,
        "windowsfilenames": True,
        "overwrites": False,
        "postprocessors": [],
    }
    if ffmpeg_location:
        opts["ffmpeg_location"] = ffmpeg_location
    if js_runtimes:
        opts["js_runtimes"] = js_runtimes
    if cookies_browser:
        opts["cookiesfrombrowser"] = (cookies_browser,)

    pps = opts["postprocessors"]
    if choice.kind == "audio":
        _audio(choice, opts, pps)
        if choice.fmt == "ogg" and native_vorbis:
            # Some ffmpeg builds (e.g. Homebrew's) ship without libvorbis; use the built-in encoder.
            opts["postprocessor_args"] = {"extractaudio": ["-c:a", "vorbis", "-strict", "-2"]}
        final_ext = choice.fmt
    elif choice.fmt == "gif":
        _gif(choice, opts, pps)
        final_ext = "gif"
    else:
        _video(choice, opts, pps)
        final_ext = choice.fmt

    if final_ext != "gif":
        pps.append({"key": "FFmpegMetadata", "add_metadata": True, "add_chapters": True})
    if final_ext in THUMBNAIL_EXTS:
        opts["writethumbnail"] = True
        # mp4/m4a/mp3 can't hold webp art, so normalise thumbnails to jpg first.
        pps.insert(0, {"key": "FFmpegThumbnailsConvertor", "format": "jpg", "when": "before_dl"})
        pps.append({"key": "EmbedThumbnail", "already_have_thumbnail": False})
    _extras(choice, opts, pps, final_ext)
    return opts


def parse_langs(text: str) -> tuple[str, ...]:
    return tuple(part for part in re.split(r"[\s,]+", text.strip()) if part)


def apply_saved(choice: Choice, settings: Settings) -> Choice:
    """A quick download (`d` or `a`) picks up the extras saved from the Convert dialog."""
    langs = parse_langs(settings.subtitle_langs)
    max_kb = int(settings.image_max_kb) if settings.image_max_kb.isdigit() else None
    video = choice.kind == "video"
    heard = choice.kind in {"video", "audio"}
    return replace(
        choice,
        subtitles=settings.subtitle_mode if video else "off",
        languages=langs if video else (),
        sponsorblock=settings.sponsorblock if video else "off",
        split_chapters=settings.split_chapters if heard else False,
        loudnorm=settings.loudnorm if heard else False,
        max_kb=max_kb if choice.kind == "image" else None,
        strip_metadata=settings.strip_exif if choice.kind == "image" else False,
    )


def _extras(choice: Choice, opts: dict, pps: list, final_ext: str) -> None:
    if choice.start is not None or choice.end is not None:
        opts["download_ranges"] = _range(choice.start, choice.end)
        opts["force_keyframes_at_cuts"] = True
    if choice.subtitles != "off" and choice.languages and choice.kind == "video":
        opts["writesubtitles"] = True
        opts["writeautomaticsub"] = True
        opts["subtitleslangs"] = list(choice.languages)
        opts["subtitlesformat"] = "srt/best"
        if choice.subtitles == "embed" and final_ext in EMBED_SUBS:
            pps.append({"key": "FFmpegEmbedSubtitle"})
        else:
            pps.append({"key": "FFmpegSubtitlesConvertor", "format": "srt"})
    if choice.sponsorblock in {"cut", "chapters"} and choice.kind == "video" and final_ext != "gif":
        categories = list(SPONSOR_CATEGORIES)
        block = {"key": "SponsorBlock", "categories": categories}
        modify = {
            "key": "ModifyChapters",
            "remove_sponsor_segments": categories if choice.sponsorblock == "cut" else [],
        }
        at = next((i for i, pp in enumerate(pps) if pp["key"] == "FFmpegMetadata"), len(pps))
        pps[at:at] = [block, modify]
    if choice.loudnorm and choice.kind == "audio":
        args = opts.setdefault("postprocessor_args", {}).setdefault("extractaudio", [])
        args.extend(["-af", LOUDNORM])
    if choice.loudnorm and choice.kind == "video" and final_ext != "gif":
        pps.append({"key": "FFmpegVideoConvertor", "preferedformat": final_ext})
        opts.setdefault("postprocessor_args", {})["videoconvertor"] = [
            "-c:v",
            "copy",
            "-af",
            LOUDNORM,
            "-c:a",
            "aac",
            "-b:a",
            "192k",
        ]
    if choice.split_chapters and choice.kind in {"video", "audio"} and final_ext != "gif":
        pps.append({"key": "FFmpegSplitChapters"})


def _range(start: float | None, end: float | None):
    """yt-dlp calls this with (info, ydl) and downloads only the returned section."""

    def callback(info: dict, _ydl: object) -> list[dict]:
        stop = end if end is not None else (info.get("duration") or 10**8)
        return [{"start_time": 0 if start is None else start, "end_time": stop}]

    return callback


def _video(choice: Choice, opts: dict, pps: list) -> None:
    res = "res" if choice.quality == "best" else f"res:{choice.quality}"
    opts["format"] = "bv*+ba/b"
    if choice.fmt in H264_FIRST and choice.quality == "best":
        # "Best" MP4/MOV means the best copy that plays everywhere: H.264 + AAC merge straight
        # in, no re-encode. YouTube only has H.264 up to 1080p; above that it's VP9/AV1, and
        # turning those into H.264 re-encodes the whole video (minutes for 4K).
        opts["format_sort"] = ["vcodec:h264", "acodec:aac", res]
    else:
        opts["format_sort"] = [res, *VIDEO_SORT[choice.fmt]]
    if choice.fmt == "mp4":
        # MP4 can hold VP9/AV1/Opus too, so merge and remux by copying streams, never
        # re-encoding. A 1440p/2160p pick may then not open in older players.
        opts["merge_output_format"] = "mp4"
        pps.append({"key": "FFmpegVideoRemuxer", "preferedformat": "mp4"})
    elif choice.fmt == "mov" and choice.quality == "best":
        # H.264 + AAC copy into MOV as-is.
        opts["merge_output_format"] = "mov/mkv"
        pps.append({"key": "FFmpegVideoRemuxer", "preferedformat": "mov"})
    else:
        # Fall back to mkv if the streams can't go into the target container, then convert;
        # FFmpegVideoConvertor skips when the file already has the right ext.
        opts["merge_output_format"] = f"{choice.fmt}/mkv"
        pps.append({"key": "FFmpegVideoConvertor", "preferedformat": choice.fmt})


def _gif(choice: Choice, opts: dict, pps: list) -> None:
    height = choice.quality if choice.quality != "best" else str(GIF_HEIGHTS[0])
    opts["format"] = "bv*/b"
    opts["format_sort"] = [f"res:{height}"]
    pps.append({"key": "FFmpegVideoConvertor", "preferedformat": "gif"})
    opts["postprocessor_args"] = {
        "videoconvertor": [
            "-vf",
            gif_filter(height),
            "-loop",
            "0",
            "-an",
        ]
    }


def _audio(choice: Choice, opts: dict, pps: list) -> None:
    opts["format"] = "ba/b"
    quality = "0" if choice.quality == "best" else choice.quality
    pps.append(
        {
            "key": "FFmpegExtractAudio",
            "preferredcodec": AUDIO_CODECS.get(choice.fmt, choice.fmt),
            "preferredquality": quality,
        }
    )
