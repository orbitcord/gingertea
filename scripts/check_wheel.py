"""Upgrade an isolated uv tool from Python 0.2.0 to the native wheel and exercise it."""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def run(args, env):
    result = subprocess.run(args, env=env, capture_output=True, encoding="utf-8", errors="replace")
    if result.returncode:
        raise RuntimeError(f"{args!r}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset", required=True)
    args = parser.parse_args()
    wheels = list(Path("dist").glob("gingertea-0.3.0-*.whl"))
    assert len(wheels) == 1, wheels
    wheel = wheels[0].resolve()
    suffix = ".exe" if os.name == "nt" else ""
    with tempfile.TemporaryDirectory(prefix="gingertea-wheel-") as temp:
        root = Path(temp)
        env = {
            **os.environ,
            "UV_TOOL_DIR": str(root / "tools"),
            "UV_TOOL_BIN_DIR": str(root / "bin"),
            "GINGERTEA_CONFIG_DIR": str(root / "config"),
            "GINGERTEA_DATA_DIR": str(root / "data"),
        }
        command = root / "bin" / f"gingertea{suffix}"
        run(["uv", "tool", "install", "--python", sys.executable, "gingertea==0.2.0"], env)
        assert "gingertea 0.2.0" in run([str(command), "--version"], env)
        run(["uv", "tool", "install", "--force", "--python", sys.executable, str(wheel)], env)
        assert run([str(command), "--version"], env) == "gingertea 0.3.0"
        assert "youtube" in run([str(command), "sites", "youtube"], env).lower()
        setup = run([str(command), "setup"], env)
        for tool in ("yt-dlp", "ffmpeg", "ffprobe", "deno"):
            assert f"{tool}:" in setup or f"Installed {tool}" in setup, setup
        asset = Path("assets") / args.asset
        asset.parent.mkdir(exist_ok=True)
        shutil.copyfile(command.resolve(), asset)
        asset.chmod(0o755)
        print(f"PASS: Python-to-Rust upgrade, CLI, media tools, and native asset ({asset.name})")


if __name__ == "__main__":
    main()
