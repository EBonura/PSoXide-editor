import bpy,struct,json,math
import numpy as np
from pathlib import Path
from mathutils import Vector,Matrix
P=Path(__file__).resolve().parents[3];O=Path(__file__).resolve().parent
src=(P/'tools/enemy_reduction/validate_render.py').read_text();exec(src[src.index('def model'):src.index('def mesh')]);rig=np.load(P/'source_assets/characters/light_body_v1/bind.npz');bind=rig['bind'];ib=np.linalg.inv(bind);parents=rig['parents'];m=model(P/'assets/models/light_body_v1/light.psxmdl')

def skin(path):
 poses=animation(path);out=np.tile(np.eye(4),(len(poses),22,1,1))
 for k,pose in enumerate(poses):
  for j,(r,t) in enumerate(pose):out[k,j,:3,:3]=r;out[k,j,:3,3]=t
 return out

def turn(g,j,deg,axis,ids):
 r=np.array(Matrix.Rotation(math.radians(deg),3,axis));p=g[j,:3,3].copy()
 for k in ids:g[k,:3,3]=p+r@(g[k,:3,3]-p);g[k,:3,:3]=r@g[k,:3,:3]

def ik(g,a,b,c,target,pole):
 h=g[a,:3,3].copy();k=g[b,:3,3].copy();f=g[c,:3,3].copy();l1=np.linalg.norm(k-h);l2=np.linalg.norm(f-k);v=target-h;d=np.linalg.norm(v);assert d<l1+l2,(a,d,l1+l2);axis=v/d;pole=pole-axis*np.dot(pole,axis);pole/=np.linalg.norm(pole);along=(l1*l1-l2*l2+d*d)/(2*d);nk=h+axis*along+pole*math.sqrt(max(0,l1*l1-along*along))
 g[a,:3,:3]=np.array(Vector(k-h).rotation_difference(Vector(nk-h)).to_matrix())@g[a,:3,:3];g[b,:3,:3]=np.array(Vector(f-k).rotation_difference(Vector(target-nk)).to_matrix())@g[b,:3,:3];g[b,:3,3]=nk;g[c,:3,3]=target
 return d/(l1+l2)

def pose_stance(src,kind):
 out=[];extensions=[];idle_feet=(src[0]@bind)
 for k,sk in enumerate(src):
  old=sk@bind;g=old.copy();g[:,:3,3]+=np.array([500 if kind=='idle' else 0,-2200 if kind=='idle' else -1000,700 if kind=='idle' else 300])
  # Distributed spine bend, with the head looking forward under the brow.
  lean=.85 if kind=='idle' else 1/6
  turn(g,1,12*lean,'X',range(1,14));turn(g,2,12*lean,'X',range(2,14));turn(g,3,6*lean,'X',range(3,14));turn(g,4,-18*lean,'X',[4,5])
  turn(g,2,6 if kind=='idle' else 2,'Y',range(2,14))
  for shoulder,side,arm,elbow,hand in [(6,1,7,8,9),(10,-1,11,12,13)]:
   turn(g,shoulder,(3 if side==1 else -8),'Z',range(shoulder,hand+1))
   target=old[hand,:3,3]+np.array([side*350,-1500 if side==1 else -300,2200 if side==1 else 900]);extensions.append(ik(g,arm,elbow,hand,target,np.array([side*.65,-.1,-.9])))
   # Keep the end effector's source attitude: claws hang down, cannon stays readable.
   g[hand,:3,:3]=old[hand,:3,:3]
  for side,hip,knee,ankle,toe in [(1,14,15,16,17),(-1,18,19,20,21)]:
   feet=(idle_feet if kind=='idle' else old)[[ankle,toe]].copy();offset=np.array([(1700 if side==1 else -350) if kind=='idle' else side*250,0,(2200 if side==1 else -1400) if kind=='idle' else 0]);feet[:,:3,3]+=offset
   h=g[hip,:3,3];pole=old[knee,:3,3]-old[hip,:3,3]+np.array([side*300,0,1800]);extensions.append(ik(g,hip,knee,ankle,feet[0,:3,3],pole));g[ankle]=feet[0];g[toe]=feet[1]
  out.append(g@ib)
 return np.array(out),max(extensions)

for kind,path,hz in [('idle',P.parent/'default/assets/animations/rust_mantis_starter/idle.psxanim',12),('walk',P/'source_assets/animations/light_walk_v1/walk.psxanim',30)]:
 original=skin(path);new,ext=pose_stance(original,kind);shift=max(0,math.ceil(math.log2(max(1,np.abs(new[:,:,:3,3]).max())/32760)));packed=np.concatenate([new,new[:1]]) if kind=='walk' else new;data=bytearray(struct.pack('<4H',22,len(packed),hz,shift))
 for frame in packed:
  for mat in frame:data+=struct.pack('<9h3h',*np.rint(mat[:3,:3].T*4096).astype(int).ravel(),*np.rint(mat[:3,3]/2**shift).astype(int))
 head=bytearray(path.read_bytes()[:12]);struct.pack_into('<H',head,4,2);struct.pack_into('<I',head,8,len(data));(O/f'{kind}.psxanim').write_bytes(head+data);np.savez(O/f'{kind}.npz',skin=new,bind=bind,parents=parents);print('RESULT',kind,len(new),ext)
