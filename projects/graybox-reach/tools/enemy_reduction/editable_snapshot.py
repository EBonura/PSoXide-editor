import bpy,struct,math
from pathlib import Path
exec((Path(__file__).parent/'validate_render.py').read_text().split('report={}')[0])
for index,name in enumerate(['light','heavy']):
 m=model(O/f'{name}-reduced.psxmdl');ob=mesh(m,name+' reduced');ob.rotation_euler.x=math.pi/2;ob.scale=(.0001,)*3;ob.location.x=index*3
 for j in range(m['joints']):ob.vertex_groups.new(name='joint_'+str(j))
 for i,v in enumerate(m['v']):
  ob.vertex_groups[m['owner'][i]].add([i],(255-v[4])/255,'REPLACE')
  if v[4]:ob.vertex_groups[v[3]].add([i],v[4]/255,'ADD')
 ob['runtime_joint_count']=m['joints'];ob['source_psmd']=str(O/f'{name}-reduced.psxmdl')
 ob['note']='Rest mesh with runtime vertex weights and palette materials. Animation remains in project PSXA clips. No editable armature.'
 ob.select_set(True)
 bpy.context.view_layer.objects.active=ob
for screen in bpy.data.screens:
 for area in screen.areas:
  if area.type=='VIEW_3D':
   area.spaces.active.shading.type='MATERIAL';area.spaces.active.region_3d.view_distance=6;area.spaces.active.region_3d.view_location=(1.5,0,1)
bpy.ops.wm.save_as_mainfile(filepath=str(O/'enemy-reduction-editable.blend'))
