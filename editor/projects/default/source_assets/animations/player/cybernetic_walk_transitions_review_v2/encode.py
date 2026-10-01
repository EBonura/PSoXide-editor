from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess,json,hashlib
out=Path(__file__).resolve().parent;p=out.parents[3]/'review/walk-transitions-v2';meta=json.loads((p/'render-validation.json').read_text());val=json.loads((out/'validation.json').read_text());font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',18);bold=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',22)
for v in val['transitions'].values():assert v['max_reach']<.99 and v['max_foot_target_error']<.001
proc=subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pix_fmt','rgb24','-s','960x600','-r','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(p/'walk-transitions-v2-comparison.mp4')],stdin=subprocess.PIPE)
for variant,view in [('A','quarter'),('B','quarter'),('A','side')]:
 key='new_'+variant+'_'+view;n=meta[key]['frames'];stop_start=n-48
 for f in range(n):
  im=Image.new('RGB',(960,600),(14,21,29));d=ImageDraw.Draw(im)
  for col,root in enumerate([p.parent/'walk-transitions-v1',p]):im.paste(Image.open(root/('new_'+variant+'_'+view)/f'{f:03}.png'),(480*col,90))
  phase='IDLE' if f<12 or f>=n-12 else 'START' if f<26 else 'APPROVED WALK' if f<stop_start else 'STOP / '+variant
  d.text((16,8),'CORTEX IGNITION / WALK TRANSITIONS V2',font=bold,fill=(238,225,190));d.text((16,39),view.upper()+' / STOP '+variant+' / '+phase,font=font,fill=(160,195,211));d.text((16,66),'Same approved walk in both panels; only transitions change',font=font,fill=(150,166,177));d.text((16,575),'V1 / PREVIOUS PASS',font=font,fill=(220,236,245));d.text((496,575),'V2 / MORE WEIGHT + FOLLOW-THROUGH',font=font,fill=(220,236,245));proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0
subprocess.run(['ffmpeg','-v','error','-y','-i',str(p/'walk-transitions-v2-comparison.mp4'),'-vf','fps=2,scale=480:300,tile=6x5','-frames:v','1',str(p/'video-qa.jpg')],check=True)
val['source_sha256']={f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in out.glob('*.glb')};val['render']=meta;val['preview']='review/walk-transitions-v2/walk-transitions-v2-comparison.mp4';val['limits']='Source studio reconstruction with short crossfades approximating the engine. Not installed. Runtime bake, root calibration, arbitrary-phase release and contact at actual game speed remain to be validated.';(out/'validation.json').write_text(json.dumps(val,indent=2)+'\n');print('RESULT transition comparison encoded')
