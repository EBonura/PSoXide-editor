"""1080p side/three-quarter review of the claw slash at game speed."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess
P=Path(__file__).resolve().parents[3];ROOT=P.parents[2]/'build/graybox-reach/light-heavy-v1/frames/heavy';OUT=P/'validation/light-heavy-v1'
f='/System/Library/Fonts/Supplemental/Arial.ttf';font=ImageFont.truetype(f,26);title=ImageFont.truetype(f.replace('Arial.ttf','Arial Bold.ttf'),40)
cache={v:[Image.open(ROOT/v/f'{i:03}.png').convert('RGB') for i in range(1,68)] for v in ['threequarter','side']}
path=OUT/'heavy-aligned-claw-1080p.mp4';proc=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','rawvideo','-pixel_format','rgb24','-video_size','1920x1080','-framerate','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(path)],stdin=subprocess.PIPE)
for tick in range(330):
 im=Image.new('RGB',(1920,1080),(15,21,30));d=ImageDraw.Draw(im)
 d.text((40,24),'GRAYBOX REACH / HEAVY OVERHEAD CLAW',font=title,fill=(233,238,248))
 d.text((40,82),'Corrected claw attachment / unchanged animation',font=font,fill=(160,180,204))
 for v,x in [('threequarter',160),('side',1120)]:im.paste(cache[v][min(tick%110,66)],(x,140))
 phase=tick%110;active=32<=phase<=37
 d.text((790,980),'DAMAGE ACTIVE' if active else ('WIND-UP' if phase<32 else 'RECOVERY' if phase<66 else 'READY'),font=font,fill=(255,115,95) if active else (118,212,199))
 d.text((40,1027),'Actual game speed / 2.20-second heavy / pause between repeats',font=font,fill=(160,180,204))
 if tick==34:im.save(path.with_suffix('.png'))
 proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0;print(path)
