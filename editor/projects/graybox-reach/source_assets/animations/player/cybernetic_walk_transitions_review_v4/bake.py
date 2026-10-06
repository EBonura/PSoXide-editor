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

# Export the approved world-space skin matrices in the frozen cooked model's
# coordinate frame. Generic GLB retargeting changes the recovered bind offsets.
# The source model uses sixteen times the generated runtime coordinate scale.
raw_reference=project/'assets/animations/gen/walk_fwd.psxanim'
R0,T0,_,_=decode(raw_reference);R1,T1,_,_=decode(next(folder.glob('clip_02_*')))
assert np.max(np.abs(T0[0]/16-T1[0]))<2
report={}
for stem in ['walk_fwd_windup','walk_fwd_winddown','walk_fwd_winddown_mirror']:
 bpy.ops.wm.open_mainfile(filepath=str(out/(stem+'.blend')))
 s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');n=s.frame_end+1
 # Preserve a known-valid v1 animation header, replacing the sample counts.
 template=(project/'assets/animations/gen'/ (stem+'.psxanim')).read_bytes()
 header=bytearray(template[:20]);struct.pack_into('<H',header,4,1);struct.pack_into('<4H',header,12,len(names),n+1,30,0)
 payload=bytearray(header)
 for f in range(n):
  s.frame_set(f)
  for name in names:
   skin=(r.matrix_world@r.pose.bones[name].matrix)@rest[name].inverted()
   rot=skin.to_3x3();rotation=C.transposed()@rot@C
   translation=C.transposed()@(skin.translation-center+rot@center)/unit*16
   flat=[round(rotation[row][col]*4096) for col in range(3) for row in range(3)]
   payload.extend(struct.pack('<9h3i',*flat,*[round(v) for v in translation]))
 # Runtime sampler excludes its final sentinel even for non-looping actions.
 payload.extend(payload[-len(names)*30:])
 struct.pack_into('<I',payload,8,len(payload)-12)
 dest=project/'assets/animations/gen'/(stem+'.psxanim');dest.write_bytes(payload)
 report[stem]={'frames':n+1,'authored_frames':n,'hz':30,'bytes':len(payload),'sha256':hashlib.sha256(payload).hexdigest()}
(out/'bake-validation.json').write_text(json.dumps(report,indent=2)+'\n')
print('RESULT authored matrices baked',json.dumps(report))
