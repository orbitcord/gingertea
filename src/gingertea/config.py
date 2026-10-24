"""User settings, stored as TOML in the platform config directory."""

from __future__ import annotations

import sys
from dataclasses import asdict, dataclass, fields
from pathlib import Path

import platformdirs
import tomli_w

if sys.version_info >= (3, 11):
    import tomllib
else:  # pragma: no cover
    import tomli as tomllib

APP_NAME = "gingertea"


def default_download_dir() -> str:
    return str(Path(platformdirs.user_downloads_dir()))


def config_path() -> Path:
    return Path(platformdirs.user_config_dir(APP_NAME, appauthor=False)) / "config.toml"


def data_dir() -> Path:
    return Path(platformdirs.user_data_dir(APP_NAME, appauthor=False))


def history_path() -> Path:
    return data_dir() / "history.jsonl"


def archive_path() -> Path:
    return data_dir() / "archive.txt"


@dataclass
class Settings:
    download_dir: str = ""
    # Last choice made in the Convert dialog, so it opens where the user left it.
    last_kind: str = "video"
    last_video_format: str = "mp4"
    last_video_quality: str = "best"
    last_audio_format: str = "mp3"
    last_audio_quality: str = "best"
    last_image_format: str = "jpg"
    last_image_quality: str = "best"
    last_frames_format: str = "png"
    last_frames_quality: str = "1"
    # Browser cookies stay off until the user opts in for a login-only video.
    cookies_browser: str = ""
    theme: str = "masala-chai"
    # How many downloads run at once. The queue itself can be longer.
    parallel: int = 1
    filename_template: str = "{title} [{id}]"
    subtitle_mode: str = "off"  # off, embed, srt
    subtitle_langs: str = ""
    sponsorblock: str = "off"  # off, cut, chapters
    split_chapters: bool = False
    loudnorm: bool = False
    image_max_kb: str = ""
    strip_exif: bool = False

    def __post_init__(self) -> None:
        if not self.download_dir:
            self.download_dir = default_download_dir()
        try:
            self.parallel = min(4, max(1, int(self.parallel)))
        except (TypeError, ValueError):
            self.parallel = 1
        if self.subtitle_mode not in {"off", "embed", "srt"}:
            self.subtitle_mode = "off"
        if self.sponsorblock not in {"off", "cut", "chapters"}:
            self.sponsorblock = "off"
        if not self.filename_template.strip():
            self.filename_template = "{title} [{id}]"

    @property
    def download_path(self) -> Path:
        return Path(self.download_dir).expanduser()


def load(path: Path | None = None) -> Settings:
    path = path or config_path()
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return Settings()
    values = {}
    for field in fields(Settings):
        if field.name not in data:
            continue
        try:
            values[field.name] = _coerce(field.type, data[field.name])
        except (TypeError, ValueError):
            continue
    return Settings(**values)


def _coerce(type_name: str, raw: object) -> object:
    # `from __future__ import annotations` makes field.type a string.
    if type_name == "str":
        if isinstance(raw, str):
            return raw
        raise TypeError
    if type_name == "int":
        return int(raw)  # type: ignore[arg-type]
    if type_name == "bool":
        if isinstance(raw, bool):
            return raw
        if isinstance(raw, str):
            return raw.lower() in {"1", "true", "yes", "on"}
        raise TypeError
    raise TypeError


def save(settings: Settings, path: Path | None = None) -> None:
    path = path or config_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(tomli_w.dumps(asdict(settings)), encoding="utf-8")
