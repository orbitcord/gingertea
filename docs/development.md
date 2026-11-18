# Development

[Back to README](../README.md) · [Usage reference](usage.md) · [Historical migration report](rust-port.md)

## Build and check

Requires Rust 1.88+, FFmpeg, and FFprobe on PATH.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
./target/release/gingertea
```

The application lives in `rust/`. `tests/rust_e2e.rs` checks real media conversions
and CLI behavior. The saved cases in `rust/planner_cases.json` verify compatibility
with the original conversion planner without running the old Python app.

## Terminal acceptance checks

These scripts test the Rust executable using Python 3.11+ and its standard
library. They require a Unix PTY and a release build. Media checks also need
FFmpeg/FFprobe.

```sh
python3 benchmarks/layout_check.py
python3 benchmarks/theme_check.py
python3 benchmarks/motion_check.py
python3 benchmarks/ui_controls_check.py
python3 benchmarks/playlist_check.py
python3 benchmarks/thumbnail_check.py
python3 benchmarks/smoke.py
```

The smoke check also needs yt-dlp on PATH, or `GINGERTEA_YT_DLP` set to its
executable, plus `cjxl` and `djxl`. Downloads use a localhost fixture server.
Scripts write their results under `benchmarks/results/`.

## Packaging

`pyproject.toml` packages the Rust binary with Maturin for uv and pipx installs.
Its dependencies supply yt-dlp and Deno. The release workflow builds native
wheels, standalone binaries, and a source archive.

CI runs Rust checks and real-media tests on Linux, Windows, and both Mac
architectures. `scripts/check_wheel.py` checks an isolated upgrade from the
published Python 0.2.0 package to the native wheel. The old app and its Python
reference suite are no longer part of this checkout.

Saved migration measurements remain in `benchmarks/results/`. The scripts that
required the old app have been removed; use Git history to reproduce those
comparisons.
