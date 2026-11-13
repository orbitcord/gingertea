"""Same-machine Python/Rust comparison. Run after cargo build --release.

Uses isolated config/data directories; never downloads internet media. RSS and CPU
are for the idle UI process only. Conversion time includes external FFmpeg work.
"""
import argparse
import hashlib
import fcntl
import json
import os
from pathlib import Path
import platform
import pty
import re
import select
import signal
import statistics
import struct
import subprocess
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parents[1]
PYTHON = [str(ROOT / '.venv/bin/python'), '-m', 'gingertea']
RUST = [str(ROOT / 'target/release/gingertea')]
ANSI = re.compile(rb'\x1b\[[0-?]*[ -/]*[@-~]')


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
        # Release the PTY before waiting: reference TUIs can block writing shutdown
        # output if the master is no longer drained during another app's tests.
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


def median(values):
    return round(statistics.median(values), 4)


def measure(command, env, runs):
    starts, memory, cpu, output = [], [], [], []
    for _ in range(runs):
        app = Terminal(command, env)
        try:
            starts.append(app.wait('No downloads yet.'))
            app.settle(1)
            before, rss = process_stats(app.pid)
            began = time.perf_counter()
            count = app.settle(3)
            after, rss = process_stats(app.pid)
            elapsed = time.perf_counter() - began
            memory.append(rss)
            cpu.append((after - before) / elapsed * 100)
            output.append(count / elapsed)
        finally:
            app.close()
    versions = []
    for _ in range(runs * 3):
        start = time.perf_counter()
        subprocess.run(command + ['--version'], env=env, capture_output=True, check=True)
        versions.append(time.perf_counter() - start)
    return {'first_frame_ms': median(starts) * 1000, 'idle_rss_mib': median(memory),
            'idle_cpu_percent_one_core': median(cpu), 'idle_terminal_bytes_per_s': median(output),
            'version_ms': median(versions) * 1000, 'startup_samples_ms': [round(t * 1000, 2) for t in starts]}


def conversions(command, env, folder, runs):
    results = {}
    for fmt, quality in [('mkv', 'best'), ('webm', '360'), ('mp3', '192')]:
        samples = []
        for i in range(runs):
            # Unique input names avoid output collisions; identical bytes in each trial.
            src = folder / f'input-{fmt}-{i}.mp4'
            src.write_bytes((folder / 'fixture.mp4').read_bytes())
            start = time.perf_counter()
            out = subprocess.run(command + ['convert', str(src), '-f', fmt, '-q', quality],
                                 env=env, capture_output=True, text=True, check=True)
            samples.append(time.perf_counter() - start)
            saved = Path(out.stdout.strip().splitlines()[-1])
            assert saved.is_file() and saved.stat().st_size > 0, out
            probe = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_streams',
                                                        '-show_format', '-of', 'json', str(saved)]))
            assert float(probe['format']['duration']) >= 2.9
            if fmt == 'webm':
                assert probe['streams'][0]['codec_name'] == 'vp9'
            if fmt == 'mp3':
                assert probe['streams'][0]['codec_name'] == 'mp3'
            if fmt == 'mkv':
                assert probe['streams'][0]['codec_name'] == 'h264'
            saved.unlink()
            src.unlink()
        results[f'convert_{fmt}_s'] = median(samples)
        results[f'convert_{fmt}_samples_s'] = samples
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--out', default='benchmarks/results/comparison.json')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='gingertea-comparison-') as temp:
        folder = Path(temp)
        env = {**os.environ, 'HOME': temp, 'XDG_CONFIG_HOME': str(folder / 'config'),
               'XDG_DATA_HOME': str(folder / 'data'), 'GINGERTEA_CONFIG_DIR': str(folder / 'rust-config'),
               'GINGERTEA_DATA_DIR': str(folder / 'rust-data'), 'GINGERTEA_NO_UPDATE_CHECK': '1',
               'GINGERTEA_NO_AUTO_UPDATE': '1', 'GINGERTEA_NO_FFMPEG_FETCH': '1',
               'TERM': 'xterm-256color', 'COLORTERM': 'truecolor'}
        subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=640x360:rate=24',
                        '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=44100', '-t', '3',
                        '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest',
                        str(folder / 'fixture.mp4')], check=True)
        result = {'platform': platform.platform(), 'machine': platform.machine(), 'runs': args.runs,
                  'terminal': '120x46', 'mode': 'warm filesystem and bytecode; release Rust build',
                  'python': {}, 'rust': {}}
        # Warm both versions before timing and alternate conversion order between groups.
        for cmd in [PYTHON, RUST]:
            subprocess.run(cmd + ['--version'], env=env, capture_output=True, check=True)
        for name, cmd in [('python', PYTHON), ('rust', RUST)]:
            result[name].update(measure(cmd, env, args.runs))
            print(name, result[name], flush=True)
        samples = {'python': {}, 'rust': {}}
        for trial in range(args.runs):
            order = [('python', PYTHON), ('rust', RUST)]
            if trial % 2:
                order.reverse()
            for name, cmd in order:
                measured = conversions(cmd, env, folder, 1)
                for key, value in measured.items():
                    if key.endswith('_samples_s'):
                        samples[name].setdefault(key, []).extend(value)
        for name in samples:
            for key, values in samples[name].items():
                result[name][key] = values
                result[name][key.replace('_samples_s', '_s')] = median(values)
            print(name, 'conversions', samples[name], flush=True)
        result['rust_binary_mib'] = (ROOT / 'target/release/gingertea').stat().st_size / 2**20
        result['rust_executable_sha256'] = hashlib.sha256((ROOT / 'target/release/gingertea').read_bytes()).hexdigest()
        result['python_baseline_commit'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        result['ffmpeg'] = subprocess.check_output(['ffmpeg', '-version'], text=True).splitlines()[0]
        Path(args.out).write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
