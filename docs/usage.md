# Usage reference

[Back to README](../README.md) · [Command line](#command-line) · [Controls](#terminal-controls) · [Media tools](#media-tools)

## Command line

```sh
gingertea URL
gingertea /path/to/file
gingertea get URL -f mp4
gingertea get URL --audio -f mp3 -q 192
gingertea convert file.mov -f mp4
gingertea convert file.mp4 -f png --frames -q 1
gingertea convert file.mp4 -f mp3 --start 1:30 --end 2:00
gingertea probe file.mp4          # JSON metadata
gingertea sites youtube
gingertea update                  # update yt-dlp
```

Saved paths go to stdout; progress and errors go to stderr. Conversions preserve
original files. Completed jobs publish their staged output with numbered names
on collisions. Failed and cancelled jobs remove their staged files.

## Terminal controls

Enter loads a URL, search, or dropped paths. Quoted paths and file URLs work.
Tab leaves the source input; Esc returns to it.

| Key | Action |
| --- | --- |
| `1` / `2`, Ctrl+T | Download / Convert files |
| `d` / `a` | Download selected videos / audio |
| `c` | Format, quality, clips, subtitles, SponsorBlock, and conversion extras |
| `e` | Choose playlist items |
| `p` / `x` | Pause waiting jobs / cancel first unfinished job |
| `f` / `r` | Browse files / remove a local file or history record |
| `h` | Open searchable history |
| `s` / `3` | Settings |
| `l` / `4` | Search supported sites |
| `t` | Theme picker |
| `v` / `o` | Load clipboard / open last saved result |
| `u` / `q` | Update yt-dlp / quit |

In playlists, arrows move, Space toggles, `a` selects all, `n` clears, and `i`
inverts. First N previews a selection and its duration.

In history, Enter opens, `d` reloads, and `y` copies the URL.

### Forms and themes

Tab/Shift+Tab moves focus; Enter/Space activates buttons. Arrows choose options;
Enter opens settings dropdowns. Text fields support caret movement, insertion,
and deletion. Ctrl+U clears text, Ctrl+S saves or runs, and Esc cancels.

In the theme picker, arrows or Tab preview, Enter/Space or a click saves, and Esc
restores the previous theme.

### Settings and motion

Settings includes the output directory, filename template, browser cookies
for login-required videos, parallelism of 1–4 jobs, and animations. Browser cookies
are off by default. Configuration, history, and archives reuse the existing
platform directories.

Animations run during launch, paste, lookup, and active jobs, then stop. Disable
them in Settings or launch with `GINGERTEA_NO_ANIMATION=1`. Progress still updates.

## Media tools

Run `gingertea setup` to find installed tools or download missing ones into the
app's data directory. Setup needs curl and unzip, or PowerShell on Windows.

| Tool | Used for |
| --- | --- |
| yt-dlp | Downloads and site extraction |
| FFmpeg / FFprobe | Conversion and media metadata |
| Deno | JavaScript support for extraction |
| `cjxl` / `djxl` | Optional JPEG XL conversion |
| macOS `sips`, or `heif-enc` / `heif-convert` | Optional HEIC conversion |

Install optional image tools separately. Other image formats require their
corresponding FFmpeg encoders. Image metadata and output can differ from older
Pillow-based releases.

On macOS, you can install the main tools yourself:

```sh
brew install yt-dlp ffmpeg deno
```

The app invokes media tools as separate executables. Standalone yt-dlp bundles its
own Python interpreter. Use your package manager to update package-managed tools.
