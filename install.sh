#!/bin/sh
# Build and install the Rust application from this checkout.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if ! command -v cargo >/dev/null 2>&1; then
  echo 'Rust is required. Install it from https://rustup.rs, then rerun this script.' >&2
  exit 1
fi
cargo install --locked --path "$ROOT"
if [ -z "${GINGERTEA_NO_FFMPEG:-}" ]; then
  "${CARGO_HOME:-$HOME/.cargo}/bin/gingertea" setup
fi
printf '\nInstalled. Run gingertea.\n'
