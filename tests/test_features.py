import pytest

from gingertea.engine import archive, history
from gingertea.engine.names import UnknownField, example_name, output_path, playlist_folder, render
from gingertea.engine.options import DOWNLOAD, Choice, build_options
from gingertea.engine.probe import is_collection, videos_tab
from gingertea.engine.times import clip_error, parse_time
from gingertea.screens.main import key_bar


def test_times():
    assert parse_time("") is None
    assert parse_time("1:30") == 90
    assert parse_time("90") == 90
    assert parse_time("1m30s") == 90
    assert parse_time("1:02:03") == 3723
    assert parse_time("1h2m3s") == 3723
    with pytest.raises(ValueError):
        parse_time("1:99")
    with pytest.raises(ValueError):
        parse_time("later")
    assert clip_error(90, 90) == "The end time has to come after the start."
    assert clip_error(10, 20) == ""


def test_channel_link_opens_the_videos_tab():
    assert videos_tab("https://www.youtube.com/@fieldnotes") == (
        "https://www.youtube.com/@fieldnotes/videos"
    )
    assert videos_tab("https://www.youtube.com/@fieldnotes/videos").endswith("/videos")
    assert "watch" in videos_tab("https://www.youtube.com/watch?v=abc")
    assert is_collection("https://www.youtube.com/playlist?list=PL1")
    assert is_collection("https://www.youtube.com/@fieldnotes")
    assert not is_collection("https://www.youtube.com/watch?v=abc&list=PL1")
    assert is_collection("ytsearch8:kyoto")


def test_filename_template(tmp_path):
    assert example_name("{channel}/{date} {title}") == "Ginger Tea/2024-01-02 Tea and Ginger.mp4"
    with pytest.raises(UnknownField):
        render("{nope}", {})
    path = output_path(
        tmp_path,
        "{title} [{id}]",
        {"title": "Tea", "id": "abc", "channel": "", "date": "", "index": "", "ext": ""},
        "mp4",
        folder="Kyoto walks",
    )
    assert path == tmp_path / "Kyoto walks" / "Tea [abc].mp4"
    assert playlist_folder("Videos", "Field Notes") == "Field Notes"
    assert playlist_folder("Kyoto walks", "Field Notes") == "Kyoto walks"


def test_subtitles_sponsorblock_and_clip_options():
    choice = Choice(
        "video",
        "mkv",
        "1080",
        start=10,
        end=20,
        subtitles="embed",
        languages=("en", "ja"),
        sponsorblock="cut",
        loudnorm=True,
        split_chapters=True,
    )
    opts = build_options(choice, "/tmp/out")
    keys = [pp["key"] for pp in opts["postprocessors"]]
    assert "SponsorBlock" in keys and "ModifyChapters" in keys
    assert "FFmpegEmbedSubtitle" in keys
    assert "FFmpegSplitChapters" in keys
    assert opts["subtitleslangs"] == ["en", "ja"]
    assert opts["download_ranges"]({"duration": 100}, None) == [{"start_time": 10, "end_time": 20}]
    assert opts["postprocessor_args"]["videoconvertor"][0] == "-c:v"


def test_plain_download_options_stay_the_same():
    opts = build_options(DOWNLOAD, "/tmp/out")
    assert "download_ranges" not in opts
    assert [pp["key"] for pp in opts["postprocessors"]] == [
        "FFmpegThumbnailsConvertor",
        "FFmpegVideoRemuxer",
        "FFmpegMetadata",
        "EmbedThumbnail",
    ]


def test_history_and_archive(tmp_path):
    history.add("https://example.com/a", "Tea", str(tmp_path / "a.mp4"), "abc")
    assert history.search("tea")[0]["url"].endswith("/a")
    history.forget(history.search("")[0]["id"])
    assert history.search("") == []
    assert not archive.contains("youtube", "abc")
    archive.add("YouTube", "abc")
    assert archive.contains("youtube", "abc")


def test_get_and_convert_print_paths(monkeypatch, capsys, tmp_path):
    from gingertea.commands import convert_command, get_command
    from gingertea.engine.local import AUDIO, FileInfo, Stream
    from gingertea.engine.probe import VideoInfo
    from gingertea.engine.runner import Progress

    info = VideoInfo(
        "https://example.com/v",
        "abc",
        "Tea",
        "Notes",
        10,
        None,
        None,
        "Youtube",
        720,
    )

    def fake_run(job, download_dir, on_progress, cookies_browser=""):
        on_progress(Progress("downloading", "Downloading", 0.5))
        path = tmp_path / "out.mp4"
        path.write_text("x")
        return str(path)

    monkeypatch.setattr("gingertea.commands.probe", lambda url, cookies_browser="": info)
    monkeypatch.setattr("gingertea.commands.runner.run_job", fake_run)
    assert get_command([info.url, "--dir", str(tmp_path)]) == 0
    captured = capsys.readouterr()
    assert str(tmp_path / "out.mp4") in captured.out
    assert "Downloading" in captured.err

    source = FileInfo(tmp_path / "song.mp3", AUDIO, "mp3", 10, 2.0, None, Stream("mp3"))

    def fake_probe(path, ffmpeg_dir=None):
        return source

    def fake_convert(file_info, choice, on_progress, cancel_event, ffmpeg_dir=None):
        on_progress(Progress("converting", "Converting", 1.0))
        return str(tmp_path / "song.mp3")

    monkeypatch.setattr("gingertea.commands.local.probe_file", fake_probe)
    monkeypatch.setattr("gingertea.commands.local.run_convert", fake_convert)
    monkeypatch.setattr("gingertea.commands.deps.ffmpeg_candidates", lambda: [])
    assert convert_command([str(source.path), "-f", "mp3"]) == 0
    assert "song.mp3" in capsys.readouterr().out


def test_key_bar_offers_the_new_shortcuts():
    bar = key_bar(False, "download", 140)
    for label in ("choose", "audio", "pause", "history", "paste"):
        assert label in bar
