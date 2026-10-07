"""Bake Aletha's ready/fire poses and aiming gaits onto the existing 26-joint bind.
Run headless Blender with factory startup. Re-running replaces this generated study.
"""
import bpy, sys, struct, json, math, re
import numpy as np
from pathlib import Path
from mathutils import Matrix, Vector, Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent
project=out.parents[3]
sys.path.insert(0,str(out))
sys.path.insert(0,str(out.parent/'cybernetic_walk_transitions_review_v4'))

bpy.ops.wm.open_mainfile(filepath=str(out/'hook-launch.blend'))
s=bpy.context.scene
r=next(o for o in s.objects if o.type=='ARMATURE')


bpy.context.view_layer.update()
names=[b.name for b in r.data.bones]
assert names==['Hips','LeftUpperLeg','LeftLowerLeg','LeftFoot','RightUpperLeg','RightLowerLeg','RightFoot','Spine','Chest','Neck','RightShoulder','RightUpperArm','RightLowerArm','RightHand','RightThumbMetacarpal','RightThumbProximal','RightIndexProximal','RightIndexIntermediate','LeftShoulder','LeftUpperArm','LeftLowerArm','LeftHand','LeftThumbMetacarpal','LeftThumbProximal','LeftIndexProximal','LeftIndexIntermediate']
world=r.matrix_world.copy(); inv=world.inverted(); rest={n:world@r.data.bones[n].matrix_local for n in names}
# PSX bind coordinates are normalized about the model centre; Blender is Z-up.
blob=(project/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes()
j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16
raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float)
mesh=bpy.data.objects['Aletha optimized']
verts=np.array([mesh.matrix_world@v.co for v in mesh.data.vertices])
C=Matrix(((1,0,0),(0,0,-1),(0,1,0)))
rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=np.ptp(verts[:,2])/np.ptp(rv[:,2])
center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())

data=bytearray((project/'assets/animations/zenith_ranged_v1/aim.psxanim').read_bytes()[:20])
struct.pack_into('<H',data,4,1);struct.pack_into('<4H',data,12,len(names),11,50,0)
for frame in range(11):
 s.frame_set(frame);bpy.context.view_layer.update()
 for name in names:
  skin=(world@r.pose.bones[name].matrix)@rest[name].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C
  tt=C.transposed()@(skin.translation-center+rot@center)/unit
  flat=[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)]
  data.extend(struct.pack('<9h3i',*flat,*[round(v) for v in tt]))
struct.pack_into('<I',data,8,len(data)-12)
target=project/'assets/animations/hook_launch_v1/launch.psxanim';target.write_bytes(data)
s.frame_start=0;s.frame_end=10;s.render.fps=50;s.frame_set(0)
for ob in list(s.objects):
 if ob.type=='ARMATURE' and ob!=r:bpy.data.objects.remove(ob,do_unlink=True)
for a in list(bpy.data.actions):
 if a!=r.animation_data.action:bpy.data.actions.remove(a)
bpy.ops.wm.save_as_mainfile(filepath=str(out/'hook-launch.blend'))
report={'source':'Universal Animation Library Standard / Jump_Start','source_frames':[0,10],'source_fps':30,'playback_hz':50,'frames':11,'bytes':len(data),'joints':len(names),'purpose':'crouched anticipation to raised-knee takeoff; hold final pose during flight'}
(out/'animation.json').write_text(json.dumps(report,indent=2)+'\n')
print('RESULT',json.dumps(report))
