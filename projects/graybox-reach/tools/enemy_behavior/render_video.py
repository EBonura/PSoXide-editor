#!/usr/bin/env python3
"""Render a recorded encounter with audio and chapter banners outside the game image."""
import argparse
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--name', default='enemy-behaviour-review.mp4')
    args = parser.parse_args()
    p = args.directory.resolve()
    report = json.loads((p / 'recording.json').read_text())
    duration = report['frames'] / report['actual_fps']
    chapters = report['chapters']
    metadata = ';FFMETADATA1\ntitle=Graybox Reach - single enemy behaviour review\n'
    for i, chapter in enumerate(chapters):
        end = chapters[i + 1]['seconds'] if i + 1 < len(chapters) else duration
        metadata += (f"[CHAPTER]\nTIMEBASE=1/1000\nSTART={round(chapter['seconds'] * 1000)}\n"
                     f"END={round(end * 1000)}\ntitle={chapter['title']}\n")
    (p / 'chapters.txt').write_text(metadata)
    command = ['ffmpeg', '-y', '-hide_banner', '-i', str(p / 'continuous-native.mp4'),
               '-ss', str(report['audio_start_seconds']), '-i', str(p / 'continuous-audio.wav')]
    for i in range(1, 4):
        command += ['-i', str(p / f'chapter-{i}.png')]
    command += ['-i', str(p / 'chapters.txt')]
    ratio = report['encoded_fps'] / report['actual_fps']
    filters = f'[0:v]setpts=PTS*{ratio},scale=960:720:flags=neighbor,pad=960:792:0:72:black[v0];'
    for i, chapter in enumerate(chapters):
        end = chapters[i + 1]['seconds'] if i + 1 < len(chapters) else duration
        filters += (f"[v{i}][{i+2}:v]overlay=0:0:enable='between(t,{chapter['seconds']},{end})'[v{i+1}]"
                    + (';' if i < 2 else ''))
    command += ['-filter_complex', filters, '-map', '[v3]', '-map', '1:a:0',
                '-map_metadata', '5', '-map_chapters', '5', '-t', str(duration), '-r', '60',
                '-c:v', 'libx264', '-preset', 'medium', '-crf', '18', '-pix_fmt', 'yuv420p',
                '-c:a', 'aac', '-b:a', '160k', '-movflags', '+faststart', str(p / args.name)]
    (p / 'encode-command.json').write_text(json.dumps(command, indent=2) + '\n')
    with (p / 'encode-final.log').open('w') as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
    print(p / args.name)


if __name__ == '__main__':
    main()
