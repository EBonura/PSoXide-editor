from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess,json,hashlib
out=Path(__file__).resolve().parent;project=out.parents[3];p=project/'review/run-v2';old=p/'original';font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',18);bold=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',23)
report=json.loads((out/'validation.json').read_text());assert report['loop_seam_max_vertex_distance']<1e-5;assert max(x['unchanged_body_max_matrix_delta'] for x in report['samples'])<1e-5
for b in report['framing'].values():assert 0<b['xmin']<b['xmax']<1 and 0<b['ymin']<b['ymax']<1
proc=subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pix_fmt','rgb24','-s','960x576','-r','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(p/'run-v2-comparison.mp4')],stdin=subprocess.PIPE)
for view in ['side','quarter','front']:
 for f in range(126):
  im=Image.new('RGB',(960,576),(14,21,29));im.paste(Image.open(old/('run-'+view)/f'{f%18:03}.png'),(0,72));im.paste(Image.open(p/view/f'{f%18:03}.png'),(480,72));d=ImageDraw.Draw(im);d.text((16,8),'CORTEX IGNITION / FORWARD RUN V2',font=bold,fill=(238,225,190));d.text((16,39),view.upper()+' / MATCHED 0.60s CYCLES / REAL TIME',font=font,fill=(155,183,197));d.text((16,550),'ORIGINAL',font=font,fill=(220,236,245));d.text((496,550),'V2 / ORIGINAL-BASED REFINEMENT',font=font,fill=(220,236,245));proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0
subprocess.run(['ffmpeg','-v','error','-y','-i',str(p/'run-v2-comparison.mp4'),'-vf','fps=2,scale=480:288,tile=5x5','-frames:v','1',str(p/'video-qa.jpg')],check=True)
report['source_sha256']={n:hashlib.sha256((out/n).read_bytes()).hexdigest() for n in ['run_fwd.blend','run_fwd.glb']};report['preview']='review/run-v2/run-v2-comparison.mp4';report['stage_floor_z']=report['lowest_vertex_subframe_z'];report['scope']='Studio review of original game poses versus reconstructed original-based source; candidate not installed. Each is floor-aligned for presentation. Runtime rebake and transition validation pending.';(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT v2 comparison encoded; body preservation and loop checks passed')
