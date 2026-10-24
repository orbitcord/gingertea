"""CLI messages also work when Windows redirects output using its legacy encoding."""

import io
import sys

import pytest

from gingertea import cli
from gingertea.engine import deps, update


@pytest.mark.parametrize("command", ["setup", "update"])
def test_commands_support_windows_output_encoding(command, monkeypatch):
    tool = deps.Tool("C:/gingertea/ffmpeg", bundled=True)
    monkeypatch.setattr(deps, "bundled_ffmpeg_ready", lambda: False)
    monkeypatch.setattr(deps, "fetch_bundled_ffmpeg", lambda **kwargs: tool)
    monkeypatch.setattr(deps, "system_ffmpeg", lambda: None)
    monkeypatch.setattr(deps, "ffmpeg_candidates", lambda: [tool])
    monkeypatch.setattr(deps, "ffmpeg_has_encoder", lambda *args: True)
    monkeypatch.setattr(update, "installed_version", lambda: "2026.8.19")
    monkeypatch.setattr(update, "latest_version", lambda: "2026.10.21")
    monkeypatch.setattr(update, "upgrade_command", lambda: ["uv", "tool", "upgrade", "yt-dlp"])
    monkeypatch.setattr(update, "upgrade", lambda: (True, ""))
    output = io.BytesIO()
    stream = io.TextIOWrapper(output, encoding="cp1252")
    monkeypatch.setattr(sys, "stdout", stream)

    assert cli.main([command]) == 0
    stream.flush()
    assert output.getvalue()
