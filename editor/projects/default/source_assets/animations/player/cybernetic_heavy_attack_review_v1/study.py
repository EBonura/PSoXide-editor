import bpy,math,json
from pathlib import Path
from mathutils import Vector,Matrix
project=Path(__file__).resolve().parents[4];p=project/'review/heavy-attack-v1';p.mkdir(parents=True,exist_ok=True)
print('VERSION',bpy.app.version_string)
bpy.ops.wm.open_mainfile(filepath=str(project/'review/light-attack-v5/preview.blend'));s=bpy.context.scene
oldrig=next(o for o in s.objects if o.type=='ARMATURE')
for o in list(s.objects):
 if o==oldrig or o.parent==oldrig or (o.type=='MESH' and any(m.type=='ARMATURE' for m in o.modifiers)):bpy.data.objects.remove(o,do_unlink=True)
bpy.ops.import_scene.gltf(filepath=str(project/'source_assets/animations/player/direct_shoulders/r2_horizon_heavy.glb'))
r=next(o for o in s.objects if o.type=='ARMATURE');a=r.animation_data.action or next(t.strips[0].action for t in r.animation_data.nla_tracks)
for t in list(r.animation_data.nla_tracks):r.animation_data.nla_tracks.remove(t)
r.animation_data.action=a;r.animation_data.action_slot=a.slots[0];s.render.fps=30
print('ACTION',a.name,list(a.frame_range));rest={b.name:r.matrix_world@b.matrix_local for b in r.data.bones}
for ob in s.objects:
 if ob.type=='MESH' and any(m.type=='ARMATURE' for m in ob.modifiers):ob.color=(.22,.64,.74,1)
archive=Path('/Users/ebonura/Desktop/repos/_archive/PSoXide-monorepo-20260915/editor/archive/legacy-grid/archive/cortex/cortex_v1/source_assets/props')
for side,kind,height,gripq,yaw,cast in [('Right','heavy',768,-26320,1696,54),('Left','light',566,-25370,-1696,80)]:
 before=set(bpy.data.objects);bpy.ops.import_scene.gltf(filepath=str(archive/f'sword1_{kind}.glb'));bpy.context.view_layer.update();w=next(o for o in bpy.data.objects if o not in before and o.type=='MESH');w.name=side+' blade'
 verts=[w.matrix_world@v.co for v in w.data.vertices];lo=Vector([min(v[i] for v in verts) for i in range(3)]);hi=Vector([max(v[i] for v in verts) for i in range(3)]);wh=max(hi-lo)/2;grip=(lo+hi)/2+Vector((0,0,gripq))*wh/32767;scale=1.878*height/1024/max(hi-lo)
 for v,co in zip(w.data.vertices,verts):v.co=(co-grip)*scale
 w.matrix_world=Matrix.Identity(4);C=Matrix.Rotation(math.pi/2,4,'X');R=C@Matrix.Rotation(yaw*2*math.pi/4096,4,'Z')@Matrix.Rotation(32*2*math.pi/4096,4,'X')@C.inverted();hand=side+'Hand';socket=rest[hand].translation+(rest[side+'IndexProximal'].translation-rest[hand].translation)*.72
 local=rest[hand].inverted()@Matrix.Translation(socket)@R;w.parent=r;w.parent_type='BONE';w.parent_bone=hand;w.matrix_parent_inverse=Matrix.Identity(4);w.matrix_basis=Matrix.Translation((0,-r.data.bones[hand].length,0))@local;w.color=(1,.5,.075,1)
 wire=w.copy();wire.data=w.data.copy();s.collection.objects.link(wire);wire.name=side+' wire';wire.color=(1,.75,.15,1);mod=wire.modifiers.new('Energy outline','WIREFRAME');mod.thickness=.006
 for ob,events in [(wire,[(0,True),(cast-16,False),(cast,True),(156,False),(172,True)]),(w,[(0,True),(cast,False),(156,True)])]:
  for f,v in events:ob.hide_render=v;ob.keyframe_insert('hide_render',frame=f)
s.render.resolution_x=640;s.render.resolution_y=640;s.render.resolution_percentage=100;s.frame_start=18;s.frame_end=186
cam=s.camera;target=Vector((0,-.1,1));cam.location=target+Vector((0,-6,1.4));cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=4.3
s.render.image_settings.media_type='IMAGE';s.render.image_settings.file_format='PNG';s.frame_set(18);bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(p/'original-preview.blend'))
for f in [18,38,54,80,104,110,120,132,144,152,172,186]:
 s.frame_set(f);s.render.filepath=str(p/f'study-{f:03}.png');bpy.ops.render.render(write_still=True)
print('RESULT original heavy study saved')
