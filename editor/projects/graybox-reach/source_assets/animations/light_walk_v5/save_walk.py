"""Refresh the editable walk on its packed rig; optionally render --render."""
import ast,json,sys,struct
from pathlib import Path
import bpy
import numpy as np
from mathutils import Matrix,Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
OUT=Path(__file__).resolve().parent;P=OUT.parents[2]
tree=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'model','deform'}],type_ignores=[]),'skin helpers','exec'))
bpy.ops.wm.open_mainfile(filepath=str(OUT/'walk.blend'))
s=bpy.context.scene;rig=bpy.data.objects['Mantis body rig'];ob=bpy.data.objects['Mantis body 457'];names=json.loads(rig['joint_order']);rig.animation_data_clear();rig.hide_set(False)
data=np.load(OUT/'walk.npz');skin=np.concatenate([data['skin'],data['skin'][:1]]);m=model(P/'assets/models/light_body_v1/light.psxmdl');C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted();last={}
for fi,frame in enumerate(skin):
 s.frame_set(fi+1)
 for j,name in enumerate(names):
  pb=rig.pose.bones[name];pb.rotation_mode='QUATERNION';pb.matrix=C@Matrix(frame[j].tolist())@CI@pb.bone.matrix_local
  if name in last and pb.rotation_quaternion.dot(last[name])<0:pb.rotation_quaternion.negate()
  last[name]=pb.rotation_quaternion.copy();bpy.context.view_layer.update()
  for channel in ['location','rotation_quaternion','scale']:pb.keyframe_insert(data_path=channel,frame=fi+1)
act=rig.animation_data.action;act.name='Asymmetric stalking walk v5 / flat soles';act.use_fake_user=True
for fc in action_get_channelbag_for_slot(act,rig.animation_data.action_slot).fcurves:
 for key in fc.keyframe_points:key.interpolation='LINEAR'
 fc.modifiers.new('CYCLES')
s.frame_start=1;s.frame_end=len(skin)-1;s.render.fps=30;rig['runtime_frames']=len(skin);rig['export_note']='Flat planted soles; asymmetric stalking walk at 30 Hz, 24 unique samples plus loop endpoint.'
err=0.
for fi,frame in enumerate(skin):
 s.frame_set(fi+1);bpy.context.view_layer.update();ev=ob.evaluated_get(bpy.context.evaluated_depsgraph_get());me=ev.to_mesh();target=deform(m,[(a[:3,:3],a[:3,3]) for a in frame]);target=np.array([(C@Vector(tuple(v)+(1,)))[:3] for v in target]);err=max(err,float(np.linalg.norm(np.array([v.co[:] for v in me.vertices])-target,axis=1).max()));ev.to_mesh_clear()
assert err<.001,err
rig.hide_set(True);s.frame_set(1);s.render.engine='BLENDER_EEVEE';s.eevee.taa_render_samples=8;s.render.resolution_x=480;s.render.resolution_y=640;s.render.resolution_percentage=100;s.render.image_settings.file_format='PNG';s.view_settings.view_transform='Standard';s.world.color=(.08,.095,.115)
if bpy.data.objects.get('Walk review floor') is None:
 bpy.ops.mesh.primitive_plane_add(size=200,location=(0,0,-2.01));floor=bpy.context.object;floor.name='Walk review floor';mat=bpy.data.materials.new('Walk review floor');mat.use_nodes=True;bsdf=next(n for n in mat.node_tree.nodes if n.type=='BSDF_PRINCIPLED');bsdf.inputs['Base Color'].default_value=(.075,.095,.12,1);bsdf.inputs['Roughness'].default_value=.95;floor.data.materials.append(mat)
cam=s.camera;aim=Vector((-.23,0,-.15));cam.location=(4,-7,1.7);cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=4.65
bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'walk.blend'))
if '--render' in sys.argv:
 root=P.parents[2]/'build/graybox-reach/light-walk-flat/frames'
 for view,pos in [('threequarter',(4,-7,1.7)),('side',(7,-.3,.8))]:
  cam.location=pos;cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler();directory=root/view;directory.mkdir(parents=True,exist_ok=True)
  for fi in range(1,len(skin)):
   s.frame_set(fi);s.render.filepath=str(directory/f'{fi:03}.png');bpy.ops.render.render(write_still=True)
print('RESULT',json.dumps({'rig_deformation_max_error':err,'frames_tested':len(skin)}))
