import bpy,sys,json,math
from pathlib import Path
from mathutils import Vector
from bpy_extras.object_utils import world_to_camera_view
out=Path(__file__).resolve().parent;p=out.parents[3]/'review/walk-transitions-v4';report={}
for variant,views in [('A',['quarter','side']),('B',['quarter'])]:
 for kind in ['new']:
  bpy.ops.wm.open_mainfile(filepath=str(out/(kind+'_'+variant+'.blend')));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');meshes=[o for o in s.objects if o.type=='MESH'];end=s.frame_end;bounds=[];cache={};framekeys=[]
  for f in range(end+1):
   s.frame_set(f);key=tuple(round(v,5) for b in r.pose.bones for row in b.matrix for v in row);framekeys.append(key)
   if key in cache:continue
   dg=bpy.context.evaluated_depsgraph_get();vs=[]
   for ob in meshes:
    ev=ob.evaluated_get(dg);me=ev.to_mesh();vs.extend(ev.matrix_world@v.co for v in me.vertices);ev.to_mesh_clear()
   cache[key]=f;bounds.append(vs)
  floorz=min(v.z for vs in bounds for v in vs);center=Vector((sum(v.x for v in bounds[0])/len(bounds[0]),sum(v.y for v in bounds[0])/len(bounds[0]),floorz+.96))
  s.world=bpy.data.worlds.new('Studio');s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=480;s.render.resolution_y=480;s.render.resolution_percentage=100;s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.display.shading.cavity_type='BOTH';s.display.shading.background_type='WORLD';s.world.color=(.055,.065,.085);s.view_settings.view_transform='Standard';s.render.image_settings.file_format='PNG'
  for ob in meshes:ob.color=(.22,.64,.74,1)
  bpy.ops.mesh.primitive_plane_add(size=20,location=(0,0,floorz));bpy.context.object.color=(.24,.27,.32,1)
  for axis in [0,1]:
   for i in range(-8,9):
    bpy.ops.mesh.primitive_cube_add(size=1,location=(i*.25 if axis==0 else 0,i*.25 if axis==1 else 0,floorz+.001));g=bpy.context.object;g.scale=(.006,4,.001) if axis==0 else (4,.006,.001);g.color=(.39,.43,.48,1)
  bpy.ops.object.camera_add();cam=bpy.context.object;s.camera=cam;cam.data.type='ORTHO';cam.data.ortho_scale=2.8
  for view in views:
   direction=(3.8,-5.4,1.3) if view=='quarter' else (6,0,1.0);cam.location=center+Vector(direction);cam.rotation_euler=(center-cam.location).to_track_quat('-Z','Y').to_euler();bpy.context.view_layer.update();pts=[world_to_camera_view(s,cam,v) for vs in bounds for v in vs];framing={'xmin':min(v.x for v in pts),'xmax':max(v.x for v in pts),'ymin':min(v.y for v in pts),'ymax':max(v.y for v in pts)};assert 0<framing['xmin']<framing['xmax']<1 and 0<framing['ymin']<framing['ymax']<1
   folder=p/(kind+'_'+variant+'_'+view);folder.mkdir(exist_ok=True)
   for f in range(end+1):
    dst=folder/f'{f:03}.png';first=cache[framekeys[f]]
    if first<f:
     import shutil;shutil.copyfile(folder/f'{first:03}.png',dst);continue
    s.frame_set(f);s.render.filepath=str(dst);bpy.ops.render.render(write_still=True)
   report[kind+'_'+variant+'_'+view]={'frames':end+1,'floor_z':floorz,'framing':framing};print('VIEW_DONE',kind,variant,view,flush=True)
(p/'render-validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT transition review rendered')
