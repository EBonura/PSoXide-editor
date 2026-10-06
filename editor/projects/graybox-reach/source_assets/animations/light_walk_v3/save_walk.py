import bpy,json,struct,math
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector
P=Path(__file__).resolve().parents[3];OUT=Path(__file__).resolve().parent;T=P/'tools/enemy_reduction';__file__=str(T/'validate_render.py');exec((T/'validate_render.py').read_text().split('report={}')[0]);data=np.load(OUT/'walk.npz');bind=data['bind'];parents=data['parents'];skin=np.concatenate([data['skin'],data['skin'][:1]]);m=model(P/'assets/models/light_body_v1/light.psxmdl');C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted();names=['Hips','Spine','Spine1','Spine2','Neck','Head','Shoulder.L','Arm.L','Forearm.L','Claw.L','Shoulder.R','Arm.R','Forearm.R','Cannon.R','Thigh.L','Shin.L','Foot.L','Toe.L','Thigh.R','Shin.R','Foot.R','Toe.R']
arm=bpy.data.armatures.new('Mantis runtime skeleton');rig=bpy.data.objects.new('Mantis body rig',arm);s.collection.objects.link(rig);bpy.context.view_layer.objects.active=rig;rig.select_set(True);bpy.ops.object.mode_set(mode='EDIT')
for j,name in enumerate(names):
 b=arm.edit_bones.new(name);b.head=Vector((0,0,0));b.tail=Vector((0,0,.15));mat=C@Matrix(bind[j].tolist());rot=mat.to_3x3().normalized().to_4x4();rot.translation=mat.translation;b.matrix=rot;b.length=.15
 if parents[j]>=0:b.parent=arm.edit_bones[names[parents[j]]]
bpy.ops.object.mode_set(mode='OBJECT');ob=mesh(m,'Mantis body 457');ob.data.transform(C)
for name in names:ob.vertex_groups.new(name=name)
for i,v in enumerate(m['v']):
 ob.vertex_groups[names[m['owner'][i]]].add([i],(255-v[4])/255,'REPLACE')
 if v[4]:ob.vertex_groups[names[v[3]]].add([i],v[4]/255,'ADD')
mod=ob.modifiers.new('Runtime skin weights','ARMATURE');mod.object=rig;ob.parent=rig
for frame in range(len(skin)):
 s.frame_set(frame+1)
 for j,name in enumerate(names):
  pb=rig.pose.bones[name];pb.rotation_mode='QUATERNION';pb.matrix=C@Matrix(skin[frame,j].tolist())@CI@pb.bone.matrix_local;bpy.context.view_layer.update();pb.keyframe_insert(data_path='location',frame=frame+1);pb.keyframe_insert(data_path='rotation_quaternion',frame=frame+1);pb.keyframe_insert(data_path='scale',frame=frame+1)
act=rig.animation_data.action;act.name='Stalking walk v3';act.use_fake_user=True
from bpy_extras.anim_utils import action_get_channelbag_for_slot
bag=action_get_channelbag_for_slot(act,rig.animation_data.action_slot)
for fc in bag.fcurves:
 for k in fc.keyframe_points:k.interpolation='LINEAR'
 fc.modifiers.new('CYCLES')
s.frame_start=1;s.frame_end=len(skin)-1;s.render.fps=30;rig['joint_order']=json.dumps(names);rig['runtime_frames']=len(skin);rig['export_note']='Stalking walk v3, 30 samples per second. PSXA export includes a duplicate endpoint.'
# Verify actual armature deformation against the intended runtime skinning.
err=0
for fi in [0,6,12,18,23]:
 s.frame_set(fi+1);bpy.context.view_layer.update();ev=ob.evaluated_get(bpy.context.evaluated_depsgraph_get());me=ev.to_mesh();target=deform(m,[(a[:3,:3],a[:3,3]) for a in skin[fi]]);target=np.array([(C@Vector(tuple(v)+(1,)))[:3] for v in target]);err=max(err,float(np.linalg.norm(np.array([v.co[:] for v in me.vertices])-target,axis=1).max()));ev.to_mesh_clear()
assert err<.001,err
s.frame_set(1);cam.location=(4,-7,2);cam.rotation_euler=(Vector((0,0,0))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=4.8
for screen in bpy.data.screens:
 for area in screen.areas:
  if area.type=='VIEW_3D':area.spaces.active.shading.type='MATERIAL';area.spaces.active.region_3d.view_distance=5
bpy.ops.object.select_all(action='DESELECT');ob.select_set(True);bpy.context.view_layer.objects.active=ob;rig.hide_set(True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'walk.blend'));print('RESULT rig deformation max error',err)
