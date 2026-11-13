# gingertea

Download and convert videos, audio and images in your terminal. Version 0.3.0 uses
Rust and Ratatui for the interface and independent yt-dlp,
FFmpeg/FFprobe and Deno executables for media processing. The original terminal
design is retained: large shaded wordmark, tea palettes, two source tabs, rounded
input, open section rules, thumbnail/metadata card, solid action buttons and
centered shortcut chips. Dialogs follow the original settings and four-panel
conversion layouts.

## Install and update

Install a native package with uv or pipx:

```sh
uv tool install gingertea
# or: pipx install gingertea
gingertea setup
gingertea
```

Existing users can upgrade from the Python version:

```sh
uv tool upgrade gingertea
# or: pipx upgrade gingertea
gingertea --version
gingertea setup
```

PyPI wheels contain the Rust executable for Linux x86-64, Windows x86-64, and
Apple Silicon and Intel Macs. Python 3.10+ is required by the package manager;
the interface runs natively. yt-dlp and Deno install alongside the app. `setup`
finds or downloads FFmpeg and other missing tools. Existing settings, download
history, and playlist archives are reused. Optional image tools are listed below.

Standalone executables are also available from
[GitHub Releases](https://github.com/orbitcord/gingertea/releases). Replace an
existing standalone executable to update it. `gingertea update` updates yt-dlp;
use your package manager to update gingertea itself.

## Build and run from source

```sh
cargo build --release --locked
./target/release/gingertea setup
./target/release/gingertea
```

`setup` uses installed tools when available and downloads missing tools into
Gingertea's data directory. It needs curl and unzip (PowerShell on Windows).
You can also install the media tools yourself, for example on macOS:

```sh
brew install yt-dlp ffmpeg deno
cargo install --path . --locked
```

The Rust application does not import or launch the old Python application. yt-dlp
is still Python software; its standalone executables bundle their own interpreter.
Python source under `src/` is retained for repeatable migration comparisons.

```sh
gingertea URL                     # open video details or a playlist
gingertea /path/to/file           # open local files
gingertea get URL -f mp4
gingertea get URL --audio -f mp3 -q 192
gingertea convert file.mov -f mp4
gingertea convert file.mp4 -f png --frames -q 1
gingertea convert file.mp4 -f mp3 --start 1:30 --end 2:00
gingertea probe file.mp4          # JSON metadata
gingertea sites youtube
gingertea update                  # update the external yt-dlp executable
```

Saved paths go to stdout, progress/errors to stderr. Conversions never replace
original files. Completed results move out of private staging directories; output
collisions get numbered names. Cancelled and failed jobs remove staged files.

## Terminal controls

Press Tab after typing to use shortcuts. Esc focuses the input; Enter loads a
URL, search, or dropped paths. Quoted paths and file URLs are accepted.

| Key | Action |
| --- | --- |
| `1` / `2`, Ctrl+T | Download / Convert files |
| `d` / `a` | Download selected videos / audio |
| `c` | Format, quality, clips, subtitles, SponsorBlock and conversion extras |
| `e`, ↑ / ↓, Space | Choose playlist items; `a` selects all, `n` clears, `i` inverts; First previews the first N |
| `p` / `x` | Pause waiting jobs / cancel first unfinished job |
| `f` / `r` | Browse files / remove a local file or history record |
| `h` | Searchable history; Enter opens, `d` reloads, `y` copies the URL |
| `s` / `3` | Settings |
| `l` / `4` | Search supported site extractors |
| `t` | Open the theme picker |
| `v` / `o` | Load clipboard / open last saved result |
| `u` / `q` | Update yt-dlp / quit |

Click tabs, buttons, shortcut chips, fields and options directly. Tab/Shift+Tab
moves focus between controls and action buttons; Enter/Space activates a focused
button. In forms, arrows choose formats/quality or cycle values; Enter opens a
settings dropdown. Text fields support caret movement, insertion and deletion.
Ctrl+U clears text, Ctrl+S saves or runs, and Esc cancels. Settings include the
output directory, filename template, browser cookies (off by default),
parallelism (1–4), and motion; press `t` for the theme picker. Arrows or Tab preview
themes, Enter/Space or a click saves, and Esc cancels. Configuration, history and
playlist archives use the Python app's existing platform directories.

Local formats retain the original video/audio/image target tables. Image output
requires the corresponding FFmpeg encoder. JPEG XL uses `cjxl`/`djxl`; HEIC uses
macOS `sips` or libheif's `heif-enc`/`heif-convert` on other platforms. Install
optional native image tools separately. Image metadata and encoder output can
differ from the old Pillow implementation.

## Motion

The header resolves into `gingertea` on launch. Pasting a link adds a brief border
sweep and acknowledgment; metadata lookup has a moving accent. Active downloads
shimmer inside the actual filled progress bar, followed by a short completion accent.
Animations stop when work finishes. Set `animations` to `false` in Settings, or
launch with `GINGERTEA_NO_ANIMATION=1`, for static presentation with live progress.

## Validation and benchmarks

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked                 # requires FFmpeg/FFprobe
cargo build --release --locked
# The following use the retained Python environment for baseline/testing only:
uv sync --locked
PYTHONPATH=src .venv/bin/python benchmarks/compare.py --runs 5
.venv/bin/python benchmarks/smoke.py
.venv/bin/python benchmarks/motion_check.py
.venv/bin/python benchmarks/design_check.py
.venv/bin/python benchmarks/layout_check.py
```

See [the migration and benchmark report](docs/rust-port.md) and
[raw measurements](benchmarks/results/comparison.json). Measurements separate
idle UI overhead from real conversion time. The smoke check uses a localhost
server rather than downloading public internet videos.

The old Python documentation is retained in [docs/python-reference.md](docs/python-reference.md).
The release checks build and test the Rust app and native wheels on Windows,
Linux, and both Mac architectures, including an upgrade from the Python release.
Browser login, live site behavior, and optional codec tools depend on your machine
and installed media tools.

## License

MIT. See [LICENSE](LICENSE).
