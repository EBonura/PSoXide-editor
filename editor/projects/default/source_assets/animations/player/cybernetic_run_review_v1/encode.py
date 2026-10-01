from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess,json,hashlib,math
out=Path(__file__).resolve().parent;p=out.parents[3]/'review/run-v1';font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',18);bold=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',23)
report=json.loads((out/'validation.json').read_text())
assert report['max_extension']<.99
assert report['loop_seam_max_vertex_distance']<1e-5
assert report['lowest_vertex_subframe_z']>=0
assert report['moving_contact_max_step_m']<.001
for framing in report['framing'].values():
 assert 0<framing['xmin']<framing['xmax']<1 and 0<framing['ymin']<framing['ymax']<1
proc=subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pix_fmt','rgb24','-s','960x576','-r','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(p/'run-comparison.mp4')],stdin=subprocess.PIPE)
for view in ['quarter','side','front']:
 for f in range(120):
  im=Image.new('RGB',(960,576),(14,21,29));d=ImageDraw.Draw(im);im.paste(Image.open(p/'original'/('run-'+view)/f'{f%18:03}.png'),(0,72));im.paste(Image.open(p/view/f'{f%20:03}.png'),(480,72));d.text((16,8),'CORTEX IGNITION / FORWARD RUN',font=bold,fill=(238,225,190));d.text((16,39),view.upper()+' VIEW / REAL-TIME PLAYBACK',font=font,fill=(155,183,197));d.text((16,550),'ORIGINAL / 0.60s CYCLE',font=font,fill=(220,236,245));d.text((496,550),'NEW V1 / 0.67s CYCLE',font=font,fill=(220,236,245));proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0
# Inspect representative frames from the actual encoded video, plus trajectory diagnostics.
subprocess.run(['ffmpeg','-v','error','-y','-i',str(p/'run-comparison.mp4'),'-vf','fps=2,scale=480:288,tile=6x4','-frames:v','1',str(p/'video-qa.jpg')],check=True)
ims=Image.new('RGB',(1440,720),(20,26,35));d=ImageDraw.Draw(ims)
for row,view in enumerate(['quarter','side']):
 for col,f in enumerate([0,2,4,7,9,12]):
  ims.paste(Image.open(p/view/f'{f:03}.png').resize((240,240)),(col*240,row*360));d.text((col*240+8,row*360+243),f'{view} / frame {f}',font=font,fill='white')
ims.save(p/'pose-qa.jpg')
curve=Image.new('RGB',(1000,700),(20,26,35));d=ImageDraw.Draw(curve)
def plot(vals,box,color):
 x,y,w,h=box;lo=min(vals);hi=max(vals);span=max(hi-lo,1e-6);pts=[(x+i*w/(len(vals)-1),y+h-(v-lo)/span*h) for i,v in enumerate(vals)];d.rectangle((x,y,x+w,y+h),outline=(85,100,115));d.line(pts,fill=color,width=3);return lo,hi
lo,hi=plot([s['hip'][2] for s in report['samples']],(50,45,900,180),(92,220,220));d.text((50,12),f'Hip height / {lo:.3f}-{hi:.3f} m',font=font,fill='white')
for side,col in [('Left',(250,180,95)),('Right',(90,180,250))]:
 samples=[s['feet'][side]['ankle'] for s in report['samples']];pts=[(500+v[1]*750,585-v[2]*650) for v in samples];d.line(pts+[pts[0]],fill=col,width=3)
 for q in pts:d.ellipse((q[0]-3,q[1]-3,q[0]+3,q[1]+3),fill=col)
d.text((50,270),'Ankle arcs / sagittal plane / orange left, blue right',font=font,fill='white');curve.save(p/'motion-qa.png')
report['source_sha256']={n:hashlib.sha256((out/n).read_bytes()).hexdigest() for n in ['run_fwd.blend','run_fwd.glb']}
report['preview']='review/run-v1/run-comparison.mp4';report['comparison']='Original cooked game poses at current speed; candidate source poses at 30 fps. Matched character, stage and cameras. Candidate not installed; world-speed matching and transitions remain to be tested on approval.'
(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
print('RESULT run video encoded; floor, reach, framing, contact and loop checks passed')
