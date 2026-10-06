#!/usr/bin/env python3
"""Record the actual guest display/audio with bounded temporary disk usage.

Requires ffmpeg. Run after building the project disc and creating video-tape.csv.
PPM files are consumed and deleted as the emulator writes them; gameplay is not
altered. The route log records timing so the final encode can preserve real speed.
"""
import argparse
import csv
import json
import subprocess
import tempfile
import time
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[2]
ROOT = PROJECT.parents[2]
OUT = PROJECT / 'validation/enemy-behavior/video-review'
CPU_HZ = 33_868_800
FIRST_FRAME = 450
FPS = CPU_HZ / 571_243


def main():
    global OUT
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=OUT)
    parser.add_argument('--disc', type=Path, default=PROJECT/'baked/graybox_reach.cue')
    args = parser.parse_args()
    OUT = args.output.resolve()
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='cortex-enemy-video-') as work:
        shots = Path(work)
        with (OUT/'capture.log').open('w') as log, (OUT/'encode-native.log').open('w') as encode_log:
            encoder = subprocess.Popen([
                'ffmpeg', '-y', '-hide_banner', '-f', 'image2pipe', '-vcodec', 'ppm',
                '-framerate', str(FPS), '-i', 'pipe:0', '-an', '-c:v', 'libx264',
                '-preset', 'veryfast', '-crf', '0', '-pix_fmt', 'yuv420p',
                str(OUT/'continuous-native.mp4')], stdin=subprocess.PIPE,
                stdout=encode_log, stderr=subprocess.STDOUT)
            capture = subprocess.Popen([
                str(ROOT/'target/release/frontend'), 'launch', '--config-dir',
                str(PROJECT/'validation/enemy-behavior/emulator-config'), '--path',
                str(args.disc.resolve()), '--embedded-playtest',
                '--input-tape', str(OUT/'video-tape.csv'), '--stop-at-poll', '4860',
                '--steps', '10000000000', '--route-screenshot-dir', str(shots),
                '--route-screenshot-interval', '1', '--route-log', str(OUT/'video-route.csv'),
                '--dump-audio', str(OUT/'continuous-audio.wav'), '--dump-hash'],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
            frame = 1
            try:
                while True:
                    path = shots/f'tick-{frame:06}.ppm'
                    if path.exists():
                        data = path.read_bytes()
                        parts = data.split(b'\n', 3)
                        if len(parts) == 4:
                            width, height = map(int, parts[1].split())
                            if len(parts[3]) == width*height*3:
                                if frame >= FIRST_FRAME:
                                    assert (width, height) == (320, 240)
                                    encoder.stdin.write(data)
                                path.unlink()
                                frame += 1
                                continue
                    if capture.poll() is not None:
                        if capture.returncode:
                            raise RuntimeError(f'Capture failed; see {OUT / "capture.log"}')
                        break
                    time.sleep(.01)
            finally:
                if capture.poll() is None:
                    capture.terminate()
                    capture.wait()
                encoder.stdin.close()
                encoder.wait()
            assert encoder.returncode == 0
        rows = list(csv.DictReader((OUT/'video-route.csv').open()))
        start = next(r for r in rows if int(r['route_tick']) == FIRST_FRAME)
        end = next(r for r in rows if int(r['route_tick']) == frame-1)
        frames = frame - FIRST_FRAME
        actual_fps = (frames-1)*CPU_HZ/(int(end['bus_cycles'])-int(start['bus_cycles']))
        report = {'frames': frames, 'first_route_tick': FIRST_FRAME,
                  'last_route_tick': frame-1, 'encoded_fps': FPS, 'actual_fps': actual_fps,
                  'audio_start_seconds': int(start['bus_cycles'])/CPU_HZ,
                  'chapters': []}
        for title, poll in [('Engage and flank',360), ('Disengage and return home',1980), ('Blocked-path recovery',3420)]:
            row = next(r for r in rows if int(r['port1_polls']) >= poll)
            report['chapters'].append({'title':title,'poll':poll,
                'seconds':max(0,(int(row['bus_cycles'])-int(start['bus_cycles']))/CPU_HZ)})
        (OUT/'recording.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report,indent=2))

if __name__ == '__main__':
    main()
