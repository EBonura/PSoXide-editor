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

bpy.ops.wm.open_mainfile(filepath=str(project/'source_assets/characters/aletha_closed_458/Aletha-closed-458.blend'))
s=bpy.context.scene
r=next(o for o in s.objects if o.type=='ARMATURE')
r.animation_data_clear()
for b in r.pose.bones: b.matrix_basis=Matrix.Identity(4)
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

sys.path.insert(0,str(project.parents[2]/'tools'))
import types
bridge=types.ModuleType("hook_retarget")
bridge_source=(project.parents[2]/"tools/aletha_bvh_retarget.py").read_text().replace("action.fcurves", "action.layers[0].strips[0].channelbag(action.slots[0]).fcurves")
exec(compile(bridge_source,"aletha_bvh_retarget.py","exec"),bridge.__dict__)
print('BLENDER_VERSION',bpy.app.version_string)
s.render.fps=30
source_path=Path('/Users/ebonura/Downloads/Universal Animation Library[Standard]/Unreal-Godot/UAL1_Standard.glb') if '--ual' in sys.argv else out/'jumping up.fbx'
src=bridge.import_animation_source(source_path,'Jump_Start' if '--ual' in sys.argv else None)
a,b=map(round,src.animation_data.action.frame_range)
action,speed=bridge.retarget(src,r,'HookLaunch',a,b,smooth=False)
print('TAKE',a,b)
for ob in s.objects:
 if ob.type=='MESH':ob.hide_render=ob!=mesh;ob.hide_set(ob!=mesh)
mesh.hide_render=False;mesh.hide_set(False);mesh.color=(.44,.73,.8,1)
s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=400;s.render.resolution_y=480;s.render.resolution_percentage=100
s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard'
s.camera.location=(3,5,2.4);focus=Vector((0,0,1));s.camera.rotation_euler=(focus-s.camera.location).to_track_quat('-Z','Y').to_euler();s.camera.data.type='ORTHO';s.camera.data.ortho_scale=2.8
label='ual' if '--ual' in sys.argv else 'adventure'
review=project.parents[2]/'build/graybox-reach/hooks/launch'
for i,frame in enumerate([a,a+(b-a)//4,a+(b-a)//2,a+3*(b-a)//4,b]):
 s.frame_set(frame);s.render.filepath=str(review/f'{label}-{i}.png');bpy.ops.render.render(write_still=True)
s.frame_start=a;s.frame_end=b
bpy.ops.wm.save_as_mainfile(filepath=str(out/f'{label}.blend'))
print('RESULT',json.dumps({'source':str(source_path),'range':[a,b]}))
