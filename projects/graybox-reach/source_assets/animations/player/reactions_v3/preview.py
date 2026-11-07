from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess,json
O=Path(__file__).resolve().parent;P=O.parents[3];ROOT=P.parents[2]/'build/graybox-reach/player-reactions-v3/frames';OUT=P/'validation/player-reactions-v3'
f='/System/Library/Fonts/Supplemental/Arial.ttf';font=ImageFont.truetype(f,26);title=ImageFont.truetype(f.replace('Arial.ttf','Arial Bold.ttf'),38)
cache={}
audit=json.loads((OUT/'audit.json').read_text())
for clip in ['poise','hit']:
 for view in ['quarter','side']:cache[(clip,view)]=[Image.open(p).convert('RGB') for p in [ROOT/clip/view/f'{i:03}.png' for i in range(audit['clips'][clip]['frames'])]][::2]
def movie(filename,clips,heading,detail):
 path=OUT/(filename+'.mp4');proc=subprocess.Popen(['ffmpeg','-y','-loglevel','error','-f','rawvideo','-pixel_format','rgb24','-video_size','1920x1080','-framerate','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(path)],stdin=subprocess.PIPE)
 for tick in range(270):
  im=Image.new('RGB',(1920,1080),(15,21,30));d=ImageDraw.Draw(im);d.text((40,22),heading,font=title,fill=(236,241,248));d.text((40,78),detail,font=font,fill=(168,189,210));phase=tick%90
  for x,(clip,view,label) in zip([160,1120],clips):
   seq=cache[(clip,view)];im.paste(seq[min(max(0,phase-12),len(seq)-1)],(x,132));d.text((x,998),label,font=font,fill=(129,216,205))
  d.text((40,1040),'Animation study / actual intended speed / 30 fps / pause between repeats',font=font,fill=(162,181,204))
  if tick==19:im.save(path.with_suffix('.png'))
  proc.stdin.write(im.tobytes())
 proc.stdin.close();assert proc.wait()==0;print(path)
movie('poise-break-1080p',[('poise','quarter','Three-quarter'),('poise','side','Side')],'ALETHA / POISE BREAK / REVISION 3','Hard recoil / legs buckle / one-knee collapse / effortful recovery / 2.07 seconds')
movie('hit-reaction-1080p',[('hit','quarter','Three-quarter'),('hit','side','Side')],'ALETHA / HIT REACTION / REVISION 3','Previous poise-break motion reassigned unchanged as hit reaction / 1.33 seconds')
im=Image.new('RGB',(2240,1820),(15,21,30));d=ImageDraw.Draw(im)
for row,(clip,view) in enumerate([('poise','quarter'),('poise','side'),('hit','quarter'),('hit','side')]):
 seq=cache[(clip,view)]
 for col,f in enumerate([0,4,10,19,28,42,len(seq)-1]):
  f=min(f,len(seq)-1);x=col*320;y=row*455;im.paste(seq[f].resize((320,432)),(x,y+22));d.text((x+8,y+3),f'{clip} / {view} / {f/30:.2f}s',fill='white')
im.save(OUT/'pose-review.jpg')
