import bpy,json,math
from pathlib import Path
from mathutils import Vector
from bpy_extras.object_utils import world_to_camera_view
out=Path(__file__).resolve().parent;p=out.parents[3]/'review/back-walk-v2';bpy.ops.wm.open_mainfile(filepath=str(out/'walk_bwd.blend'));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');meshes=[o for o in s.objects if o.type=='MESH'];bounds=[];minz=100;rotstep=0;prev={}
for f in range(85):
 s.frame_set(f//2,subframe=(f%2)/2);dg=bpy.context.evaluated_depsgraph_get();vs=[]
 for ob in meshes:
  ev=ob.evaluated_get(dg);me=ev.to_mesh();vs.extend(ev.matrix_world@v.co for v in me.vertices);ev.to_mesh_clear()
 minz=min(minz,min(v.z for v in vs));bounds.append(vs)
 for b in r.pose.bones:
  q=b.rotation_quaternion.copy()
  if b.name in prev:rotstep=max(rotstep,math.degrees(q.rotation_difference(prev[b.name]).angle))
  prev[b.name]=q
s.world=bpy.data.worlds.new('Studio');s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=480;s.render.resolution_y=480;s.render.resolution_percentage=100;s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.display.shading.cavity_type='BOTH';s.display.shading.background_type='WORLD';s.world.color=(.055,.065,.085);s.view_settings.view_transform='Standard';s.render.image_settings.file_format='PNG'
for ob in meshes:ob.color=(.22,.64,.74,1)
bpy.ops.mesh.primitive_plane_add(size=20);bpy.context.object.color=(.24,.27,.32,1)
for axis in [0,1]:
 for i in range(-8,9):
  bpy.ops.mesh.primitive_cube_add(size=1,location=(i*.25 if axis==0 else 0,i*.25 if axis==1 else 0,.001));g=bpy.context.object;g.scale=(.006,4,.001) if axis==0 else (4,.006,.001);g.color=(.39,.43,.48,1)
bpy.ops.object.camera_add();cam=bpy.context.object;s.camera=cam;cam.data.type='ORTHO';cam.data.ortho_scale=2.8;framing={}
for view,d in [('front',(0,-6,1.1)),('quarter',(3.8,-5.4,1.3)),('side',(6,0,1.0))]:
 target=Vector((0,0,.96));cam.location=target+Vector(d);cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();bpy.context.view_layer.update();points=[world_to_camera_view(s,cam,v) for vs in bounds for v in vs];framing[view]={'xmin':min(v.x for v in points),'xmax':max(v.x for v in points),'ymin':min(v.y for v in points),'ymax':max(v.y for v in points)};folder=p/view;folder.mkdir(exist_ok=True)
 for f in range(42):
  s.frame_set(f);s.render.filepath=str(folder/f'{f:03}.png');bpy.ops.render.render(write_still=True)
 print('VIEW_DONE',view,flush=True)
report=json.loads((out/'validation.json').read_text());report.update(lowest_vertex_subframe_z=minz,max_local_rotation_half_frame_degrees=rotstep,framing=framing,loop_seam_max_vertex_distance=max((a-b).length for a,b in zip(bounds[0],bounds[-1])));(out/'validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({k:v for k,v in report.items() if k!='samples'}))
