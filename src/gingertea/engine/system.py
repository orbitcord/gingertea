"""Clipboard, opening a file, and a notification when the terminal is in the background."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def read_clipboard() -> str:
    commands = _read_commands()
    for command in commands:
        try:
            out = subprocess.run(command, capture_output=True, text=True, timeout=1, check=False)
        except (OSError, subprocess.TimeoutExpired):
            continue
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout
    return ""


def write_clipboard(text: str) -> bool:
    command, use_stdin = _write_command()
    if command is None:
        return False
    try:
        subprocess.run(
            command, input=text if use_stdin else None, text=True, timeout=1, check=False
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return True


def notify(title: str, body: str) -> None:
    """A system notification. Failure is quiet: the queue already shows the result."""
    try:
        if sys.platform == "darwin":
            script = f'display notification "{_apple(body)}" with title "{_apple(title)}"'
            subprocess.Popen(["osascript", "-e", script])
        elif sys.platform == "win32":
            safe_title, safe_body = title.replace("'", "''"), body.replace("'", "''")
            subprocess.Popen(
                [
                    "powershell",
                    "-NoProfile",
                    "-Command",
                    "Add-Type -AssemblyName System.Windows.Forms; "
                    "$n = New-Object System.Windows.Forms.NotifyIcon; "
                    "$n.Icon = [System.Drawing.SystemIcons]::Information; "
                    "$n.Visible = $true; "
                    f"$n.ShowBalloonTip(5000, '{safe_title}', '{safe_body}', 'Info')",
                ]
            )
        else:
            subprocess.Popen(["notify-send", title, body])
    except OSError:
        return


def reveal(path: str) -> None:
    """Show a file in Finder, Explorer, or the default file manager."""
    try:
        if sys.platform == "darwin":
            subprocess.Popen(["open", "-R", path])
        elif sys.platform == "win32":
            subprocess.Popen(["explorer", "/select,", path])
        else:
            subprocess.Popen(["xdg-open", str(Path(path).parent)])
    except OSError:
        return


def open_path(path: str) -> None:
    """Open a file or folder with the default app."""
    try:
        if sys.platform == "darwin":
            subprocess.Popen(["open", path])
        elif sys.platform == "win32":
            subprocess.Popen(["cmd", "/c", "start", "", path])
        else:
            subprocess.Popen(["xdg-open", path])
    except OSError:
        return


def _apple(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"')


def _read_commands() -> list[list[str]]:
    if sys.platform == "darwin":
        return [["pbpaste"]]
    if sys.platform == "win32":
        return [["powershell", "-NoProfile", "-Command", "Get-Clipboard"]]
    return [["wl-paste", "-n"], ["xclip", "-selection", "clipboard", "-o"]]


def _write_command() -> tuple[list[str] | None, bool]:
    if sys.platform == "darwin":
        return ["pbcopy"], True
    if sys.platform == "win32":
        return ["clip"], True
    return ["wl-copy"], True
