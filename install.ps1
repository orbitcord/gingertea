# gingertea installer for Windows.
#
#   powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/orbitcord/gingertea/main/install.ps1 | iex"
#
# Installs uv if it's missing, then installs gingertea with its own Python, then downloads
# ffmpeg. Set GINGERTEA_NO_FFMPEG=1 to skip that (gingertea gets it on first use).
$ErrorActionPreference = "Stop"
$source = if ($env:GINGERTEA_SOURCE) { $env:GINGERTEA_SOURCE } else { "gingertea" }

Write-Host "  gingertea installer" -ForegroundColor DarkYellow

if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    Write-Host "-> Installing uv (Python package manager)..."
    powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
    $env:Path = "$env:USERPROFILE\.local\bin;$env:Path"
}

Write-Host "-> Installing gingertea..."
uv tool install --python 3.12 --upgrade $source
uv tool update-shell | Out-Null

if (-not $env:GINGERTEA_NO_FFMPEG) {
    Write-Host "-> Getting ffmpeg..."
    $bin = (uv tool dir --bin)
    try {
        & (Join-Path $bin "gingertea.exe") setup
        if ($LASTEXITCODE -ne 0) { throw "setup failed" }
    } catch {
        Write-Host "  (gingertea will fetch ffmpeg the first time it needs it.)"
    }
}

Write-Host "Done. Open a new terminal and run:  gingertea" -ForegroundColor Green
