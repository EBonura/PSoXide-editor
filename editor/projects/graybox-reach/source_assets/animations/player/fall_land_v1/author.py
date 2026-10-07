"""Aletha arch release and planted landing; physics owns vertical travel."""
import bpy, json, math, struct, sys
import numpy as np
from pathlib import Path
from mathutils import Matrix, Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent; project=out.parents[3]
review=project/'validation/fall-land-v1';review.mkdir(exist_ok=True)
print('BLENDER_VERSION',bpy.app.version_string)
bpy.ops.wm.open_mainfile(filepath=str(out.parent/'zenith_ranged_v1/zenith-ranged.blend'))
r=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
r.animation_data.action=bpy.data.actions['Aletha_Zenith_aim'];bpy.context.scene.frame_set(0);bpy.context.view_layer.update()
ready_hand=(r.matrix_world@r.pose.bones['RightHand'].matrix).to_3x3().copy()
bpy.ops.wm.open_mainfile(filepath=str(project/'source_assets/characters/aletha_closed_458/Aletha-closed-458.blend'))
s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');mesh=bpy.data.objects['Aletha optimized']
r.animation_data_clear();names=[b.name for b in r.data.bones];world=r.matrix_world.copy();inv=world.inverted()
rest={n:world@r.data.bones[n].matrix_local for n in names}
blob=(project/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes()
j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16
raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float)
verts=np.array([mesh.matrix_world@v.co for v in mesh.data.vertices]);C=Matrix(((1,0,0),(0,0,-1),(0,1,0)))
rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=np.ptp(verts[:,2])/np.ptp(rv[:,2])
center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())
engine_per_m=98/4096/16/unit

def point(n,target):
 b=r.pose.bones[n];m=world@b.matrix;direction=m.to_3x3()@Vector((0,1,0))
 q=direction.rotation_difference(Vector(target)-m.translation)
 new=(q.to_matrix()@m.to_3x3()).to_4x4();new.translation=m.translation;b.matrix=inv@new;bpy.context.view_layer.update()
def ik(upper,lower,end,target,pole):
 start=(world@r.pose.bones[upper].matrix).translation;target=Vector(target);axis=(target-start).normalized()
 a=r.data.bones[upper].length;b=r.data.bones[lower].length;distance=(target-start).length
 assert distance<(a+b)*.998,(upper,distance,a+b)
 along=(a*a-b*b+distance*distance)/(2*distance);bend=Vector(pole)-start;bend=(bend-axis*bend.dot(axis)).normalized()
 elbow=start+axis*along+bend*math.sqrt(max(0,a*a-along*along))
 point(upper,elbow);point(lower,target)
 return (world@r.pose.bones[end].matrix).translation.copy()


def capture():
 return {n:(r.pose.bones[n].location.copy(),r.pose.bones[n].rotation_quaternion.copy(),r.pose.bones[n].scale.copy()) for n in names}
def restore(pose):
 for n,(loc,rot,scale) in pose.items():
  bone=r.pose.bones[n];bone.rotation_mode='QUATERNION';bone.location=loc;bone.rotation_quaternion=rot;bone.scale=scale
 bpy.context.view_layer.update()
def mix(a,b,t):
 return {n:(a[n][0].lerp(b[n][0],t),a[n][1].slerp(b[n][1],t),a[n][2].lerp(b[n][2],t)) for n in names}
def body(drop,air=False,bend=0):
 for bone in r.pose.bones:bone.matrix_basis=Matrix.Identity(4);bone.rotation_mode='QUATERNION'
 bpy.context.view_layer.update();m=world@r.pose.bones['Hips'].matrix;m.translation+=Vector((0,.035,-drop));r.pose.bones['Hips'].matrix=inv@m;bpy.context.view_layer.update()
 # Chest leans into the landing while the head trails the compression.
 n='Chest';m=world@r.pose.bones[n].matrix;tr=m.translation.copy();m=(Matrix.Rotation(math.radians(bend),3,'X')@m.to_3x3()).to_4x4();m.translation=tr;r.pose.bones[n].matrix=inv@m;bpy.context.view_layer.update()
 for side,extra in [('Left',.12 if air else 0),('Right',.02 if air else 0)]:
  target=rest[side+'Foot'].translation.copy();target.x*=1.18;target.y-=.08 if side=='Left' else -.02;target.z+=extra
  ik(side+'UpperLeg',side+'LowerLeg',side+'Foot',target,(target.x*1.2,-.6,target.z+.3))
  n=side+'Foot';m=rest[n].copy();m.translation=(world@r.pose.bones[n].matrix).translation;r.pose.bones[n].matrix=inv@m;bpy.context.view_layer.update()
 for side in ['Left','Right']:
  shoulder=(world@r.pose.bones[side+'UpperArm'].matrix).translation
  direction=Vector((.10 if side=='Left' else -.10,-.16,-.52)).normalized()
  target=shoulder+direction*(.515 if air else .49)
  ik(side+'UpperArm',side+'LowerArm',side+'Hand',target,shoulder+Vector((.10 if side=='Left' else -.10,-.36,-.4)))
  n=side+'Hand';m=world@r.pose.bones[n].matrix
  if side=='Right':
   q=Vector((0,-1,0)).rotation_difference(direction);basis=q.to_matrix()@ready_hand
  else:basis=rest[n].to_3x3()
  new=basis.to_4x4();new.translation=m.translation;r.pose.bones[n].matrix=inv@new;bpy.context.view_layer.update()
 return capture()
# Import the selected compact brace as the first release pose.
with bpy.data.libraries.load(str(out.parent/'arch_perch_v2/arch-perch-a.blend')) as (fr,to):
 to.actions=[name for name in fr.actions if name=='Aletha_ArchPerch_A_AimBank']
a=to.actions[0];r.animation_data_create();r.animation_data.action=a;s.frame_set(7);bpy.context.view_layer.update();release=capture();r.animation_data_clear()
fall=body(.085,True,4)
contact=body(.025,False,3)
compress=body(.30,False,19)
rebound=body(.09,False,8)
settle=body(.025,False,0)
# 30 fps: release into a relaxed fall at frame 7. The non-looping clip
# clamps there for long falls, so duplicate held samples need not occupy RAM.
# Landing: contact 0, compression 3, rebound 7, settle 12. Ground contact is never eased.
fallposes=[mix(release,fall,min(1,f/7)) for f in range(8)]
landposes=[]
for f in range(13):
 if f<=3:p=mix(contact,compress,f/3)
 elif f<=7:p=mix(compress,rebound,(f-3)/4)
 else:p=mix(rebound,settle,(f-7)/5)
 landposes.append(p)
assets=project/'assets/animations/fall_land_v1';assets.mkdir(exist_ok=True)
measurements={}
for label,poses in [('fall',fallposes),('land',landposes)]:
 contacts=[]
 r.animation_data_create();action=bpy.data.actions.new('Aletha_'+label.title());r.animation_data.action=action;action.use_fake_user=True
 b=bytearray(b'PSXA'+struct.pack('<HHI4H',1,0,0,len(names),len(poses),30,0))
 for frame,pose in enumerate(poses):
  s.frame_set(frame);restore(pose)
  if label=='land':
   for side in ['Left','Right']:
    target=rest[side+'Foot'].translation.copy();target.x*=1.18;target.y-=.08 if side=='Left' else -.02
    ik(side+'UpperLeg',side+'LowerLeg',side+'Foot',target,(target.x*1.2,-.6,target.z+.3))
    n=side+'Foot';m=rest[n].copy();m.translation=target;r.pose.bones[n].matrix=inv@m;bpy.context.view_layer.update()
  contacts.append([list((world@r.pose.bones[n].matrix).translation) for n in ['LeftFoot','RightFoot']])
  for n in names:
   bone=r.pose.bones[n]
   for prop in ['location','rotation_quaternion','scale']:bone.keyframe_insert(prop,frame=frame)
  bpy.context.view_layer.update()
  for n in names:
   skin=(world@r.pose.bones[n].matrix)@rest[n].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C;tt=C.transposed()@(skin.translation-center+rot@center)/unit
   flat=[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)]
   b.extend(struct.pack('<9h3i',*flat,*[round(v) for v in tt]))
 for fc in action_get_channelbag_for_slot(action,r.animation_data.action_slot).fcurves:
  for k in fc.keyframe_points:k.interpolation='LINEAR';k.type='EXTREME'
 struct.pack_into('<I',b,8,len(b)-12)
 if label=='fall':
  # Match the shipped brace exactly, independent of tiny quantizer refits after mesh edits.
  perch=(project/'assets/animations/arch_perch_v2/perch_a.psxanim').read_bytes();stride=len(names)*30
  assert struct.unpack_from('<H',perch,12)[0]==len(names)
  b[20:20+stride]=perch[20+7*stride:20+8*stride]
 (assets/f'{label}.psxanim').write_bytes(b)
 for ob in s.objects:
  if ob.type=='MESH':ob.hide_render=ob!=mesh;ob.hide_set(ob!=mesh)
 mesh.color=(.56,.79,.84,1);s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=480;s.render.resolution_y=600;s.render.resolution_percentage=100
 s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard';s.camera.data.type='ORTHO';s.camera.data.ortho_scale=2.35
 s.camera.location=(3,-5,2.6);focus=Vector((0,0,.94));s.camera.rotation_euler=(focus-s.camera.location).to_track_quat('-Z','Y').to_euler()
 for frame in ([0,3,7] if label=='fall' else [0,3,7,12]):
  s.frame_set(frame);s.render.filepath=str(review/f'{label}-{frame}.png');bpy.ops.render.render(write_still=True)
 s.frame_start=0;s.frame_end=len(poses)-1;s.frame_set(0);bpy.ops.wm.save_as_mainfile(filepath=str(out/f'{label}.blend'))
 measurements[label]={'frames':len(poses),'bytes':len(b),'foot_drift_m':float(np.max(np.ptp(np.array(contacts),axis=0)))}
 if label=='land':assert measurements[label]['foot_drift_m']<1e-5
 print('EXPORTED',label,len(poses),len(b))
(out/'beats.json').write_text(json.dumps({'fps':30,'fall':{'release':0,'arms_release':3,'settled_fall':7,'held_end':7},'landing':{'contact':0,'compression':3,'rebound':7,'settle':12},'root_motion':'Physics owns vertical translation; authored clips provide body articulation only'},indent=2)+'\n')

(out/'validation.json').write_text(json.dumps(measurements,indent=2)+'\n')
