import bpy,json
from pathlib import Path
from mathutils import Vector
P=Path(__file__).resolve().parents[4];O=P/'validation/zenith-cannon-v2';O.mkdir(exist_ok=True)
files=[Path('/Users/ebonura/Desktop/godot/cortex-ignition-0/assets')/f'ship test modular {n}.blend' for n in (1,2,3)]
files+=[Path('/Users/ebonura/Desktop/godot/cortex-ignition-0/assets/ship test 5.blend'),Path('/Users/ebonura/Desktop/godot/racing-test4/assets/ships/ship_lamiera.blend')]
report=[]
for i,path in enumerate(files):
 bpy.ops.wm.open_mainfile(filepath=str(path));s=bpy.context.scene;s.frame_set(1)
 meshes=[o for o in s.objects if o.type=='MESH' and not o.hide_render]
 points=[o.matrix_world@Vector(c) for o in meshes for c in o.bound_box]
 low=Vector(tuple(min(p[k] for p in points) for k in range(3)));hi=Vector(tuple(max(p[k] for p in points) for k in range(3)));target=(low+hi)/2;size=(hi-low).length
 cam=bpy.data.objects.new('ReviewCamera',bpy.data.cameras.new('ReviewCamera'));s.collection.objects.link(cam);s.camera=cam
 cam.location=target+Vector((1,-1,.8)).normalized()*size*1.6;cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.type='ORTHO';cam.data.ortho_scale=size*1.12
 s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=700;s.render.resolution_y=500;s.render.resolution_percentage=100
 s.display.shading.light='STUDIO';s.display.shading.color_type='MATERIAL';s.display.shading.background_type='WORLD';s.world.color=(.045,.055,.065);s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard'
 s.render.filepath=str(O/f'ship-{i+1}.png');bpy.ops.render.render(write_still=True)
 report.append({'path':str(path),'image':s.render.filepath,'objects':[{'name':o.name,'vertices':len(o.data.vertices),'dimensions':list(o.dimensions)} for o in meshes]})
(O/'references.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps(report))
