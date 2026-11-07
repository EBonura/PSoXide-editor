"""Assemble full-resolution frames into a 1080p real-time review."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess
P=Path(__file__).resolve().parents[3];ROOT=P.parents[2]/'build/graybox-reach/light-motion-v1/frames';OUT=P/'validation/light-motion-v1'
f='/System/Library/Fonts/Supplemental/Arial.ttf';bold=f.replace('Arial.ttf','Arial Bold.ttf');font=ImageFont.truetype(f,24);large=ImageFont.truetype(bold,37);label=ImageFont.truetype(bold,27)
counts={'run':20,'turn':24,'alert':61};cache={}
for name in counts:
 for view in ['threequarter','side']:
  cache[name,view]=[Image.open(ROOT/name/view/f'{i:03}.png').convert('RGB') for i in range(1,counts[name]+1)]
video=OUT/'run-turn-alert-1080p.mp4'
proc=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','rawvideo','-pixel_format','rgb24','-video_size','1920x1080','-framerate','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(video)],stdin=subprocess.PIPE)
for tick in range(240):
 view='threequarter' if tick<120 else 'side'
 im=Image.new('RGB',(1920,1080),(15,21,30));d=ImageDraw.Draw(im)
 d.text((35,22),'GRAYBOX REACH / LIGHT ENEMY MOTION',font=large,fill=(233,238,248))
 d.text((35,73),'Actual playback speed / '+('three-quarter view' if view=='threequarter' else 'side view'),font=font,fill=(160,180,204))
 for col,(name,count) in enumerate(counts.items()):
  fi=tick%count if name!='alert' else min(tick%90,count-1)
  im.paste(cache[name,view][fi],(col*640,145));d.text((col*640+30,113),name.upper(),font=label,fill=(233,238,248))
  line={'run':'0.67 s / cycle','turn':'0.80 s / cycle','alert':'2.00 s / then hold'}[name]
  d.text((col*640+30,1022),line,font=label,fill=(118,212,199))
 if tick==8:im.save(video.with_suffix('.png'))
 proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0
sheet=Image.new('RGB',(1920,1296),(15,21,30));d=ImageDraw.Draw(sheet)
for row,name in enumerate(counts):
 for col,(view,fi) in enumerate([('threequarter',0),('threequarter',3),('threequarter',9),('side',0),('side',5),('side',12)]):
  sheet.paste(cache[name,view][fi].resize((320,432)),(col*320,row*432));d.text((col*320+8,row*432+8),f'{name} f{fi}',font=font,fill='white')
sheet.save(OUT/'pose-review.jpg');print(video)
