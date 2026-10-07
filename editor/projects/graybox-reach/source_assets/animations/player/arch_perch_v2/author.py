"""Aletha three-contact arch perch: fixed left hand/feet, independent cannon aim.
Bank layout: ready/recoil, pitch (-60..60 by30), yaw (-80..80 by40).
Each sample is a deliberately held pose, runtime bilinearly samples aim angles.
"""
import bpy, json, math, struct, sys, os
variant=os.environ.get("PERCH_VARIANT","a").lower()
import numpy as np
from pathlib import Path
from mathutils import Matrix, Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent; project=out.parents[3]
review=project/'validation/arch-perch-v2';review.mkdir(exist_ok=True)
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
 if 'Leg' in upper:
  # Both segments share the knee hinge axis; independent shortest-arc rotations
  # otherwise twist the thigh against the pelvis when the knee is tightly folded.
  hinge=(elbow-start).cross(target-elbow).normalized()
  if upper.startswith('Right'):hinge=-hinge
  for n,head,tail in [(upper,start,elbow),(lower,elbow,target)]:
   y=(tail-head).normalized();x=y.cross(hinge).normalized()
   m=Matrix((x,y,hinge)).transposed().to_4x4();m.translation=head
   r.pose.bones[n].matrix=inv@m;bpy.context.view_layer.update()
 return (world@r.pose.bones[end].matrix).translation.copy()

# All contacts lie on the slanted face Y = 0.30*Z. Feet use the sole, hand uses the fist/palm.
hand=Vector((.26,.30*1.76,1.76))
lz,rz=(.59,.43) if variant=='a' else (.26,.16)
left=Vector((.16,.30*lz-.10,lz));right=Vector((-.13,.30*rz-.10,rz))
# Fit the complete flat sole, from rear heel through forefoot to toe.
sole_centers={};foot_rotations={};sole_bind_points={}
for side in ['Left','Right']:
 group=mesh.vertex_groups[side+'Foot'].index
 points=[mesh.matrix_world@v.co for v in mesh.data.vertices if any(g.group==group and g.weight>.5 for g in v.groups)]
 toe=sorted(points,key=lambda v:v.z)[:2]
 heel=sorted(points,key=lambda v:v.y)[-2:]
 # Include all six coplanar bottom vertices of the revised shoe.
 ball=[min(points,key=lambda v:v.x),max(points,key=lambda v:v.x)]
 pts=np.array([list(v) for v in points if abs(v.z)<.0001]);sole_bind_points[side]=pts.tolist()
 center_sole=Vector(pts.mean(0));_,_,vh=np.linalg.svd(pts-pts.mean(0));normal=Vector(vh[-1])
 if normal.z<0:normal=-normal
 forward=(sum(toe,Vector())-sum(heel,Vector())).normalized()
 forward=(forward-normal*forward.dot(normal)).normalized()
 across=forward.cross(normal).normalized()
 face_normal=Vector((0,-1,.30)).normalized()
 # Preserve the natural heel-to-toe direction; the sole alignment must
 # not introduce a half-turn around the contact normal.
 face_forward=Vector((.10 if side=='Left' else -.10,-.30,-1)).normalized()
 face_across=face_forward.cross(face_normal).normalized()
 rotation=Matrix((face_across,face_forward,face_normal)).transposed()@Matrix((across,forward,normal))
 sole_centers[side]=center_sole;foot_rotations[side]=rotation
 target=left if side=='Left' else right
 offset=rotation@(center_sole-rest[side+'Foot'].translation)
 target.y=.30*(target.z+offset.z)-offset.y-.001
# Preserve the approved knees; refit the foot itself to the revised flat sole.
leg_targets=json.loads((out/'approved_leg_targets.json').read_text()) if (out/'approved_leg_targets.json').exists() and variant=='a' else {}
poses=[];metrics=[]
for recoil in [0,.045]:
 for pitch in [-60,-30,0,30,60]:
  for yaw in [-80,-40,0,40,80]:
   for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4);b.rotation_mode='QUATERNION'
   bpy.context.view_layer.update();m=world@r.pose.bones['Hips'].matrix;m.translation+=Vector((-.02,.18,-.13) if variant=='a' else (-.04,.12,-.015));
   tilt=Matrix.Rotation(math.radians(67 if variant=='a' else 12),3,'Z');tr=m.translation.copy();m=(tilt@m.to_3x3()).to_4x4();m.translation=tr;r.pose.bones['Hips'].matrix=inv@m;bpy.context.view_layer.update()
   if variant=='a':
    # Keep the approved torso in world space while untwisting the pelvis beneath it.
    torso=r.pose.bones['Spine'].matrix.copy()
    m=world@r.pose.bones['Hips'].matrix;tr=m.translation.copy()
    m=(Matrix.Rotation(math.radians(25),3,'Z')@rest['Hips'].to_3x3()).to_4x4();m.translation=tr
    r.pose.bones['Hips'].matrix=inv@m;bpy.context.view_layer.update()
    r.pose.bones['Spine'].matrix=torso;bpy.context.view_layer.update()
   for side,target in [('Left',left),('Right',right)]:
    ik(side+'UpperLeg',side+'LowerLeg',side+'Foot',leg_targets.get(side,target),(target.x*1.15,-.55,target.z+.24))
    b=r.pose.bones[side+'Foot'];m=world@b.matrix;new=(foot_rotations[side]@rest[side+'Foot'].to_3x3()).to_4x4();new.translation=target;b.matrix=inv@new;bpy.context.view_layer.update()
   # Lift the clavicle with the overhead reach, opening the shoulder/armpit
   # without changing the approved chest, spine or head world transforms.
   if variant=='a':
    shoulder=(world@r.pose.bones['LeftUpperArm'].matrix).translation.copy()
    point('LeftShoulder',shoulder+Vector((.025,-.015,.085)))
   # Supporting arm remains independent of cannon aiming.
   ik('LeftUpperArm','LeftLowerArm','LeftHand',hand,(.65,.20,1.62))
   point('LeftHand',hand+Vector((0,.30,1))*.1)
   if variant=='a':
    # Small transverse volume correction for the raised support arm. Restore each
    # segment in world space so the elbow, wrist and stone contact cannot stretch.
    support={n:(world@r.pose.bones[n].matrix).copy() for n in ['LeftUpperArm','LeftLowerArm','LeftHand']}
    for n,width in [('LeftUpperArm',1.15),('LeftLowerArm',1.25),('LeftHand',1.0)]:
     m=support[n]
     if n=='LeftLowerArm':m=m@Matrix.Rotation(math.radians(25),4,'Y')
     r.pose.bones[n].matrix=inv@m@Matrix.Diagonal((width,1,width,1));bpy.context.view_layer.update()
   direction=Vector((math.sin(math.radians(yaw))*math.cos(math.radians(pitch)),-math.cos(math.radians(yaw))*math.cos(math.radians(pitch)),math.sin(math.radians(pitch))))
   shoulder=(world@r.pose.bones['RightUpperArm'].matrix).translation
   target=shoulder+direction*(.49-recoil)
   ik('RightUpperArm','RightLowerArm','RightHand',target,shoulder+Vector((-.3,0,-.6)))
   b=r.pose.bones['RightHand'];m=world@b.matrix;q=Vector((0,-1,0)).rotation_difference(direction)
   new=(q.to_matrix()@ready_hand).to_4x4();new.translation=m.translation;b.matrix=inv@new;bpy.context.view_layer.update()
   contacts=[(world@r.pose.bones[n].matrix).translation.copy() for n in ['LeftHand','LeftFoot','RightFoot']]
   metrics.append([list(p) for p in contacts]);poses.append({n:(r.pose.bones[n].location.copy(),r.pose.bones[n].rotation_quaternion.copy(),r.pose.bones[n].scale.copy()) for n in names})
assert np.max(np.ptp(np.array(metrics),axis=0))<1e-5
# Save one editable slotted action, bake the exact same skin transforms to PSXA.
a=bpy.data.actions.new('Aletha_ArchPerch_'+variant.upper()+'_AimBank');r.animation_data_create();r.animation_data.action=a
b=bytearray(b'PSXA'+struct.pack('<HHI4H',1,0,0,len(names),len(poses),30,0))
for frame,pose in enumerate(poses):
 s.frame_set(frame)
 for n in names:
  bone=r.pose.bones[n];bone.location,bone.rotation_quaternion,bone.scale=pose[n]
  for prop in ['location','rotation_quaternion','scale']:bone.keyframe_insert(prop,frame=frame)
 bpy.context.view_layer.update()
 for n in names:
  skin=(world@r.pose.bones[n].matrix)@rest[n].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C;tt=C.transposed()@(skin.translation-center+rot@center)/unit
  flat=[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)]
  b.extend(struct.pack('<9h3i',*flat,*[round(v) for v in tt]))
for fc in action_get_channelbag_for_slot(a,r.animation_data.action_slot).fcurves:
 for k in fc.keyframe_points:k.interpolation='CONSTANT';k.type='EXTREME'
struct.pack_into('<I',b,8,len(b)-12)
assets=project/'assets/animations/arch_perch_v2';assets.mkdir(exist_ok=True);(assets/f'perch_{variant}.psxanim').write_bytes(b)
# Sloped contact masonry used to review the pose from three directions.
for ob in s.objects:
 if ob.type=='MESH':ob.hide_render=ob!=mesh;ob.hide_set(ob!=mesh)
faceverts=[(x,.3*z+depth,z) for depth in [0,.28] for x,z in [(-.58,-.1),(.60,-.1),(.52,2.0),(-.48,2.0)]]
me=bpy.data.meshes.new('Perch slanted face');me.from_pydata(faceverts,[],[(0,1,2,3),(4,7,6,5),(0,4,5,1),(3,2,6,7),(0,3,7,4),(1,5,6,2)]);me.update();ob=bpy.data.objects.new('Perch slanted face',me);s.collection.objects.link(ob);ob.color=(.20,.23,.25,1)
# Cannon preview follows the same bind-space attachment basis as the shipped weapon.
sys.path.insert(0,str(out.parent/'zenith_ranged_v1'));from cannon_mesh import cannon_mesh
cv,cf=cannon_mesh();cm=bpy.data.meshes.new('Perch cannon');cm.from_pydata(cv,[],cf);cm.update();co=bpy.data.objects.new('Perch cannon',cm);s.collection.objects.link(co);co.color=(.10,.65,.56,1)
s.frame_set(7);bpy.context.view_layer.update();socket=json.loads((out.parent/'zenith_ranged_v1/socket.json').read_text());grip=Vector(socket['grip'])
from mathutils import Euler
basis=Euler(tuple(v*math.tau/4096 for v in socket['rotation_q12']),'XYZ').to_matrix()
co.matrix_world=(world@r.pose.bones['RightHand'].matrix)@rest['RightHand'].inverted()@Matrix.Translation(C@grip*unit+center)@(C@basis).to_4x4()@Matrix.Diagonal((unit,unit,unit,1));m=co.matrix_world.copy();co.parent=r;co.parent_type='BONE';co.parent_bone='RightHand';co.matrix_world=m
mesh.color=(.56,.79,.84,1);s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=560;s.render.resolution_y=640;s.render.resolution_percentage=100
s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard';s.camera.data.type='ORTHO';s.camera.data.ortho_scale=2.5
for view,eye in [('front',(2,-5,2.8)),('side',(5,-1,2.4)),('high',(-3,-4,4))]:
 s.camera.location=eye;focus=Vector((0,.08,.94));s.camera.rotation_euler=(focus-s.camera.location).to_track_quat('-Z','Y').to_euler()
 for frame in [7,5,9,12,32]:
  s.frame_set(frame);s.render.filepath=str(review/f'{view}-{variant}-{frame}.png');bpy.ops.render.render(write_still=True)
s.frame_start=0;s.frame_end=49;s.frame_set(7);bpy.ops.wm.save_as_mainfile(filepath=str(out/f'arch-perch-{variant}.blend'))
# Contact coordinate convention: raw model origin plus bind floor lift, engine-rescaled.
def engine_point(p):
 v=C.transposed()@(Vector(p)-center);v.y+=-raw[:,1].min()*unit
 return [round(x*engine_per_m) for x in v]
report={'frames':50,'bytes':len(b),'engine_units_per_m':engine_per_m,'hand_local_engine':engine_point(hand),'foot_local_engine':[engine_point(left),engine_point(right)],'sole_local_engine':[engine_point(target+foot_rotations[side]@(sole_centers[side]-rest[side+'Foot'].translation)) for side,target in [('Left',left),('Right',right)]],'contact_bind_engine':{n:[round(v/unit/16) for v in C.transposed()@(rest[n].translation-center)] for n in ['LeftHand','LeftFoot','RightFoot']},'sole_alignment':'rear heel and broad forefoot support plane, heel above toe, 1mm clearance','sole_bind_points':sole_bind_points,'contact_drift_m':float(np.max(np.ptp(np.array(metrics),axis=0))),'layout':'2 recoil banks x 5 pitches x 5 yaws; yaw -80..80 step40, pitch -60..60 step30','beat_plan':'Held brace; 3-tick recoil accent, then held recovery; contact keyframes remain fixed'}
(out/f'animation-{variant}.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT',json.dumps(report))
