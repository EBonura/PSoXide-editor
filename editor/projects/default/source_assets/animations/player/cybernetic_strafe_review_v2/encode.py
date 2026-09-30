from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess,json
out=Path(__file__).resolve().parent;p=out.parents[3]/'review/strafe-v2';old=p.parent/'strafe-v1';font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',18);bold=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',23)
proc=subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pix_fmt','rgb24','-s','960x576','-r','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(p/'side-step-comparison.mp4')],stdin=subprocess.PIPE)
for side,stem,view in [('left','walk_lft','quarter'),('right','walk_rgt','quarter'),('left','walk_lft','front'),('right','walk_rgt','front')]:
 for f in range(120):
  im=Image.new('RGB',(960,576),(14,21,29));d=ImageDraw.Draw(im);im.paste(Image.open(old/stem/view/f'{f%42:03}.png'),(0,72));im.paste(Image.open(p/stem/view/f'{f%30:03}.png'),(480,72));d.text((16,8),'CORTEX IGNITION / LOCKED-ON SIDE STEPS',font=bold,fill=(238,225,190));d.text((16,39),side.upper()+' / '+view.upper()+' VIEW',font=font,fill=(155,183,197));d.text((16,550),'V1 / 1.40s CYCLE',font=font,fill=(220,236,245));d.text((496,550),'V2 / 1.00s CYCLE',font=font,fill=(220,236,245));proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0
im=Image.new('RGB',(1440,1080))
for row,(stem,v) in enumerate([('walk_lft','front'),('walk_lft','quarter'),('walk_rgt','front')]):
 for col,f in enumerate([0,5,10,15,20,25]):
  im.paste(Image.open(p/stem/v/f'{f:03}.png').resize((240,240)),(col*240,row*360));ImageDraw.Draw(im).text((col*240+5,row*360+240),f'{stem} {f}',fill='white')
im.save(p/'qa.jpg')
for stem in ['walk_lft','walk_rgt']:
 d=json.loads((out/(stem+'-validation.json')).read_text());assert d['max_extension']<.99;assert d['loop_seam_max_vertex_distance']<1e-5;assert d['lowest_vertex_subframe_z']>=0
print('RESULT faster left/right comparison encoded; floor and seam checks passed')
