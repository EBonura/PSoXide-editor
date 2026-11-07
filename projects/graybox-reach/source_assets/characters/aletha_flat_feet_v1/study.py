import bpy,json,numpy as np
from pathlib import Path
from mathutils import Vector,Matrix
P=Path(__file__).resolve().parents[3];O=Path(__file__).resolve().parent;R=P/'validation/aletha-flat-feet-v1'
source=P/'source_assets/characters/aletha_closed_458/Aletha-closed-458.blend'
bpy.ops.wm.open_mainfile(filepath=str(source));mesh=bpy.data.objects['Aletha optimized'];w=mesh.matrix_world.copy();changes=[]
saved_changes=json.loads((O/'vertex-changes.json').read_text()) if (O/'vertex-changes.json').exists() else []
for c in saved_changes:mesh.data.vertices[c['index']].co=w.inverted()@Vector(c['old'])
for side in ['Left','Right']:
 g=mesh.vertex_groups[side+'Foot'].index
 vs=[v for v in mesh.data.vertices if any(k.group==g and k.weight>.5 for k in v.groups)]
 for v in vs:
  p=w@v.co;q=p.copy()
  if p.z>.11 and p.y<.02:continue # keep the ankle seam
  if p.y>.06:q.y=.065;q.z=.095 # rear heel upper rim
  elif p.y>.03:q.y=.065;q.z=0.0 # rear heel sole
  elif p.y<-.04:q.y=-.145;q.z=0.0 if p.z<.008 else .032 # flat toe box
  elif p.z<.025:q.y=-.050;q.z=0.0 # broad forefoot sole
  else:q.y=-.050;q.z=.065 # instep slopes into the toe box
  changes.append({'index':v.index,'side':side,'old':list(p),'new':list(q)})
assert len(changes)==24
(O/'vertex-changes.json').write_text(json.dumps(changes,indent=2)+'\n')
for label,src,focus,eye,scale in [('neutral',source,(0,0,.15),(3,-1,.3),.55),('perch',P/'source_assets/animations/player/arch_perch_v2/arch-perch-a.blend',(0,.1,.48),(5,-.6,.68),.82)]:
 for state in ['before','after']:
  bpy.ops.wm.open_mainfile(filepath=str(src));s=bpy.context.scene;mesh=bpy.data.objects['Aletha optimized'];rig=next(o for o in s.objects if o.type=='ARMATURE')
  for c in changes:mesh.data.vertices[c['index']].co=mesh.matrix_world.inverted()@Vector(c['old'])
  if label=='neutral':
   rig.animation_data_clear()
   for b in rig.pose.bones:b.matrix_basis=Matrix.Identity(4)
  else:s.frame_set(7)
  if state=='after':
   inv=mesh.matrix_world.inverted()
   for c in changes:mesh.data.vertices[c['index']].co=inv@Vector(c['new'])
   mesh.data.update()
   if label=='perch':
    for side in ['Left','Right']:
     n=side+'Foot';bone=rig.pose.bones[n];old=rig.matrix_world@bone.matrix
     normal=Vector((0,-1,.3)).normalized();rotation=Vector((0,0,1)).rotation_difference(normal).to_matrix()
     rest=rig.matrix_world@rig.data.bones[n].matrix_local
     bottom=[Vector(c['new']) for c in changes if c['side']==side and c['new'][2]==0]
     sole=sum(bottom,Vector())/len(bottom);offset=rotation@(sole-rest.translation)
     head=old.translation.copy();head.y=.3*(head.z+offset.z)-offset.y-.001
     m=(rotation@rest.to_3x3()).to_4x4();m.translation=head;bone.matrix=rig.matrix_world.inverted()@m
     bpy.context.view_layer.update()
  bpy.context.view_layer.update()
  for ob in s.objects:
   if ob.type=='MESH' and ob!=mesh and ob.name!='Perch slanted face':ob.hide_render=True
  s.render.engine='BLENDER_WORKBENCH';s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';mesh.color=(.56,.79,.84,1);s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard';s.render.resolution_x=600;s.render.resolution_y=480;s.render.resolution_percentage=100;s.camera.data.type='ORTHO';s.camera.data.ortho_scale=scale;s.camera.location=eye;s.camera.rotation_euler=(Vector(focus)-s.camera.location).to_track_quat('-Z','Y').to_euler();s.render.filepath=str(R/f'{label}-{state}.png');bpy.ops.render.render(write_still=True)
print('RESULT',json.dumps({'changed_vertices':len(changes),'triangles':sum(len(p.vertices)-2 for p in mesh.data.polygons)}))
