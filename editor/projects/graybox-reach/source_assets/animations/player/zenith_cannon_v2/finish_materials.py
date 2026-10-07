"""Assign finish masks to the existing cannon faces; preserve mesh and animation."""
import bpy,json
from pathlib import Path
S=Path(__file__).resolve().parent
print('BLENDER_VERSION',bpy.app.version_string)
bpy.ops.wm.open_mainfile(filepath=str(S/'aletha-crystal-iris.blend'))
scene=bpy.context.scene;scene.frame_set(1);bpy.context.view_layer.update()
image=bpy.data.images.load(str(S/'cannon-finish-128-4bit.png'),check_existing=True);image.reload();image.pack()
mat=bpy.data.materials.get('Cannon / opaque B finish atlas') or bpy.data.materials.new('Cannon / opaque B finish atlas')
mat.use_nodes=True;nodes=mat.node_tree.nodes;nodes.clear()
out=nodes.new('ShaderNodeOutputMaterial');emit=nodes.new('ShaderNodeEmission');tex=nodes.new('ShaderNodeTexImage');tex.image=image;tex.interpolation='Closest'
mat.node_tree.links.new(tex.outputs['Color'],emit.inputs['Color']);mat.node_tree.links.new(emit.outputs[0],out.inputs['Surface'])
mat.diffuse_color=(.22,.65,.5,1)
report={}
for ob in bpy.data.collections['ZENITH IRIS - editable parts'].objects:
 if ob.type!='MESH':continue
 me=ob.data;attr=me.attributes.get('psx_reflection_band') or me.attributes.new('psx_reflection_band','INT','FACE')
 me.materials.clear();me.materials.append(mat)
 for poly in me.polygons:
  if 'petal' in ob.name: band=2 if poly.index<2 else (1 if poly.index==5 else 3)
  elif 'six-sided core' in ob.name: band=4 if 6<=poly.index<12 or poly.index==13 else 1
  elif 'charge lens' in ob.name: band=4
  elif 'wrist collar' in ob.name: band=2 if poly.index<6 else 3
  else: band=1
  attr.data[poly.index].value=band;poly.material_index=0
  # Preview UVs show the same part contrast with baked lighting.
  coords=[me.vertices[me.loops[li].vertex_index].co for li in poly.loop_indices]
  axes=sorted(range(3),key=lambda a:max(v[a] for v in coords)-min(v[a] for v in coords),reverse=True)[:2]
  mins=[min(v[a] for v in coords) for a in axes];span=[max(v[a] for v in coords)-mins[i] or 1 for i,a in enumerate(axes)]
  for li in poly.loop_indices:
   v=me.vertices[me.loops[li].vertex_index].co
   u=(v[axes[0]]-mins[0])/span[0];w=(v[axes[1]]-mins[1])/span[1]
   me.uv_layers.active.data[li].uv=(((band-1)*32+2+u*27)/128,.12+w*.75)
 ob['finish_bands']='1 dark core, 2 jade shell, 3 bright edges, 4 pale muzzle'
 report[ob.name]=[v.value for v in attr.data]
scene['cannon_material']='Opaque B palette; four reflection strips; no extra polygons'
bpy.context.preferences.filepaths.save_version=0
bpy.ops.wm.save_as_mainfile(filepath=str(S/'aletha-crystal-iris.blend'))
(S/'cannon-finishes.json').write_text(json.dumps(report,indent=2)+'\n')
print('RESULT',json.dumps({'objects':len(report),'face_bands':report}))
