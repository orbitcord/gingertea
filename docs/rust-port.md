# Rust migration

Branch: codex/rust-port. The existing Python implementation remains as a baseline;
the new executable has no dependency on gingertea's Python modules. yt-dlp remains
an independently installed external backend, with FFmpeg/FFprobe and Deno.

Implementation sequence:
1. Capture the original application's startup, idle CPU/RSS and CLI latency.
2. Port settings/history, format tables, media probing and conversion planning.
3. Port download options to yt-dlp's public command interface, with progress,
   cancellation, archive and collision-safe publication.
4. Build the terminal interface: downloads/search/playlists, local files, format
   options, queue controls, history, settings, supported sites and themes.
5. Validate with Rust tests, real local media conversions, terminal interaction,
   and a reproducible before/after benchmark. Record any parity gaps explicitly.

Measurements must distinguish the UI process from external media workers. Never
compare a Rust UI-only measurement with a Python process including yt-dlp imports.

## Verification notes

- The initial benchmark timeout was caused by an obsolete readiness string, not
  application performance. `benchmarks/bench.py` now waits for the current empty
  queue text. Both implementations use that readiness milestone.
- The terminal acceptance harness reconstructs cursor-addressed screen updates;
  stripping ANSI alone cannot verify messages rendered as cell differences.
- Compiler errors were isolated to string replacement argument types and a form
  borrow spanning a mutation; corrected types and an owned form snapshot resolved
  them without suppressions. Clippy's reported style fixes were applied.
- Subprocesses use separate process groups on Unix. Cancellation kills the entire
  media worker group, and a test verifies prompt return with a descendant process.
  Probe commands have a 30-second limit. Application exit kills registered workers.
- Filename publication uses exclusive hard links for files so parallel jobs cannot
  overwrite each other. Staging is on the destination filesystem. Playlist archives
  are committed after publication; repeated single downloads remain allowed.
- The Rust FFmpeg planner is checked against 98 Python-generated reference cases.
  Image cases whose encoder is absent on the host are explicitly excluded, and
  audio fallback arguments are adjusted to the host's installed encoders.

## Compatibility and remaining release validation

The interface is implemented in Ratatui, retaining the original Textual design
through cell-based rendering, original branding assets and tea palette values. Clipboard operations use platform commands. JPEG XL and HEIC
use optional native tools rather than Pillow, so metadata and encoded output are
not guaranteed byte-for-byte equivalent. The Python environment is retained only
for baseline measurements and reference tests.

Risks: live authenticated downloads, subtitles/SponsorBlock behavior against real
sites, every optional media codec, and Windows/Linux runtime behavior have not
been exhaustively tested locally. CI provides a three-platform build/test matrix.
The external yt-dlp updater uses its own update mechanism; package-manager-owned
yt-dlp installations may require updating with that package manager. The Python
app's PyPI self-updater is replaced by Rust installation/release artifacts; no
release was published and no system application installation was changed.

## Measured results — November 13, 2026

Five-run medians on this Apple Silicon Mac, macOS 15.8.3, terminal 120×46,
release Rust build, warm filesystem/Python bytecode. Identical generated 3-second
640×360 H.264/AAC inputs and FFmpeg 8.1.2. Conversion trials alternate Python/Rust
order. The output codec and duration are verified with FFprobe.

| Measurement | Python | Rust | Observed difference |
| --- | ---: | ---: | ---: |
| First usable screen | 1,146 ms | 17.8 ms | 64× faster in this run |
| Idle UI RSS | 43.89 MiB | 4.44 MiB | 89.9% less |
| Idle CPU, one core | 0.986% | 0.00% measured | Below sampling resolution for Rust |
| Idle terminal output | 524 bytes/s | 0 bytes/s | No idle redraws |
| MP4 → MKV, stream copy | 398 ms | 256 ms | 35.6% less wall time |
| MP4 → MP3, 192 kbps | 448 ms | 357 ms | 20.2% less wall time |
| MP4 → WebM, VP9 | 5.018 s | 5.112 s | Rust 1.9% slower; effectively similar |
| `--version` | 191 ms | 16.2 ms | Different metadata work; indicative only |

Background load was present during the final run. An earlier comparison before
sustained load measured 213 ms versus 9 ms startup, 45.1 versus 4.2 MiB idle RSS,
and 1.064 versus 1.040 seconds for WebM encoding. Absolute timings therefore vary
substantially with scheduling/load; the 64× startup ratio is an observation for
this run, not a general performance guarantee. The consistent findings are lower
UI overhead and essentially unchanged encoder throughput.

Idle CPU uses `ps` CPU-time counters over three seconds; 0.00% means no additional
CPU time was visible at that counter's resolution, not literally zero work.
RSS/CPU measurements cover only the idle UI process, before yt-dlp runs. They do
not represent total memory during downloads/conversions. The Rust executable is
2.13 MiB; external media tools and their runtimes are excluded from that size.
Network throughput was not benchmarked; localhost downloads are functional checks.

Full samples and the measured executable checksum are in
`benchmarks/results/comparison.json`. The original pre-port baseline is also
saved separately in `benchmarks/results/python-baseline.json`.

Validation completed locally: 15 Rust tests (including the 98-case planner parity
check), formatting, warning-free Clippy, release build, original Python suite
(219 passed, 4 skipped), and offline acceptance covering real yt-dlp downloads,
playlist folders/archive skipping, safe repeated publication, file/format forms,
queue completion, history, settings, themes, file browsing, and JPEG XL round-trip.

Run the built application with `./target/release/gingertea`.

## Event-driven motion

The chosen character-resolve wordmark runs for 700 ms on launch, without delaying
input or moving the navigation. Terminal-safe glyphs resolve into a fixed nine-cell
wordmark. Pasting a URL or file path acknowledges it with a 220 ms border accent;
metadata/file loading has a compact spinner; ready content gets a 240 ms border
settle. Active jobs have a spinner plus a ten-cell progress bar (actual percentage
when known, a traveling segment otherwise). Completion/error gets a 280 ms queue
border accent, with a persistent success/error icon afterward.

Animation frames are scheduled only during these events: up to 20 Hz for short
feedback and 10 Hz for active work. One final frame clears the animation, after
which the normal dirty-only rendering resumes. No continuous decorative idle loop
or extra animation dependency is used. `animations = false` in Settings or
`GINGERTEA_NO_ANIMATION=1` disables all timed effects while retaining progress/status
updates. Existing settings files default to motion enabled.

Validation: deterministic elapsed-time tests cover wordmark width/final state,
finite feedback, disabled scheduling and progress parsing. The terminal acceptance
script `benchmarks/motion_check.py` uses an explicitly timed external backend stub
to exercise actual bracketed paste, loading/progress frames, queue completion,
Settings persistence and both ways to disable motion. Post-queue measurements live
in `benchmarks/results/motion.json`.

Design decision: the requested paste acknowledgment is feedback on acceptance;
ordinary typing, navigation, focus and shortcut dispatch remain immediate. No
transition delays a command or worker. Initial compiler diagnostics came from two
commas left inside newly nested message-handler matches; removing those commas
resolved the parse errors without suppressions.

Motion acceptance results on this machine: after a completed queue, 0 bytes/s of
terminal output and no additional CPU time visible in `ps` over three seconds;
resident memory was 4.83 MiB after loading details and completing the test job.
All eight terminal acceptance checks passed, including motion disabled at startup
through the saved setting and environment override. Rust checks: 19 tests passed,
formatting and warning-free Clippy, plus the release build.

Test-harness corrections: the timed backend fixture uses `/usr/bin/env python3`
because an executable interpreter path with spaces cannot be used directly in a
Unix shebang. The motion-off check drains the remaining initial paint before its
idle sample: readiness text can arrive before the footer bytes from that same
paint. A separate reproduction confirmed those bytes were initial drawing, with
zero output afterward. No product behavior was relaxed to satisfy these checks.

## Original design restored

The initial Rust port changed the presentation unnecessarily. The current build
restores the original visual hierarchy instead of the compact boxed replacement:
large centered carved wordmark, version/path status, Download/Convert files tabs,
rounded source field, open Video/Downloads rules, unframed thumbnail and aligned
metadata, solid action buttons, three-row download entries, and centered clickable
shortcut chips. Original theme values and terminal glyphs are copied directly from
the retained Python source into `rust/branding.json`, without a Python runtime
requirement. Short terminals hide the banner, as in the original app.

Settings uses the original human-facing folder/cookies/parallelism/filename form,
with a single additional animation toggle. Convert restores the Format, Quality,
Clip and Extras panels. History, supported sites, playlist selection and themes
are modal pickers; the source workspace stays behind them. Playlist selection
includes First N. Source tabs, footer chips, action buttons, fields and choices
are mouse-operable. Ctrl+T switches source tabs; Tab leaves the focused source
field so letter shortcuts work. The wordmark's character resolve animates the
original glyph silhouette, keeping its footprint fixed, and job/paste feedback
remains within the original surfaces.

At 120×46 the original Python and restored Rust empty-workspace captures match
on all 45 rows other than the version/status line. That line intentionally shows
the Rust release version. The comparison checks full rows, not just the presence
of the application name, and exercises the original-style tabs, settings, theme,
history and sites dialogs. Reference captures and results are saved under
`benchmarks/results/*design*` and `design-parity.json`. This verifies terminal
text/layout; theme colors and logo shading are separately ported from source.

The reference-test PTY cleanup now releases the master before waiting and uses a
bounded termination wait, preventing shutdown output from blocking an unattended
reference app. Escape-key test events are separated so they are not interpreted
as Alt-key chords. These harness corrections do not relax the design assertions.

Restoration validation completed: 23 Rust tests, warning-free Clippy, release
build, the 45-row reference comparison, mouse/dialog acceptance, motion checks,
and real-media smoke checks (downloads, playlists, safe publication, local
conversion, history/settings, browser and JPEG XL). After work and transient
messages settled, the restored UI emitted 0 bytes/s with no measurable idle CPU
increase over three seconds; RSS in that test was 5.5 MiB. The corrected release
build was opened in a fresh local Terminal window.

## Theme picker selection fix

Full-size arrow/Enter and mouse confirmation reproduced successfully in isolation.
A compact window exposed a real issue: the picker truncated its rows without
scrolling, so the saved/selected theme could be invisible. The picker now uses a
scrolling Ratatui list with persistent viewport offset, an explicit selection
marker and live palette preview. Arrows, Tab/Shift+Tab, Home/End and j/k navigate;
Enter/Space or clicking saves. Escape restores the palette from before the picker
opened. Hover previews without writing settings. Save errors stay in the picker
instead of propagating out of the application. Repeated key events are accepted.

`benchmarks/theme_check.py` verifies selected-row visibility, no premature save,
Escape rollback, Space confirmation, mouse confirmation and Tab/Enter confirmation
at 120×46 and 120×12. All 12 checks pass; results are in
`benchmarks/results/themes.json`. Rust tests, formatting and warning-free Clippy
also pass. Risks: the native Terminal window cannot be inspected through computer
use in this environment; interaction verification uses controlled PTY sessions.
