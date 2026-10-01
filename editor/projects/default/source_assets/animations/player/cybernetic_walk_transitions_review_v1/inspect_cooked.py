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

clip=next(folder.glob('clip_02_*.psxanim'));R,T,n,hz=decode(clip)
for k,name in enumerate(names):
 rot=C@Matrix(R[0,k].tolist())@C.transposed();skin=rot.to_4x4();skin.translation=center-rot@center+C@Vector((T[0,k]*unit).tolist());r.pose.bones[name].matrix=inv@skin@rest[name];bpy.context.view_layer.update()
pose=lambda:{n:list((world@r.pose.bones[n].matrix).translation) for n in ['Hips','LeftUpperLeg','LeftLowerLeg','LeftFoot','RightUpperLeg','RightLowerLeg','RightFoot','Chest','Neck']}
print('IDLE',json.dumps(pose()))
bpy.ops.wm.open_mainfile(filepath=str(out.parent/'cybernetic_walk/walk_fwd.blend'));r=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');world=r.matrix_world.copy()
for f in [0,10,21,31,42]:
 bpy.context.scene.frame_set(f);print('WALK',f,json.dumps(pose()))
