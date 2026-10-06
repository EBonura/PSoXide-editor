import bpy,json,struct,math
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
P=Path(__file__).resolve().parents[3];D=P.parent/'default';OUT=Path(__file__).resolve().parent;T=P/'tools/enemy_reduction';__file__=str(T/'validate_render.py');exec((T/'validate_render.py').read_text().split('report={}')[0]);print('VERSION',bpy.app.version_string)
rig=np.load(OUT/'rig.npz');bind=rig['bind'];ib=np.linalg.inv(bind);parents=rig['parents'];local=rig['local'];original=animation(D/'assets/animations/rust_mantis_starter/walk.psxanim');m=model(P/'assets/models/enemy_reduced/light.psxmdl');N=24;HZ=30;duty=.52;scale=((113*417+128)//256)/4096;speed=28*60/scale*(N/HZ);floor=-20000.;height=np.ptp(np.array(m['v'])[:,1]);phase_offset=[4/22.5,4/22.5+.5]
# A full source stride spans approximately 22.5 samples. Periodic rotation curves
# keep the source pose language while removing the partial-stride seam.
qref=[];logs=[];trans=np.mean(local[:,:,:3,3],axis=0)
for j in range(22):
 qs=[Matrix(a[:3,:3].tolist()).to_quaternion().normalized() for a in local[:,j]];ref=qs[1];qref.append(ref);vals=[]
 for k in range(N):
  f=1+k/N*22.5;i=int(f);q=qs[i].slerp(qs[i+1],f-i);vals.append(tuple((ref.inverted()@q).to_exponential_map()))
 ff=np.fft.rfft(np.array(vals),axis=0);ff[6:]=0;logs.append(np.fft.irfft(ff,n=N,axis=0))
base=[]
for k in range(N):
 gl=[]
 for j in range(22):
  q=qref[j]@Quaternion(Vector(logs[j][k]));mat=np.eye(4);mat[:3,:3]=np.array(q.to_matrix());mat[:3,3]=trans[j]
  if j==0:mat[1,3]+=250*math.cos(4*math.pi*(k/N-phase_offset[0]-.08))
  gl.append(mat if parents[j]<0 else gl[parents[j]]@mat)
 base.append(np.array(gl))
base=np.array(base);specs=[]
for leg,(hip,knee,ankle,toe,frame) in enumerate([(14,15,16,17,10),(18,19,20,21,21)]):
 g=rig['globals'][frame];skin=g@ib;pp=deform(m,[(a[:3,:3],a[:3,3]) for a in skin]);ids=[i for i in range(len(m['v'])) if m['owner'][i] in [ankle,toe]];foot=pp[ids];sole=np.array([foot[:,0].mean(),foot[:,1].min(),foot[:,2].mean()]);relative=[g[ankle].copy(),g[toe].copy()]
 for a in relative:a[:3,3]-=sole
 l1=np.linalg.norm(trans[knee]);l2=np.linalg.norm(trans[ankle]);specs.append((hip,knee,ankle,toe,relative,float(base[:,hip,0,3].mean()),l1,l2))

def foot_target(k,leg):
 hip,knee,ankle,toe,relative,x,l1,l2=specs[leg];u=(k/N-phase_offset[leg])%1;travel=speed*duty;front=travel/2-800;back=-travel/2-800;pitch=0
 if u<duty:y=0;z=front-speed*u
 else:
  v=(u-duty)/(1-duty);h00=2*v**3-3*v**2+1;h10=v**3-2*v**2+v;h01=-2*v**3+3*v**2;h11=v**3-v**2;z=h00*back+h10*(-speed)*(1-duty)+h01*front+h11*(-speed)*(1-duty);y=2200*math.sin(math.pi*v)**2;pitch=-.14*math.sin(2*math.pi*v)
 r=np.array(Matrix.Rotation(pitch,3,'X'));target=np.array([x,floor+y,z]);out=[]
 for mat in relative:
  a=mat.copy();a[:3,:3]=r@a[:3,:3];a[:3,3]=r@a[:3,3]+target;out.append(a)
 return out,u<duty
# Use a constant pelvis adjustment to keep both chains below full extension.
required=0
for k in range(N):
 for leg,(hip,knee,ankle,toe,rel,x,l1,l2) in enumerate(specs):
  target=foot_target(k,leg)[0][0][:3,3];h=base[k,hip,:3,3];horizontal=np.sum((h[[0,2]]-target[[0,2]])**2);vertical=math.sqrt(max(1,(.985*(l1+l2))**2-horizontal));required=max(required,h[1]-target[1]-vertical)
print('PELViS_DROP',required)
new=[];contact=[];extension=[]
for k in range(N):
 gl=base[k].copy();gl[:,:3,3]-=np.array([0,required,0]);contacts=[]
 for leg,(hip,knee,ankle,toe,rel,x,l1,l2) in enumerate(specs):
  feet,plant=foot_target(k,leg);target=feet[0][:3,3];h=gl[hip,:3,3];oldk=gl[knee,:3,3].copy();olda=gl[ankle,:3,3].copy();v=target-h;d=np.linalg.norm(v);extension.append(d/(l1+l2));axis=v/d;pole=oldk-h-axis*np.dot(oldk-h,axis);pole/=np.linalg.norm(pole);along=(l1*l1-l2*l2+d*d)/(2*d);bend=math.sqrt(max(0,l1*l1-along*along));newk=h+axis*along+pole*bend
  r1=np.array(Vector(oldk-h).rotation_difference(Vector(newk-h)).to_matrix());r2=np.array(Vector(olda-oldk).rotation_difference(Vector(target-newk)).to_matrix());gl[hip,:3,:3]=r1@gl[hip,:3,:3];gl[knee,:3,:3]=r2@gl[knee,:3,:3];gl[knee,:3,3]=newk;gl[ankle]=feet[0];gl[toe]=feet[1];contacts.append(plant)
 new.append(gl@ib);contact.append(contacts)
new=np.array(new);shift=max(0,math.ceil(math.log2(max(1,np.abs(new[:,:,:3,3]).max())/32760)));data=bytearray(struct.pack('<4H',22,N,HZ,shift))
for frame in new:
 for mat in frame:data+=struct.pack('<9h3h',*np.rint(mat[:3,:3].T*4096).astype(int).ravel(),*np.rint(mat[:3,3]/2**shift).astype(int))
b=bytearray((D/'assets/animations/rust_mantis_starter/walk.psxanim').read_bytes()[:12]);struct.pack_into('<H',b,4,2);struct.pack_into('<I',b,8,len(data));b+=data;(OUT/'walk.psxanim').write_bytes(b)
np.savez(OUT/'walk.npz',skin=new,contact=contact,bind=bind,parents=parents)
(OUT/'authoring.json').write_text(json.dumps({'frames':N,'sample_hz':HZ,'period_seconds':N/HZ,'source_stride':[1,23.5],'stance_fraction':duty,'swing_clearance_model_units':2200,'pelvis_drop_model_units':required,'max_leg_extension':max(extension),'target_world_speed_per_second':1680,'visual_scale_q8':417,'local_to_world_scale':scale,'translation_shift':shift},indent=2));print('RESULT',N,HZ,max(extension))
