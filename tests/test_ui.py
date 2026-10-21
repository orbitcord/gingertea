"""Drive the app with Textual's Pilot. Network calls are replaced with fakes."""

import asyncio
from pathlib import Path

import pytest
from textual import events

from gingertea import config
from gingertea.app import GingerTeaApp
from gingertea.engine import runner
from gingertea.engine.probe import VideoInfo
from gingertea.screens import main as main_screen
from gingertea.screens.convert import ConvertScreen
from gingertea.screens.settings import SettingsScreen

URL = "https://www.youtube.com/watch?v=abc123"


def fake_info(url=URL):
    return VideoInfo(
        url=url,
        id="abc123",
        title="A test video",
        channel="Tester",
        duration=95,
        upload_date="20240102",
        thumbnail=None,
        extractor="Youtube",
        max_height=1080,
    )


@pytest.fixture
def fakes(monkeypatch, isolated_config):
    calls = {"probe": [], "run": []}

    def fake_probe(url, cookies_browser=""):
        calls["probe"].append((url, cookies_browser))
        if "login" in url:
            raise Exception("ERROR: [youtube] x: Sign in to confirm your age")
        return fake_info(url)

    def fake_run_job(job, download_dir, on_progress, cookies_browser=""):
        calls["run"].append(job.choice)
        on_progress(runner.Progress(runner.DOWNLOADING, "Downloading", 0.5, 1_000_000, 3))
        path = Path(download_dir) / f"{job.title}.{job.choice.fmt}"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("x")
        return str(path)

    monkeypatch.setattr(main_screen, "probe", fake_probe)
    monkeypatch.setattr(main_screen.runner, "run_job", fake_run_job)
    return calls


async def settle(pilot, pred, timeout=5.0):
    for _ in range(int(timeout / 0.05)):
        await pilot.pause()
        if pred():
            return True
        await asyncio.sleep(0.05)
    return False


async def test_paste_then_download(fakes):
    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        screen = app.screen
        screen.query_one("#url").post_message(events.Paste(URL))
        assert await settle(pilot, lambda: screen.info is not None)
        # Widget.focus() queues a callback; wait for it as well as the probe result.
        assert await settle(pilot, lambda: app.focused is not None and app.focused.id == "download")
        await pilot.press("d")
        assert await settle(pilot, lambda: screen.jobs and screen.jobs[0].state == runner.DONE)
        assert fakes["run"][0].fmt == "mp4"
        assert Path(screen.last_file).exists()


async def test_convert_remembers_choice(fakes):
    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        screen = app.screen
        screen.load(URL)
        assert await settle(pilot, lambda: screen.info is not None)
        await pilot.press("c")
        assert await settle(pilot, lambda: isinstance(app.screen, ConvertScreen))
        dialog = app.screen
        dialog.query_one("#kind").active = "audio"
        await pilot.pause()
        dialog.query_one("#audio-format").highlighted = 3  # flac
        await pilot.pause()
        assert await settle(
            pilot, lambda: [o.id for o in dialog.query_one("#audio-quality").options] == ["best"]
        )
        await pilot.click("#go")
        assert await settle(pilot, lambda: screen.jobs and screen.jobs[0].state == runner.DONE)
        assert fakes["run"][0].fmt == "flac"
    saved = config.load()
    assert (saved.last_kind, saved.last_audio_format) == ("audio", "flac")


async def test_queue_runs_one_at_a_time(fakes):
    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        screen = app.screen
        screen.load(URL)
        assert await settle(pilot, lambda: screen.info is not None)
        await pilot.press("d", "d", "d")
        assert await settle(
            pilot,
            lambda: len(screen.jobs) == 3 and all(j.state == runner.DONE for j in screen.jobs),
        )
        assert len(fakes["run"]) == 3


async def test_login_error_offers_cookies(fakes):
    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        screen = app.screen
        screen.load("https://www.youtube.com/watch?v=login")
        assert await settle(
            pilot, lambda: screen.query_one("#card-switcher").current == "card-error"
        )
        assert screen.query_one("#use-cookies").display
        await pilot.click("#use-cookies")
        await pilot.pause()
        await pilot.press("down", "enter")  # Firefox
        assert await settle(pilot, lambda: len(fakes["probe"]) == 2)
        assert fakes["probe"][-1][1] == "firefox"
    assert config.load().cookies_browser == "firefox"


async def test_settings_changes_folder(fakes, tmp_path):
    target = tmp_path / "elsewhere"
    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        await pilot.press("escape")  # leave the URL bar so letter keys are shortcuts
        app.screen.query_one("#url").blur()
        await pilot.press("s")
        assert await settle(pilot, lambda: isinstance(app.screen, SettingsScreen))
        app.screen.query_one("#folder").value = str(target)
        await pilot.click("#save")
        await pilot.pause()
    assert config.load().download_dir == str(target)
    assert target.is_dir()


@pytest.mark.parametrize("width", [60, 80, 120])
async def test_logo_fits(fakes, width):
    app = GingerTeaApp()
    async with app.run_test(size=(width, 40)) as pilot:
        await pilot.pause()
        logo = app.screen.query_one("#logo")
        assert logo.size.width <= width


def all_theme_names():
    from gingertea.theme import THEMES

    return sorted(set(GingerTeaApp().available_themes) | {t.name for t in THEMES})


@pytest.mark.parametrize("theme", all_theme_names())
async def test_every_theme_renders(fakes, theme, tmp_path):
    """Each theme in the command palette survives every screen and job state."""
    from PIL import Image

    from gingertea.engine.options import DOWNLOAD
    from gingertea.engine.runner import Job

    app = GingerTeaApp()
    async with app.run_test(size=(120, 50)) as pilot:
        app.theme = theme
        screen = app.screen
        screen.load(URL)
        assert await settle(pilot, lambda: screen.info is not None)
        screen.query_one(main_screen.VideoCard).set_thumbnail(Image.new("RGB", (64, 36), "teal"))
        queue = screen.query_one(main_screen.DownloadQueue)
        for state in (runner.QUEUED, runner.DOWNLOADING, runner.DONE, runner.FAILED):
            job = Job(url=URL, title=state, choice=DOWNLOAD)
            job.state, job.fraction, job.status = state, 0.5, "Downloading"
            job.error = Exception("boom")
            queue.add(job)
        await pilot.pause()
        for screen_cls in (ConvertScreen, SettingsScreen):
            app.push_screen(
                screen_cls(screen.info, app.settings)
                if screen_cls is ConvertScreen
                else screen_cls(app.settings)
            )
            await pilot.pause()
            app.save_screenshot(path=str(tmp_path))  # forces a full render of the current screen
            app.pop_screen()
            await pilot.pause()
        screen.load("https://www.youtube.com/watch?v=login")
        assert await settle(
            pilot, lambda: screen.query_one("#card-switcher").current == "card-error"
        )
        await pilot.resize_terminal(60, 24)
        await pilot.pause()
        app.save_screenshot(path=str(tmp_path))

        # The Convert files tab, with a file listed and a conversion in each state.
        from gingertea.engine.local import VIDEO, FileInfo, Stream
        from gingertea.screens.files import FilePickerScreen, FilesPane

        await pilot.resize_terminal(120, 50)
        screen.action_show_tab("files")
        pane = screen.query_one(FilesPane)
        clip = FileInfo(tmp_path / "a.mov", VIDEO, "mov", 1, 2.0, Stream("h264", 64, 36), None)
        pane.files.append(clip)
        pane._render_files()
        for state in (runner.QUEUED, runner.CONVERTING, runner.DONE, runner.FAILED):
            job = Job(url=str(clip.path), title=state, choice=DOWNLOAD, file=clip)
            job.state, job.fraction, job.status = state, 0.5, "Converting"
            job.error = Exception("boom")
            pane.query_one("#conversions").add(job)
        await pilot.pause()
        app.save_screenshot(path=str(tmp_path))
        app.push_screen(ConvertScreen(pane.convert_options(), app.settings))
        await pilot.pause()
        app.save_screenshot(path=str(tmp_path))
        app.pop_screen()
        app.push_screen(FilePickerScreen(tmp_path))
        await pilot.pause()
        app.save_screenshot(path=str(tmp_path))


async def test_sites_list_searches_and_checks_links(fakes):
    from gingertea.screens.sites import SitesScreen

    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        app.screen.query_one("#url").blur()
        await pilot.press("l")
        assert await settle(pilot, lambda: isinstance(app.screen, SitesScreen))
        sites = app.screen.query_one("#sites")
        everything = sites.option_count
        assert everything > 1000
        search = app.screen.query_one("#site-search")
        search.value = "vimeo"
        await pilot.pause()
        assert 0 < sites.option_count < everything
        search.value = "https://vimeo.com/76979871"
        await pilot.pause()
        assert "Supported" in str(app.screen.query_one("#site-result").render())
        search.value = "https://example.com/some/page"
        await pilot.pause()
        assert "No dedicated support" in str(app.screen.query_one("#site-result").render())
        await pilot.press("escape")
        assert await settle(pilot, lambda: not isinstance(app.screen, SitesScreen))


async def test_cancel_running_and_waiting(fakes, monkeypatch):
    from yt_dlp.utils import DownloadCancelled

    def slow_run_job(job, download_dir, on_progress, cookies_browser=""):
        on_progress(runner.Progress(runner.DOWNLOADING, "Downloading", 0.1))
        if not job.cancel_event.wait(5):
            raise AssertionError("never cancelled")
        raise DownloadCancelled("Cancelled")

    monkeypatch.setattr(main_screen.runner, "run_job", slow_run_job)
    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        screen = app.screen
        screen.load(URL)
        assert await settle(pilot, lambda: screen.info is not None)
        await pilot.press("d", "d")
        assert await settle(pilot, lambda: screen.jobs[0].state == runner.DOWNLOADING)
        first, second = screen.jobs
        assert second.state == runner.QUEUED

        await pilot.click(f"#job-{second.id} .job-cancel")  # waiting job leaves the line
        assert await settle(pilot, lambda: second.state == runner.CANCELLED)
        assert first.state == runner.DOWNLOADING

        await pilot.press("x")  # cancels the running one
        assert await settle(pilot, lambda: first.state == runner.CANCELLED)
        assert screen.running is None
        assert not screen.query_one(f"#job-{first.id} .job-cancel").visible


async def test_status_line_shows_both_versions(fakes):
    from gingertea import __version__

    app = GingerTeaApp()
    async with app.run_test(size=(100, 40)) as pilot:
        await pilot.pause()
        text = str(app.screen.query_one("#status-line").render())
        assert f"gingertea {__version__}" in text and "yt-dlp" in text
        assert text.index("gingertea") < text.index("yt-dlp")


async def test_tabs_switch_workflows_and_shortcuts_open_settings(fakes):
    app = GingerTeaApp()
    async with app.run_test(size=(120, 38)) as pilot:
        # Tab labels retain clickable Rich actions after removing sidebar navigation.
        tabs = app.screen.query_one("#tabs")
        await pilot.click("#tabs", offset=(20, 0))
        assert app.screen.tab == "files"
        assert app.focused.id == "file-input"
        await pilot.click("#tabs", offset=(3, 0))
        assert app.screen.tab == "download"
        assert app.focused.id == "url"
        assert tabs.display
        assert not app.screen.query("#sidebar, #cup")
        app.screen.query_one("#url").blur()
        await pilot.press("s")
        assert isinstance(app.screen, SettingsScreen)
        await pilot.press("escape")
        await pilot.press("l")
        from gingertea.screens.sites import SitesScreen

        assert isinstance(app.screen, SitesScreen)


@pytest.mark.parametrize("size", [(60, 24), (40, 20)])
async def test_compact_layout_keeps_download_and_queue_visible(fakes, size):
    app = GingerTeaApp()
    async with app.run_test(size=size) as pilot:
        screen = app.screen
        screen.load(URL)
        assert await settle(pilot, lambda: screen.info is not None)
        download = screen.query_one("#download")
        queue = screen.query_one("#queue")
        footer = screen.query_one("#keys")
        assert download.region.height > 0
        assert download.region.bottom <= footer.region.y
        assert queue.region.height >= 3
        assert queue.region.bottom <= footer.region.y
        assert not screen.query("#sidebar, #cup")
        await pilot.press("ctrl+t")
        assert screen.tab == "files"
        assert app.focused.id == "file-input"


async def test_tea_theme_selection_is_saved_and_restored(fakes):
    app = GingerTeaApp()
    async with app.run_test(size=(120, 38)) as pilot:
        assert app.theme == "gingertea"
        app.theme = "hibiscus"
        await pilot.pause()
        assert config.load().theme == "hibiscus"
        main = app.screen
        main.query_one("#url").blur()
        await pilot.press("t")
        assert app.screen is not main
        await pilot.press("escape")
    app = GingerTeaApp()
    async with app.run_test(size=(120, 38)) as pilot:
        await pilot.pause()
        assert app.theme == "hibiscus"
