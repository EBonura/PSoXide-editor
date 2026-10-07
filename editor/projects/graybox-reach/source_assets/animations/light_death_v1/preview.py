"""1080p side/three-quarter review of the death animation at game speed."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess
P=Path(__file__).resolve().parents[3];ROOT=P.parents[2]/'build/graybox-reach/light-death-v1/frames/death';OUT=P/'validation/light-death-v1'
f='/System/Library/Fonts/Supplemental/Arial.ttf';font=ImageFont.truetype(f,26);title=ImageFont.truetype(f.replace('Arial.ttf','Arial Bold.ttf'),40)
cache={v:[Image.open(ROOT/v/f'{i:03}.png').convert('RGB') for i in range(1,68)] for v in ['threequarter','side']}
path=OUT/'death-collapse-1080p.mp4';proc=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','rawvideo','-pixel_format','rgb24','-video_size','1920x1080','-framerate','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(path)],stdin=subprocess.PIPE)
for tick in range(360):
 im=Image.new('RGB',(1920,1080),(15,21,30));d=ImageDraw.Draw(im)
 d.text((40,24),'GRAYBOX REACH / LIGHT ENEMY / DEATH',font=title,fill=(233,238,248))
 d.text((40,82),'Balance lost / knee buckle / collapse / settle',font=font,fill=(160,180,204))
 for v,x in [('threequarter',160),('side',1120)]:im.paste(cache[v][min(tick%120,66)],(x,140))
 phase=tick%120;active=False
 d.text((790,980),'FATAL HIT' if phase<11 else 'BUCKLE' if phase<25 else 'FALL' if phase<40 else 'SETTLE' if phase<58 else 'STILL',font=font,fill=(255,115,95) if active else (118,212,199))
 d.text((40,1027),'Actual game speed / 2.20-second death / 30 fps / final corpse pose held',font=font,fill=(160,180,204))
 if tick==58:im.save(path.with_suffix('.png'))
 proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0;print(path)
