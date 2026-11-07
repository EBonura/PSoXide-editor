from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess
out=Path(__file__).resolve().parent;p=out.parents[3]/'review/back-walk-v3';audit=p.parent/'back-walk-v2';font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',18);bold=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',23)

def encode(name,sections):
 proc=subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pix_fmt','rgb24','-s','960x576','-r','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(p/name)],stdin=subprocess.PIPE)
 for mode,view in sections:
  for f in range(120 if mode=='compare' else 126):
   im=Image.new('RGB',(960,576),(14,21,29));d=ImageDraw.Draw(im)
   if mode=='compare':
    left=Image.open(audit/view/f'{f%42:03}.png');right=Image.open(p/view/f'{f%42:03}.png');labels=['DYNAMIC BASE / V2','POLISHED / V3'];subtitle=view.upper()+' VIEW | studio comparison'
   else:
    left=Image.open(p/'front'/f'{f%42:03}.png');right=Image.open(p/'side'/f'{f%42:03}.png');labels=['REFINED / FRONT','REFINED / SIDE'];subtitle='DETAIL VIEWS | three continuous cycles'
   im.paste(left,(0,72));im.paste(right,(480,72));d.text((16,8),'CORTEX IGNITION / BACKWARD WALK',font=bold,fill=(238,225,190));d.text((16,39),subtitle,font=font,fill=(155,183,197));d.text((16,550),labels[0],font=font,fill=(220,236,245));d.text((496,550),labels[1],font=font,fill=(220,236,245));proc.stdin.write(im.tobytes())
 proc.stdin.close();assert proc.wait()==0
encode('back-walk-comparison.mp4',[('compare','quarter'),('compare','side')]);encode('back-walk-detail.mp4',[('detail','front')])
im=Image.new('RGB',(1440,720))
for row,v in enumerate(['quarter','side']):
 for col,f in enumerate([0,7,14,21]):
  a=Image.open(p/v/f'{f:03}.png').resize((360,360));im.paste(a,(col*360,row*360))
im.save(p/'final-qa.jpg');print('RESULT videos encoded')
