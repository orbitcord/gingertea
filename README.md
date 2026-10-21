<pre>
 ██████╗ ██╗███╗   ██╗ ██████╗ ███████╗██████╗ ████████╗███████╗ █████╗
██╔════╝ ██║████╗  ██║██╔════╝ ██╔════╝██╔══██╗╚══██╔══╝██╔════╝██╔══██╗
██║  ███╗██║██╔██╗ ██║██║  ███╗█████╗  ██████╔╝   ██║   █████╗  ███████║
██║   ██║██║██║╚██╗██║██║   ██║██╔══╝  ██╔══██╗   ██║   ██╔══╝  ██╔══██║
╚██████╔╝██║██║ ╚████║╚██████╔╝███████╗██║  ██║   ██║   ███████╗██║  ██║
 ╚═════╝ ╚═╝╚═╝  ╚═══╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝   ╚═╝   ╚══════╝╚═╝  ╚═╝
</pre>

gingertea downloads videos from YouTube and 1,300+ other sites in your terminal. Paste a
link and press `d` to save an MP4, or press `c` to choose a format and quality. You can
also convert videos, audio and images already on your computer. It runs on
[yt-dlp](https://github.com/yt-dlp/yt-dlp), [ffmpeg](https://ffmpeg.org) and
[Textual](https://textual.textualize.io/).

## Install

On macOS or Linux:

```sh
curl -LsSf https://raw.githubusercontent.com/orbitcord/gingertea/main/install.sh | sh
```

On Windows, run this in PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/orbitcord/gingertea/main/install.ps1 | iex"
```

The installer sets up [uv](https://docs.astral.sh/uv/), Python, gingertea, and ffmpeg.
Set `GINGERTEA_NO_FFMPEG=1` to skip the ffmpeg download; gingertea can get it the first
time you need it.

If you already use uv or pipx:

```sh
uv tool install gingertea
pipx install gingertea          # needs Python 3.10+
gingertea setup                 # optional: get ffmpeg now
```

To install the current GitHub version:

```sh
uv tool install git+https://github.com/orbitcord/gingertea
```

## Use

```sh
gingertea                  # open the app
gingertea <url>            # open with a video link loaded
gingertea <file>           # open Convert files with a file loaded
gingertea setup            # get ffmpeg and check available formats
gingertea update           # update yt-dlp
gingertea --version
```

Paste a link and gingertea shows the title, channel, length, upload date and thumbnail.
The footer lists the keyboard shortcuts, and its shortcuts are clickable.

| Key | Action |
| --- | --- |
| `d` | Download the loaded video as MP4 |
| `c` | Choose a format and quality |
| `x` | Cancel the current job |
| `l` or `4` | Search supported sites |
| `s` or `3` | Open Settings |
| `t` | Choose a theme |
| `o` | Reveal the last saved file |
| `u` | Update yt-dlp |
| `ctrl+t` | Switch between Download and Convert files |
| `1` / `2` | Open Download / Convert files |
| `Tab` | Leave the URL field so letter shortcuts work |
| `esc` | Focus the current input field |
| `q` | Quit |

While the URL field has focus, letters go into the link. Press `Tab` first or click a
shortcut in the footer. You can add another link while a download runs; downloads run
one at a time in their queue. The tabs, wordmark, thumbnail and shortcuts adapt to
compact terminals so the input, actions and queue stay visible.

### Download formats

| Video | Audio |
| --- | --- |
| MP4, MKV, WebM, MOV, GIF | MP3, M4A, Opus, FLAC, WAV, OGG |
| best, 2160p … 360p | best, 320 / 192 / 128 kbps |

The quality list only shows options available for the video. The default MP4 choice
prefers H.264 for compatibility. YouTube usually limits H.264 to 1080p; higher MP4
resolutions may use VP9 or AV1, which some older players cannot play.

### Convert files on your computer

Open Convert files with `ctrl+t` or click the tab. Drop files into the terminal, paste
their paths, or press `f` to browse. You can add several files at once and mix videos,
audio and images. Dropping a file on the Download tab switches to Convert files.

Press `c` to choose a format. The dialog shows which files the selection applies to.
Use `r` to remove the selected file and `x` to cancel a conversion. The Frames tab
exports video frames to a folder of images.

| Video | Audio | Image |
| --- | --- | --- |
| MP4, MKV, WebM, MOV, M4V, AVI, TS, FLV, WMV, MPG, 3GP, OGV, GIF, animated PNG, animated WebP | MP3, M4A, AAC, Opus, OGG, FLAC, ALAC, WAV, AIFF, WMA, AC3, E-AC3, MP2, WavPack, TTA, CAF, AU, MKA | JPG, PNG, WebP, AVIF, JPEG XL, HEIC, JPEG 2000, TIFF, BMP, GIF, TGA, QOI, EXR, HDR, PPM, PCX, SGI, DPX |

Converted files go next to their originals. gingertea leaves each original unchanged.
When the streams already fit the selected format, it copies them without re-encoding.
The dialog says whether the conversion will copy or re-encode. Some formats need codecs
that an ffmpeg build may lack; unavailable options stay visible with a note about what's
missing. Pillow plugins provide support for HEIC and JPEG XL.

### Where files go

Downloads go to your Downloads folder. Names use the video title and ID, such as
`Title [video-id].mp4`. If a file with that name already exists, gingertea adds a
number, for example `Title [video-id] (2).mp4`. Press `s` to choose another folder.

Settings, including your theme and download folder, live in:

| Platform | File |
| --- | --- |
| macOS | `~/Library/Application Support/gingertea/config.toml` |
| Linux | `~/.config/gingertea/config.toml` |
| Windows | `%LOCALAPPDATA%\gingertea\config.toml` |

### Themes

Masala Chai is the default dark theme. Press `t` or click Themes
to choose Tea House, Matcha, Earl Grey, Masala Chai, Hibiscus, gingertea light, or a
built-in Textual theme. gingertea remembers your choice.

### Videos that need you signed in

For age-restricted or members-only videos, choose a browser in Settings where you are
signed in. gingertea can read cookies from Chrome, Firefox, Safari, Edge, Brave, Opera,
Vivaldi or Chromium through yt-dlp. Cookies stay off until you choose a browser. On
macOS, expect a Keychain prompt the first time.

### When downloads break

Sites change how they serve video, and yt-dlp ships updates to keep up. gingertea checks
for a newer yt-dlp at startup. Press `u` or run `gingertea update` to install it, then
try the link again. The Sites screen can check whether yt-dlp has a dedicated extractor
for a link. Browser cookies may help with login errors.

### Updates

With a uv tool or pipx install, gingertea checks PyPI for a newer release while it is
open. If one is available, the update runs in the background after you quit, and the
next launch uses the new version. Set `GINGERTEA_NO_AUTO_UPDATE=1` to turn this off.

### ffmpeg, Deno and image formats

Converting needs ffmpeg, and some YouTube downloads need Deno to run JavaScript. The
installer downloads gingertea's own ffmpeg build; `gingertea setup` installs it now, or
the app can get it the first time it is needed. If ffmpeg is already on your `PATH`,
gingertea uses it too. Deno and the Pillow plugins for HEIC and JPEG XL install with the
Python package. If a format still needs a codec your machine doesn't have, the Convert
dialog shows it as unavailable and names the missing codec.

## Uninstall

```sh
uv tool uninstall gingertea       # or: pipx uninstall gingertea
```

The cached ffmpeg binary stays outside the Python environment so updates don't download
it again. To remove it too, delete the `ffmpeg` folder next to the settings file:
`~/Library/Application Support/gingertea/ffmpeg` on macOS,
`~/.local/share/gingertea/ffmpeg` on Linux, or
`%LOCALAPPDATA%\gingertea\ffmpeg` on Windows.

## Development

```sh
git clone https://github.com/orbitcord/gingertea && cd gingertea
uv sync --locked
uv run gingertea
uv run pytest -q                   # offline tests
GINGERTEA_LIVE=1 uv run pytest -q   # also download real videos
uv run ruff check src tests
uv run ruff format --check src tests
uv build --no-sources
```

To release, update the version in `pyproject.toml`, run `uv lock`, then push the matching
`vX.Y.Z` tag. GitHub Actions runs the checks, builds the package, publishes to PyPI, and
creates a GitHub release. PyPI Trusted Publishing must be configured for owner
`orbitcord`, repository `gingertea`, workflow `release.yml`, and environment `pypi`.

## Please be considerate

Only download videos you have the right to download, and follow each site's terms.

## License

MIT. Source copyright notices remain in [LICENSE](LICENSE).
