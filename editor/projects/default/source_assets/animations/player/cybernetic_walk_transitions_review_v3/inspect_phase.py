import bpy,sys,json,math,re,struct,hashlib
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent;project=out.parents[3];sys.path.insert(0,str(out));from decode_clip import decode
repo=project.parents[2];folder=out/'reference'
bpy.ops.wm.open_mainfile(filepath=str(out.parent/'cybernetic_walk/walk_fwd.blend'))
s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');meshes=[o for o in s.objects if o.type=='MESH'];r.animation_data_clear()
for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update();world=r.matrix_world.copy();inv=world.inverted();rest={b.name:world@b.matrix_local for b in r.data.bones}
names=json.loads('['+re.search(r'joint_names: \[([^]]+)\]',(project/'project.ron').read_text())[1]+']');assert len(names)==26
blob=(folder/'mesh.psxmdl').read_bytes();j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16;raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float)
# Recover the affine coordinate normalization from the same character's bind mesh.
dg=bpy.context.evaluated_depsgraph_get();verts=[]
for ob in meshes:
 ev=ob.evaluated_get(dg);me=ev.to_mesh();verts.extend(ev.matrix_world@v.co for v in me.vertices);ev.to_mesh_clear()
verts=np.array(verts);C=Matrix(((1,0,0),(0,0,-1),(0,1,0)));rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=(verts[:,2].max()-verts[:,2].min())/(rv[:,2].max()-rv[:,2].min());center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())
def capture():return {b.name:b.matrix_basis.copy() for b in r.pose.bones}
def apply(pose):
 for n,m in pose.items():r.pose.bones[n].matrix_basis=m
 bpy.context.view_layer.update()
def mix(a,b,t):
 result={}
 for n in names:
  la,qa,sa=a[n].decompose();lb,qb,sb=b[n].decompose();result[n]=Matrix.LocRotScale(la.lerp(lb,t),qa.slerp(qb,t),sa.lerp(sb,t))
 return result
def smooth(t):t=max(0,min(1,t));return t*t*(3-2*t)
clips={};meta={}
for label,index,inplace in [('idle',21,True),('walk',2,False),('start_original',3,True),('stop_original',4,True),('stop_alt_original',5,True)]:
 path=next(folder.glob(f'clip_{index:02}_*'));R,T,n,hz=decode(path);seq=[]
 for f in range(n):
  delta=T[f,0]-T[0,0] if inplace else np.zeros(3);delta[1]=0
  for k,name in enumerate(names):
   rot=C@Matrix(R[f,k].tolist())@C.transposed();skin=rot.to_4x4();skin.translation=center-rot@center+C@Vector(((T[f,k]-delta)*unit).tolist());r.pose.bones[name].matrix=inv@skin@rest[name];bpy.context.view_layer.update()
  seq.append(capture())
 clips[label]=seq;meta[label]={'frames':n,'hz':hz,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
def sample(label,phase):
 seq=clips[label];a=min(int(phase),len(seq)-1);b=min(a+1,len(seq)-1);return mix(seq[a],seq[b],phase-a)

for i in range(0,34,2):
 apply(clips['walk'][i]);print('FEET',i,[(side,list((world@r.pose.bones[side+'Foot'].matrix).translation)) for side in ['Left','Right']])
