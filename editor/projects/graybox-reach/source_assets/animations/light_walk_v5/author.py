import bpy,math,json,struct
import numpy as np
from pathlib import Path
from mathutils import Vector,Matrix
P=Path(__file__).resolve().parents[3];O=Path(__file__).resolve().parent
src=(P/'source_assets/animations/light_stalk_v2/author.py').read_text();exec(src[src.index('def skin'):src.index('def pose_stance')]);text=(P/'tools/enemy_reduction/validate_render.py').read_text();exec(text[text.index('def model'):text.index('def mesh')]);data=np.load(P/'source_assets/characters/light_body_v1/bind.npz');bind=data['bind'];ib=np.linalg.inv(bind);parents=data['parents'];m=model(P/'assets/models/light_body_v1/light.psxmdl');base=skin(P/'assets/animations/light_stalk_v2/idle.psxanim')[0]@bind;source=np.load(P/'source_assets/animations/light_walk_v1/rig.npz');N=24;HZ=30;DUTY=.56;SCALE=184/4096;SPEED=1680/SCALE;PERIOD=N/HZ;TRAVEL=SPEED*PERIOD;FLOOR=-20000.;LIFT=1100.;legs=[]
# Retain the flattened mechanical foot attitudes from the planted source poses.
for side,hip,knee,ankle,toe,frame in [(1,14,15,16,17,10),(-1,18,19,20,21,21)]:
 g=source['globals'][frame];pp=deform(m,[(a[:3,:3],a[:3,3]) for a in g@ib]);ids=[i for i in range(len(m['v'])) if m['owner'][i] in [ankle,toe]];foot=pp[ids];sole=np.array([foot[:,0].mean(),foot[:,1].min(),foot[:,2].mean()]);relative=g[[ankle,toe]].copy();relative[:,:3,3]-=sole
 legs.append((side,hip,knee,ankle,toe,relative,base[hip,0,3]+side*800,np.linalg.norm(base[knee,:3,3]-base[hip,:3,3]),np.linalg.norm(base[ankle,:3,3]-base[knee,:3,3])))

def feet_at(k,leg):
 side,hip,knee,ankle,toe,relative,x,l1,l2=legs[leg];u=(k/N-leg*.5)%1;front=TRAVEL*DUTY/2-800;back=-TRAVEL*DUTY/2-800
 if u<DUTY:z=front-TRAVEL*u;y=0
 else:
  v=(u-DUTY)/(1-DUTY);z=(2*v**3-3*v**2+1)*back+(v**3-2*v*v+v)*(-TRAVEL)*(1-DUTY)+(-2*v**3+3*v*v)*front+(v**3-v*v)*(-TRAVEL)*(1-DUTY);y=LIFT*math.sin(math.pi*v)**2
 target=np.array([x,FLOOR+y,z]);out=relative.copy();out[:,:3,3]+=target
 return out,u<DUTY

# Contact at frame 0/12, compression at 2/14, recovery by 8/20.
# Periodic Hermite keys keep the impact/recovery timing asymmetric and seamless.
BOB_KEYS=[(0.,0.),(2.,-850.),(5.,-250.),(8.,250.),(10.,220.)]
def compression(k):
 t=k%12;keys=BOB_KEYS;count=len(keys)
 for i,(a,va) in enumerate(keys):
  j=(i+1)%count;b,vb=keys[j];b+=12 if j==0 else 0
  if a<=t<b:
   pt,pv=keys[(i-1)%count];pt-=12 if i==0 else 0
   nt,nv=keys[(j+1)%count];nt+=12 if j>=count-1 or j==0 else 0
   if j==0:nt=keys[1][0]+12
   ma=(vb-pv)/(b-pt);mb=(nv-va)/(nt-a);u=(t-a)/(b-a)
   return (2*u**3-3*u*u+1)*va+(u**3-2*u*u+u)*(b-a)*ma+(-2*u**3+3*u*u)*vb+(u**3-u*u)*(b-a)*mb
 raise AssertionError(t)

def body_at(k):
 phase=k/N;g=base.copy();weight=math.sin(2*math.pi*(phase-.015));g[:,:3,3]+=np.array([1750*weight,compression(k),120*math.sin(4*math.pi*phase)])
 # Pelvis leads; the chest opposes it one sample later, with a small impact fold.
 turn(g,0,-4*math.cos(2*math.pi*phase),'Y',range(22));turn(g,0,-2*weight,'Z',range(22))
 delayed=2*math.pi*(phase-1/N);twist=8+6*math.cos(delayed)+1.5*math.cos(2*delayed+.4);roll=1.5+3.8*math.sin(delayed)+1.2*math.sin(2*delayed-.35)
 turn(g,1,twist,'Y',range(1,14));turn(g,2,roll,'Z',range(2,14))
 pitch=-compression(k-1)/850*3.5*(1-.2*math.sin(2*math.pi*(phase-.04)));turn(g,2,pitch,'X',range(2,14))
 # Let the head follow, but counter some chest rotation to keep its gaze steady.
 turn(g,3,14,'Y',range(3,14));turn(g,4,-14,'Y',[4,5]);turn(g,4,compression(k-2)/850*1.5,'X',[4,5]);turn(g,4,-2*math.sin(delayed-2*math.pi/N),'Z',[4,5]);return g
# One constant lowering amount prevents knee locking without a per-frame hip pop.
drop=0
for k in range(N):
 g=body_at(k)
 for leg,(_,hip,knee,ankle,toe,rel,x,l1,l2) in enumerate(legs):
  feet,_=feet_at(k,leg);h=g[hip,:3,3];f=feet[0,:3,3];dxz=np.sum((h[[0,2]]-f[[0,2]])**2);drop=max(drop,h[1]-f[1]-math.sqrt(max(1,(.955*(l1+l2))**2-dxz)))
new=[];contacts=[];extensions=[]
for k in range(N):
 g=body_at(k);g[:,1,3]-=drop;phase=k/N;contact=[]
 for shoulder,side,arm,elbow,hand in [(6,1,7,8,9),(10,-1,11,12,13)]:
  # Hands lag the moving shoulder: cannon two samples, claw one sample.
  lag_frames=1. if side==1 else 2.;lagged=body_at(k-lag_frames);target=lagged[hand,:3,3].copy();target[1]-=drop
  swing=math.cos(2*math.pi*(phase-.07));target[2]+=(-1800 if side==1 else 900)*swing
  ik(g,arm,elbow,hand,target,np.array([side*.65,-.1,-.9]));g[hand,:3,:3]=lagged[hand,:3,:3]
 for leg,(side,hip,knee,ankle,toe,rel,x,l1,l2) in enumerate(legs):
  feet,plant=feet_at(k,leg);extensions.append(ik(g,hip,knee,ankle,feet[0,:3,3],np.array([side*.12,-.12,1.])));g[ankle]=feet[0];g[toe]=feet[1];contact.append(plant)
 new.append(g@ib);contacts.append(contact)
new=np.array(new);packed=np.concatenate([new,new[:1]]);shift=max(0,math.ceil(math.log2(max(1,np.abs(packed[:,:,:3,3]).max())/32760)));out=bytearray(struct.pack('<4sHHI4H',b'PSXA',2,0,8+len(packed)*22*24,22,len(packed),HZ,shift))
# Preserve source flags/magic exactly, write endpoint-inclusive 16-bit poses.
head=bytearray((P/'assets/animations/light_stalk_v2/walk.psxanim').read_bytes()[:12]);payload=bytearray(struct.pack('<4H',22,len(packed),HZ,shift))
for frame in packed:
 for a in frame:payload+=struct.pack('<9h3h',*np.rint(a[:3,:3].T*4096).astype(int).ravel(),*np.rint(a[:3,3]/2**shift).astype(int))
struct.pack_into('<H',head,4,2);struct.pack_into('<I',head,8,len(payload));(O/'walk.psxanim').write_bytes(head+payload);np.savez(O/'walk.npz',skin=new,bind=bind,parents=parents,contact=contacts)
meta={'unique_frames':N,'stored_frames':N+1,'sample_hz':HZ,'period_seconds':PERIOD,'stance_fraction':DUTY,'foot_clearance_model_units':LIFT,'extra_pelvis_lowering_model_units':drop,'max_leg_extension':max(extensions),'world_speed_per_second':1680,'visual_scale_q8':417,'hip_impact_keys':BOB_KEYS,'hip_lateral_amplitude':1750,'pelvis_yaw_degrees':4,'chest_counter_yaw_degrees':6,'chest_roll_degrees':4,'impact_chest_pitch_degrees':3.5,'claw_lag_frames':1,'cannon_lag_frames':2,'torso_yaw_bias_degrees':22,'waist_yaw_bias_degrees':8,'chest_yaw_bias_degrees':14,'shoulder_roll_bias_degrees':1.5,'gaze_counter_yaw_degrees':-14,'torso_yaw_second_harmonic_degrees':1.5,'shoulder_roll_second_harmonic_degrees':1.2,'impact_pitch_asymmetry_fraction':.2};(O/'authoring.json').write_text(json.dumps(meta,indent=2));print('RESULT',json.dumps(meta))
