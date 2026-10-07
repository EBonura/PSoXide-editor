"""Keep the launch anticipation; author a long, asymmetric flight silhouette."""
import bpy, sys, struct, json, math, re
import numpy as np
from pathlib import Path
from mathutils import Matrix, Vector, Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent
project=out.parents[3]
sys.path.insert(0,str(out))
sys.path.insert(0,str(out.parent/'cybernetic_walk_transitions_review_v4'))

bpy.ops.wm.open_mainfile(filepath=str(out.parent/'hook_launch_v1/hook-launch.blend'))
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

print('BLENDER_VERSION',bpy.app.version_string)
def capture():
 return {n:(r.pose.bones[n].location.copy(),r.pose.bones[n].rotation_quaternion.copy(),r.pose.bones[n].scale.copy()) for n in names}
source=[]
for f in range(11):
 s.frame_set(f);bpy.context.view_layer.update();source.append(capture())
r.animation_data_clear()
for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4);b.rotation_mode='QUATERNION'
bpy.context.view_layer.update()
# Pitch the torso into the ascent, keep legs long and trailing behind it.
hips=r.pose.bones['Hips'];m=world@hips.matrix
rot=Matrix.Rotation(math.radians(24),4,'X');posed=rot@m;posed.translation=m.translation+Vector((0,0,.10));hips.matrix=inv@posed
bpy.context.view_layer.update()
def point(name,direction):
 b=r.pose.bones[name];m=world@b.matrix
 q=(m.to_3x3()@Vector((0,1,0))).rotation_difference(Vector(direction).normalized())
 new=(q.to_matrix()@m.to_3x3()).to_4x4();new.translation=m.translation
 b.matrix=inv@new;bpy.context.view_layer.update()
# Knees remain almost straight: hips, knees and feet form two long trailing lines.
point('LeftUpperLeg',(.045,.22,-1));point('LeftLowerLeg',(.025,.34,-1))
point('RightUpperLeg',(-.065,.31,-1));point('RightLowerLeg',(-.025,.39,-1))
point('LeftFoot',(0,-.25,-1));point('RightFoot',(0,-.25,-1))
# One arm leads toward the destination; the other streams beside the torso.
point('RightUpperArm',(-.16,-.68,.80));point('RightLowerArm',(-.05,-.63,.9));point('RightHand',(0,-.63,.9))
point('LeftUpperArm',(.22,.36,-.90));point('LeftLowerArm',(.09,.50,-1));point('LeftHand',(0,.5,-1))
flight=capture()
action=bpy.data.actions.new('Aletha_Hook_Launch_And_Flight_v2');r.animation_data_create();r.animation_data.action=action
frames=36;fps=50
data=bytearray((project/'assets/animations/zenith_ranged_v1/aim.psxanim').read_bytes()[:20]);struct.pack_into('<H',data,4,1);struct.pack_into('<4H',data,12,len(names),frames,fps,0)
metrics=[]
for f in range(frames):
 s.frame_set(f)
 # A quick extension after the first 0.12 s; fully extended when wire travel begins.
 t=max(0,min(1,(f-5)/7));t=t*t*(3-2*t)
 original=source[min(f,10)]
 for n in names:
  b=r.pose.bones[n];a=original[n];z=flight[n]
  b.location=a[0].lerp(z[0],t);b.rotation_quaternion=a[1].slerp(z[1],t);b.scale=a[2].lerp(z[2],t)
  # Small trailing-arm follow-through, no repeated squat or leg pedalling.
  if n=='LeftLowerArm' and f>=12:b.rotation_quaternion=b.rotation_quaternion@Quaternion((1,0,0),math.radians(3)*math.sin((f-12)/23*math.pi))
  b.keyframe_insert('location',frame=f);b.keyframe_insert('rotation_quaternion',frame=f);b.keyframe_insert('scale',frame=f)
 bpy.context.view_layer.update()
 for n in names:
  skin=(world@r.pose.bones[n].matrix)@rest[n].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C
  tt=C.transposed()@(skin.translation-center+rot@center)/unit
  flat=[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)]
  data.extend(struct.pack('<9h3i',*flat,*[round(v) for v in tt]))
 if f>=12:
  for side in ['Left','Right']:
   a,b,c=[(world@r.pose.bones[side+joint].matrix).translation for joint in ['UpperLeg','LowerLeg','Foot']]
   metrics.append((c-a).length/((b-a).length+(c-b).length))
cb=action_get_channelbag_for_slot(action,r.animation_data.action_slot)
for fc in cb.fcurves:
 for k in fc.keyframe_points:k.interpolation='LINEAR'
struct.pack_into('<I',data,8,len(data)-12)
(project/'assets/animations/hook_flight_v2/launch.psxanim').write_bytes(data)
s.frame_start=0;s.frame_end=35;s.render.fps=fps;s.frame_set(12)
for ob in s.objects:
 if ob.type=='MESH':ob.hide_render=ob!=mesh;ob.hide_set(ob!=mesh)
mesh.hide_render=False;mesh.hide_set(False);mesh.color=(.44,.73,.8,1)
s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=400;s.render.resolution_y=480;s.render.resolution_percentage=100
s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard'
s.camera.data.type='ORTHO';s.camera.data.ortho_scale=2.7
review=project.parents[2]/'build/graybox-reach/hooks/flight-v2'
for view,eye in [('back',(3,5,2.4)),('side',(5,0,2)),('front',(3,-5,2.4))]:
 s.camera.location=eye;focus=Vector((0,0,1));s.camera.rotation_euler=(focus-s.camera.location).to_track_quat('-Z','Y').to_euler()
 for frame in [0,6,12,25]:
  s.frame_set(frame);s.render.filepath=str(review/f'{view}-{frame}.png');bpy.ops.render.render(write_still=True)
s.frame_set(12)
for a in list(bpy.data.actions):
 if a!=action:bpy.data.actions.remove(a)
bpy.ops.wm.save_as_mainfile(filepath=str(out/'hook-flight.blend'))
assert min(metrics)>.98
report={'frames':frames,'hz':fps,'bytes':len(data),'flight_straight_leg_ratio_min':min(metrics),'source':'hook_launch_v1 anticipation; authored extended flight pose','extension_complete_frame':12}
(out/'animation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT',json.dumps(report))
