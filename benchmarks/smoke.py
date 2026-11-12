"""Offline acceptance: real yt-dlp over localhost plus the Rust terminal UI."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
from compare import Terminal, RUST, ROOT, ANSI


def checked(args, env):
    r = subprocess.run(RUST + args, env=env, capture_output=True, text=True, timeout=40)
    assert r.returncode == 0, r.stderr
    return r


def main():
    with tempfile.TemporaryDirectory(prefix='gingertea-smoke-') as temp:
        folder = Path(temp)
        env = {**os.environ, 'GINGERTEA_CONFIG_DIR': str(folder / 'config'),
               'GINGERTEA_DATA_DIR': str(folder / 'data'), 'TERM': 'xterm-256color',
               'GINGERTEA_YT_DLP': str(ROOT / '.venv/bin/yt-dlp')}
        fixture = folder / 'fixture.mp4'
        subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=320x240:rate=10',
                        '-t', '2', '-c:v', 'libx264', str(fixture)], check=True)
        server = ThreadingHTTPServer(('127.0.0.1', 0), partial(SimpleHTTPRequestHandler, directory=temp))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        url = f'http://127.0.0.1:{server.server_port}/fixture.mp4'
        try:
            outputs = []
            for _ in range(2):
                r = checked(['get', url, '--dir', str(folder / 'downloads')], env)
                p = Path(r.stdout.strip().splitlines()[-1])
                assert p.is_file() and p.stat().st_size > 0
                outputs.append(p)
            assert outputs[0] != outputs[1], 'Repeated single downloads must not overwrite'
            print('PASS: real yt-dlp download, publication, history, repeated download collision')
            (folder / 'fixture2.mp4').write_bytes(fixture.read_bytes())
            (folder / 'playlist.html').write_text('<html><head><title>Fixture playlist</title></head><body><video src="fixture.mp4" controls></video><video src="fixture2.mp4" controls></video></body></html>')
            playlist_url = f'http://127.0.0.1:{server.server_port}/playlist.html'
            playlist = checked(['get', playlist_url, '--dir', str(folder / 'playlists')], env)
            assert len(playlist.stdout.strip().splitlines()) == 2, playlist
            repeated = checked(['get', playlist_url, '--dir', str(folder / 'playlists')], env)
            assert not repeated.stdout.strip(), repeated
            assert (folder / 'data/archive.txt').is_file()
            print('PASS: playlist folder publication and repeat archive skipping')

            app = Terminal(RUST + [str(fixture)], env)
            try:
                app.wait('Files ready.')
                app.send('c')
                app.wait('Convert options')
                app.send('\x13')  # Ctrl+S runs the default MP4 conversion.
                app.wait('Queue finished')
                app.send('s')
                app.wait('Settings ·')
                app.send('\x13')
                app.wait('Settings saved')
                app.send('h')
                app.wait('History ·')
                app.send('t')
                app.send('f')
                app.wait('Enter Open')
                app.send('\x1b')
                app.settle(0.2)
                (ROOT / 'benchmarks/results/rust-terminal.txt').write_text('\n'.join(line.rstrip() for line in app.screen().splitlines()).rstrip() + '\n')
                assert (folder / 'config/config.toml').exists()
                assert (folder / 'data/history.jsonl').exists()
                print('PASS: file loading, options, conversion queue, settings persistence, history, theme, file browser')
            finally:
                app.close()
            # Native JPEG XL path: PNG through cjxl, then djxl back through FFmpeg.
            frame = folder / 'still.png'
            subprocess.run(['ffmpeg', '-v', 'error', '-i', str(fixture), '-frames:v', '1', str(frame)], check=True)
            jxl = Path(checked(['convert', str(frame), '-f', 'jxl'], env).stdout.strip())
            jpg = Path(checked(['convert', str(jxl), '-f', 'jpg'], env).stdout.strip())
            assert jxl.is_file() and jpg.is_file()
            print('PASS: native JPEG XL encode/decode (no Python image backend)')
        finally:
            server.shutdown()


if __name__ == '__main__':
    main()
