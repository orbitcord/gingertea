"""Generate immutable FFmpeg planning expectations from the Python baseline.
No Python modules are imported by the Rust executable at runtime.
"""
from dataclasses import asdict
import json
from pathlib import Path
from gingertea.engine.local import parse_probe, build_ffmpeg_args, VIDEO_TARGETS, AUDIO_TARGETS, IMAGE_TARGETS
from gingertea.engine.options import Choice

raw = {'format': {'format_name': 'mov,mp4', 'duration': '10'}, 'streams': [
    {'codec_type': 'video', 'codec_name': 'h264', 'width': 1920, 'height': 1080},
    {'codec_type': 'audio', 'codec_name': 'aac', 'channels': 2}]}
cases = []
for kind, targets in [('video', VIDEO_TARGETS), ('audio', AUDIO_TARGETS), ('image', IMAGE_TARGETS)]:
    for fmt in targets:
        if fmt in ('jxl', 'heic'):
            continue
        for quality in ['best', '360' if kind == 'video' else '192' if kind == 'audio' else '50']:
            choice = Choice(kind, fmt, quality)
            args = build_ffmpeg_args(parse_probe(raw, Path('input.mp4')), choice, Path('out'), lambda _: True)
            value = asdict(choice)
            value['languages'] = ','.join(value['languages'])
            value['chapters'] = value.pop('split_chapters')
            value['strip_exif'] = value.pop('strip_metadata')
            cases.append({'raw': raw, 'choice': value, 'expected': args})
Path('rust/planner_cases.json').write_text('[\n' + ',\n'.join(json.dumps(c) for c in cases) + '\n]\n')
print(len(cases), 'reference cases')
