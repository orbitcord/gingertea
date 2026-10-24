import pytest

from gingertea import config


@pytest.fixture(autouse=True)
def isolated_config(tmp_path, monkeypatch):
    """Keep tests away from the real config file and Downloads folder."""
    monkeypatch.setattr(config, "config_path", lambda: tmp_path / "config.toml")
    monkeypatch.setattr(config, "default_download_dir", lambda: str(tmp_path / "downloads"))
    monkeypatch.setattr(config, "history_path", lambda: tmp_path / "history.jsonl")
    monkeypatch.setattr(config, "archive_path", lambda: tmp_path / "archive.txt")
    monkeypatch.setenv("GINGERTEA_NO_UPDATE_CHECK", "1")
    monkeypatch.setenv("GINGERTEA_NO_FFMPEG_FETCH", "1")
    monkeypatch.setenv("GINGERTEA_NO_CLIPBOARD", "1")
    return tmp_path
