import bpy,sys,json,math
from pathlib import Path
from mathutils import Vector
from bpy_extras.object_utils import world_to_camera_view
O=Path(__file__).resolve().parent;P=O.parents[3];B=P.parents[2]/'build/graybox-reach/player-reactions-v3';V=P/'validation/player-reactions-v3'
full='--full' in sys.argv;report={}
for label in ['poise','hit']:
 bpy.ops.wm.open_mainfile(filepath=str(O/(label+'.blend')));s=bpy.context.scene;mesh=bpy.data.objects['Aletha optimized'];end=s.frame_end
 s.world=bpy.data.worlds.new('Review studio');s.world.color=(.06,.073,.094);s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=640;s.render.resolution_y=864;s.render.resolution_percentage=100;s.render.image_settings.file_format='PNG';s.view_settings.view_transform='Standard';s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.display.shading.cavity_type='BOTH';s.display.shading.background_type='WORLD';mesh.color=(.42,.73,.79,1)
 s.frame_set(0);dg=bpy.context.evaluated_depsgraph_get();ob=mesh.evaluated_get(dg);me=ob.to_mesh();vs=[ob.matrix_world@v.co for v in me.vertices];floor=min(v.z for v in vs);ob.to_mesh_clear()
 bpy.ops.mesh.primitive_plane_add(size=20,location=(0,0,floor-.002));bpy.context.object.color=(.22,.25,.30,1)
 for axis in [0,1]:
  for i in range(-8,9):
   bpy.ops.mesh.primitive_cube_add(size=1,location=(i*.25 if axis==0 else 0,i*.25 if axis==1 else 0,floor-.001));g=bpy.context.object;g.scale=(.004,4,.001) if axis==0 else (4,.004,.001);g.color=(.35,.4,.46,1)
 bpy.ops.object.camera_add();cam=bpy.context.object;s.camera=cam;cam.data.type='ORTHO';cam.data.ortho_scale=2.42;focus=Vector((0,0,floor+.98));report[label]={};points=[];floor_min=1e9
 for view in ['quarter','side']:
  cam.location=focus+Vector((3.3,-5,1.15) if view=='quarter' else (6,0,.8));cam.rotation_euler=(focus-cam.location).to_track_quat('-Z','Y').to_euler();bpy.context.view_layer.update();folder=B/'frames'/label/view;folder.mkdir(parents=True,exist_ok=True)
  frames=range(end+1) if full else sorted(set(min(f,end) for f in [0,8,20,38,56,84,end]))
  for f in frames:
   s.frame_set(f);dg=bpy.context.evaluated_depsgraph_get();ev=mesh.evaluated_get(dg);me=ev.to_mesh();vv=[ev.matrix_world@v.co for v in me.vertices];points.extend(world_to_camera_view(s,cam,v) for v in vv);floor_min=min(floor_min,min(v.z for v in vv)-floor);ev.to_mesh_clear();s.render.filepath=str(folder/f'{f:03}.png');bpy.ops.render.render(write_still=True)
  report[label][view]={'frames':len(frames),'xmin':min(v.x for v in points),'xmax':max(v.x for v in points),'ymin':min(v.y for v in points),'ymax':max(v.y for v in points)}
  assert min(v.x for v in points)>0 and max(v.x for v in points)<1 and min(v.y for v in points)>0 and max(v.y for v in points)<1
 report[label]['min_floor_relative_to_initial']=floor_min
(V/('render-validation.json' if full else 'blocking-validation.json')).write_text(json.dumps(report,indent=2));print('RESULT',json.dumps(report))
