"""Shared Unix terminal driver for native-app acceptance checks."""

import fcntl
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import time

ROOT = Path(__file__).resolve().parents[1]
RUST = [str(ROOT / 'target/release/gingertea')]


def process_stats(pid):
    values = subprocess.check_output(['ps', '-o', 'cputime=,rss=', '-p', str(pid)], text=True).split()
    parts = [float(v) for v in values[0].split(':')]
    cpu = 0
    for part in parts:
        cpu = cpu * 60 + part
    return cpu, int(values[1]) / 1024


class Terminal:
    def __init__(self, command, env):
        self.start = time.perf_counter()
        pid, fd = pty.fork()
        if pid == 0:
            os.execvpe(command[0], command, env)
        self.pid, self.fd, self.buf = pid, fd, b''
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 46, 120, 0, 0))

    def drain(self, timeout=0.02):
        if select.select([self.fd], [], [], timeout)[0]:
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                return 0
            self.buf += data
            return len(data)
        return 0

    def screen(self):
        # Reconstruct cursor-addressed terminal diffs rather than concatenating writes.
        rows = [[' '] * 200 for _ in range(80)]
        row = col = 0
        tokens = re.split(r'(\x1b\[[0-?]*[ -/]*[@-~])', self.buf.decode(errors='replace'))
        for token in tokens:
            if token.startswith('\x1b['):
                match = re.match(r'\x1b\[(\d*);?(\d*)[Hf]$', token)
                if match:
                    row, col = max(0, int(match[1] or 1) - 1), max(0, int(match[2] or 1) - 1)
                elif token in ('\x1b[2J', '\x1b[3J'):
                    rows = [[' '] * 200 for _ in range(80)]
                continue
            for char in token:
                if char == '\r':
                    col = 0
                elif char == '\n':
                    row += 1
                elif char >= ' ':
                    if row < len(rows) and col < len(rows[0]):
                        rows[row][col] = char
                    col += 1
        return '\n'.join(''.join(row) for row in rows)

    def wait(self, text, timeout=20):
        deadline = time.perf_counter() + timeout
        while time.perf_counter() < deadline:
            self.drain(0.001)
            if text in self.screen():
                return time.perf_counter() - self.start
        raise RuntimeError(f'Terminal did not show {text!r}: {self.screen()}')

    def send(self, text):
        os.write(self.fd, text.encode())

    def settle(self, seconds):
        end = time.perf_counter() + seconds
        count = 0
        while time.perf_counter() < end:
            count += self.drain()
        return count

    def close(self):
        # Release the PTY before waiting so shutdown output cannot block the child.
        os.close(self.fd)
        try:
            os.kill(self.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            try:
                waited, _ = os.waitpid(self.pid, os.WNOHANG)
            except ChildProcessError:
                return
            if waited:
                return
            time.sleep(.01)
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.waitpid(self.pid, 0)
