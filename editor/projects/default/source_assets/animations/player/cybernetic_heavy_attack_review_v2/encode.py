from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import subprocess
project=Path(__file__).resolve().parents[4];p=project/'review/heavy-attack-v2'
regular=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',21);bold=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',27)
def decode(name):
 data=subprocess.check_output(['ffmpeg','-v','error','-i',str(p/(name+'.mp4')),'-f','rawvideo','-pix_fmt','rgb24','-']);size=720*720*3
 return [Image.frombytes('RGB',(720,720),data[i:i+size]) for i in range(0,len(data),size)]
a=decode('front');b=decode('quarter');assert len(a)==len(b)==61
out=p/'heavy-cross-slash-preview.mp4';proc=subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pix_fmt','rgb24','-s','1440x830','-r','30','-i','-','-an','-c:v','libx264','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(out)],stdin=subprocess.PIPE)
seq=[]
for speed in [1,1,2]:
 seq.extend([(0,speed)]*10)
 for f in range(61):seq.extend([(f,speed)]*speed)
 seq.extend([(60,speed)]*16)
for f,speed in seq:
 im=Image.new('RGB',(1440,830),(14,21,29));im.paste(a[f],(0,72));im.paste(b[f],(720,72));d=ImageDraw.Draw(im)
 d.text((24,12),'HORIZON HEAVY / CROSS SLASH — PASS 2 — MORE DRIVE',font=bold,fill=(222,243,248));d.text((24,46),'FRONT',font=regular,fill=(135,171,184));d.text((744,46),'THREE-QUARTER / WEIGHT TRANSFER',font=regular,fill=(135,171,184));d.text((1210,16),'HALF SPEED' if speed==2 else 'NORMAL SPEED',font=regular,fill=(255,199,111))
 phase='READY' if f<6 else 'CAST HEAVY BLADE' if f<12 else 'CAST SECOND BLADE / LOAD' if f<18 else 'DRIVE' if f<24 else 'CROSS CUT / ABSORB' if f<30 else 'FOLLOW-THROUGH' if f<42 else 'RECOVER'
 d.text((24,798),phase,font=regular,fill=(255,199,111));d.text((744,798),'Review candidate · blade casting shown schematically',font=regular,fill=(135,171,184));proc.stdin.write(im.tobytes())
proc.stdin.close();assert proc.wait()==0
subprocess.run(['ffprobe','-v','error','-show_entries','format=duration,size','-of','json',str(out)],check=True)
# A contact sheet from the actual encoded video for final QA.
subprocess.run(['ffmpeg','-v','error','-y','-i',str(out),'-vf',r'select=eq(n\,33)+eq(n\,35)+eq(n\,36)+eq(n\,39),scale=960:-1,tile=2x2','-frames:v','1',str(p/'video-qa.jpg')],check=True)
