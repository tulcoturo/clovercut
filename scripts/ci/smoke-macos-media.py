#!/usr/bin/env python3
"""Exercise the source-built media prefix before building the desktop application."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

prefix = Path(sys.argv[1]).resolve()
ffmpeg = str(prefix / 'bin/ffmpeg')
ffprobe = str(prefix / 'bin/ffprobe')


def encode(*args):
    subprocess.run([ffmpeg, '-nostdin', '-v', 'error', '-y', '-threads', '2',
                    '-filter_threads', '1', *map(str, args)], check=True, timeout=120)


def probe(path):
    return json.loads(subprocess.check_output(
        [ffprobe, '-v', 'error', '-show_streams', '-show_format', '-of', 'json', str(path)],
        timeout=30))


filters = subprocess.check_output([ffmpeg, '-hide_banner', '-filters'], text=True, stderr=subprocess.STDOUT)
for name in ('rubberband', 'asendcmd', 'asetnsamples'):
    assert any(name in line.split()[:2] for line in filters.splitlines()), f'Missing {name}'

with tempfile.TemporaryDirectory(prefix='clovercut-media-smoke-') as folder:
    root = Path(folder)
    for name, codec, pixel, options in (
        ('h264.mp4', 'libx264', 'yuv420p', ['-preset', 'ultrafast']),
        ('hevc8.mp4', 'libx265', 'yuv420p', ['-preset', 'ultrafast', '-x265-params', 'pools=none:frame-threads=1:log-level=error']),
        ('hevc10.mp4', 'libx265', 'yuv420p10le', ['-preset', 'ultrafast', '-x265-params', 'pools=none:frame-threads=1:log-level=error']),
        ('aom.mkv', 'libaom-av1', 'yuv420p', ['-cpu-used', '8']),
        ('svt.mkv', 'libsvtav1', 'yuv420p', ['-preset', '12', '-svtav1-params', 'lp=2']),
    ):
        output = root / name
        encode('-f', 'lavfi', '-i', 'testsrc2=size=64x64:rate=25', '-frames:v', '2',
               '-an', '-c:v', codec, '-pix_fmt', pixel, *options, output)
        stream = probe(output)['streams'][0]
        expected = 'h264' if codec == 'libx264' else 'hevc' if codec == 'libx265' else 'av1'
        assert (stream['codec_name'], stream['pix_fmt']) == (expected, pixel), stream
        # Decode too: AV1 needs the source-built decoder chain, not just its encoder.
        encode('-i', output, '-frames:v', '2', '-f', 'null', '-')
        print(f'Media smoke: {codec} / {pixel} encoded and decoded', flush=True)

    alpha = root / 'alpha.webm'
    encode('-f', 'lavfi', '-i', 'color=red@0.5:size=64x64:rate=25,format=yuva420p',
           '-frames:v', '2', '-an', '-c:v', 'libvpx-vp9', '-pix_fmt', 'yuva420p',
           '-auto-alt-ref', '0', '-deadline', 'realtime', alpha)
    plane = root / 'alpha.raw'
    encode('-c:v', 'libvpx-vp9', '-i', alpha, '-vf', 'alphaextract', '-frames:v', '1',
           '-f', 'rawvideo', '-pix_fmt', 'gray', plane)
    values = plane.read_bytes()
    assert len(values) == 64 * 64 and all(110 < value < 145 for value in values), 'VP9 alpha lost'
    print('Media smoke: VP9 transparency survived encoding and decoding', flush=True)

    prores = root / 'alpha.mov'
    encode('-f', 'lavfi', '-i', 'color=red@0.5:size=64x64:rate=25,format=yuva444p10le',
           '-frames:v', '2', '-an', '-c:v', 'prores_ks', '-profile:v', '4',
           '-pix_fmt', 'yuva444p10le', prores)
    encode('-i', prores, '-vf', 'alphaextract', '-frames:v', '1',
           '-f', 'rawvideo', '-pix_fmt', 'gray', plane)
    values = plane.read_bytes()
    assert len(values) == 64 * 64 and all(110 < value < 145 for value in values), 'ProRes alpha lost'
    print('Media smoke: ProRes 4444 transparency survived encoding and decoding', flush=True)

    for codec, suffix in (('aac', 'm4a'), ('libopus', 'ogg')):
        output = root / ('sound.' + suffix)
        encode('-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=0.3',
               '-c:a', codec, output)
        assert probe(output)['streams'][0]['codec_name'] == ('opus' if codec == 'libopus' else codec)
    stretched = root / 'tempo.wav'
    encode('-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=0.6',
           '-af', 'rubberband=tempo=1.5', '-c:a', 'pcm_s16le', stretched)
    duration = float(probe(stretched)['format']['duration'])
    assert 0.35 < duration < 0.45, f'Unexpected Rubber Band duration: {duration}'
    print('Media smoke: AAC, Opus and Rubber Band time stretching passed', flush=True)
