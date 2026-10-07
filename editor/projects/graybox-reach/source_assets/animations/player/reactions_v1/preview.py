from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess,json
O=Path(__file__).resolve().parent;P=O.parents[3];ROOT=P.parents[2]/'build/graybox-reach/player-reactions-v1/frames';OUT=P/'validation/player-reactions-v1'
f='/System/Library/Fonts/Supplemental/Arial.ttf';font=ImageFont.truetype(f,26);title=ImageFont.truetype(f.replace('Arial.ttf','Arial Bold.ttf'),38)
cache={}
for clip in ['poise','hit','old_active','old_stun']:
 for view in ['quarter','side']:cache[(clip,view)]=[Image.open(p).convert('RGB') for p in sorted((ROOT/clip/view).glob('*.png'))][::2 if clip in ('poise','hit') else 1]
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
movie('poise-break-1080p',[('poise','quarter','Three-quarter'),('poise','side','Side')],'ALETHA / POISE BREAK / CANDIDATE','Chest recoil, knees absorb, one foot braces, free hand protects ribs. 0.80 seconds.')
movie('hit-reaction-1080p',[('hit','quarter','Three-quarter'),('hit','side','Side')],'ALETHA / SMALL HIT REACTION / CANDIDATE','Planted feet, asymmetric torso recoil, restrained recovery. 0.60 seconds. Currently unused.')
movie('before-after-1080p',[('old_active','quarter','Current active reaction / 4x'),('poise','quarter','Poise-break candidate / 1x')],'ALETHA / CURRENT VS CANDIDATE','Current hit-react source at configured speed; candidate includes recoil, brace and recovery.')
# Review sampled directly from rendered playblast frames.
im=Image.new('RGB',(1920,1820),(15,21,30));d=ImageDraw.Draw(im)
for row,(clip,view) in enumerate([('poise','quarter'),('poise','side'),('hit','quarter'),('old_active','quarter')]):
 seq=cache[(clip,view)]
 for col,f in enumerate([0,3,7,11,18,len(seq)-1]):
  f=min(f,len(seq)-1);x=col*320;y=row*455;im.paste(seq[f].resize((320,432)),(x,y+22));d.text((x+8,y+3),f'{clip} / {view} / frame {f}',fill='white')
im.save(OUT/'pose-review.jpg')
