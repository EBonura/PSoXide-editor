"""1080p side/three-quarter review of the pointing alert at game speed."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess
P=Path(__file__).resolve().parents[3];ROOT=P.parents[2]/'build/graybox-reach/light-motion-v1/frames/alert';OUT=P/'validation/light-motion-v1'
f='/System/Library/Fonts/Supplemental/Arial.ttf';font=ImageFont.truetype(f,26);title=ImageFont.truetype(f.replace('Arial.ttf','Arial Bold.ttf'),40)
cache={v:[Image.open(ROOT/v/f'{i:03}.png').convert('RGB') for i in range(1,62)] for v in ['threequarter','side']}
path=OUT/'alert-single-step-1080p.mp4';proc=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','rawvideo','-pixel_format','rgb24','-video_size','1920x1080','-framerate','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(path)],stdin=subprocess.PIPE)
for tick in range(270):
 im=Image.new('RGB',(1920,1080),(15,21,30));d=ImageDraw.Draw(im)
 d.text((40,24),'GRAYBOX REACH / POINTING ALERT',font=title,fill=(233,238,248))
 d.text((40,82),'One forward step / rear foot planted / point / recover',font=font,fill=(160,180,204))
 for v,x in [('threequarter',160),('side',1120)]:im.paste(cache[v][min(tick%90,60)],(x,140))
 d.text((40,1027),'Actual game speed / 2.00-second gesture / pause between repeats',font=font,fill=(160,180,204))
 if tick==25:im.save(path.with_suffix('.png'))
 proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0;print(path)
