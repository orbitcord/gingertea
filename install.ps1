$ErrorActionPreference = "Stop"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Rust is required. Install it from https://rustup.rs, then rerun this script."
}
cargo install --locked --path $PSScriptRoot
if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
$cargoRoot = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME ".cargo" }
if (-not $env:GINGERTEA_NO_FFMPEG) {
    & (Join-Path $cargoRoot "bin/gingertea.exe") setup
    if ($LASTEXITCODE -ne 0) { throw "Media tool setup failed" }
}
Write-Host "Installed. Run gingertea."
