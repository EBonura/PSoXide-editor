import bpy,json,math
from pathlib import Path
from mathutils import Vector
from bpy_extras.object_utils import world_to_camera_view
project=Path(__file__).resolve().parents[4];p=project/'review/heavy-attack-v1';p.mkdir(parents=True,exist_ok=True);out=project/'source_assets/animations/player/cybernetic_heavy_attack_review_v1';bpy.ops.wm.open_mainfile(filepath=str(p/'preview.blend'));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE')
meshes=[o for o in s.objects if o.type=='MESH' and any(m.type=='ARMATURE' for m in o.modifiers)];bounds=[];extensions=[];minz=100;maxturn=0;last={};blade_samples={side:[] for side in ['Right','Left']}
for f in range(61):
 s.frame_set(f);bpy.context.view_layer.update();dg=bpy.context.evaluated_depsgraph_get();verts=[]
 for ob in meshes:
  ev=ob.evaluated_get(dg);me=ev.to_mesh();verts.extend(ev.matrix_world@v.co for v in me.vertices);ev.to_mesh_clear()
 minz=min(minz,min(v.z for v in verts))
 for side in ['Right','Left']:
  w=bpy.data.objects[side+' blade'];m=w.matrix_world.copy();blade_samples[side].append((m,m@Vector((0,0,max(v.co.z for v in w.data.vertices)))))
  if not w.hide_render:verts.extend(m@v.co for v in w.data.vertices)
  chain=[(r.matrix_world@r.pose.bones[side+n].matrix).translation for n in ['UpperLeg','LowerLeg','Foot']];extensions.append((chain[2]-chain[0]).length/((chain[1]-chain[0]).length+(chain[2]-chain[1]).length))
 for b in r.pose.bones:
  q=b.rotation_quaternion.copy()
  if b.name in last:maxturn=max(maxturn,math.degrees(q.rotation_difference(last[b.name]).angle))
  last[b.name]=q
 bounds.append(verts)
face={}
for side,frames in blade_samples.items():
 face[side]={}
 for f in range(23,30):
  m,tip=frames[f];velocity=(frames[f+1][1]-frames[f-1][1]).normalized();x=(m.to_3x3()@Vector((1,0,0))).normalized();face[side][f]=round(abs(x.dot(velocity)),4)
framing={}
for label,offset in [('front',(0,-6,1.5)),('quarter',(3.8,-5.4,1.65))]:
 cam=s.camera;target=Vector((0,-.3,1.03));cam.location=target+Vector(offset);cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=4.65;bpy.context.view_layer.update();points=[world_to_camera_view(s,cam,v) for vs in bounds for v in vs];framing[label]={'xmin':min(v.x for v in points),'xmax':max(v.x for v in points),'ymin':min(v.y for v in points),'ymax':max(v.y for v in points)}
 s.render.image_settings.media_type='VIDEO';s.render.image_settings.file_format='FFMPEG';s.render.ffmpeg.format='MPEG4';s.render.ffmpeg.codec='H264';s.render.ffmpeg.constant_rate_factor='HIGH';s.render.filepath=str(p/(label+'.mp4'));bpy.ops.render.render(animation=True)
report=json.loads((out/'validation.json').read_text());report.update({'max_leg_extension_ratio':max(extensions),'lowest_body_vertex_z':minz,'max_local_joint_rotation_per_frame_degrees':maxturn,'blade_flat_face_dot_travel_abs':face,'framing_normalized':framing});(out/'validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({k:report[k] for k in ['max_leg_extension_ratio','lowest_body_vertex_z','max_local_joint_rotation_per_frame_degrees','blade_flat_face_dot_travel_abs','framing_normalized']}))
