import bpy,json
from pathlib import Path
from mathutils import Vector
P=Path(__file__).resolve().parents[2]
O=P/'source_assets/characters/aletha_closed_458';o=bpy.data.objects['Aletha optimized'];s=bpy.context.scene;c=s.camera
mat=bpy.data.materials.new('Aletha PS1 128x128 4-bit reflection');mat.use_nodes=True;nt=mat.node_tree;nt.nodes.clear()
def node(kind,label):
 n=nt.nodes.new(kind);n.label=label;return n
def math(op,a=None,b=None):
 n=node('ShaderNodeMath',op);n.operation=op
 for i,x in enumerate([a,b]):
  if x is None:continue
  if isinstance(x,(int,float)):n.inputs[i].default_value=x
  else:nt.links.new(x,n.inputs[i])
 return n.outputs[0]
g=node('ShaderNodeNewGeometry','Flat facet normal');xf=node('ShaderNodeVectorTransform','World to camera');xf.vector_type='NORMAL';xf.convert_from='WORLD';xf.convert_to='CAMERA';nt.links.new(g.outputs['Normal'],xf.inputs[0]);sp=node('ShaderNodeSeparateXYZ','View-space normal');nt.links.new(xf.outputs[0],sp.inputs[0]);ax=[math('ABSOLUTE',sp.outputs[i]) for i in range(3)];length=math('MAXIMUM',math('ADD',math('ADD',ax[0],ax[1]),ax[2]),.00001)
x=math('MULTIPLY_ADD',math('DIVIDE',sp.outputs[0],length),.5);x.node.inputs[2].default_value=.5
y=math('MULTIPLY_ADD',math('DIVIDE',sp.outputs[1],length),.5);y.node.inputs[2].default_value=.5
combine=node('ShaderNodeCombineXYZ','Octahedral reflection lookup');nt.links.new(x,combine.inputs[0]);nt.links.new(y,combine.inputs[1]);tex=node('ShaderNodeTexImage','128x128 / 16 colours');tex.image=bpy.data.images.load(str(O/'reflection-map.png'),check_existing=True);tex.image.pack();tex.interpolation='Closest';attr=node('ShaderNodeAttribute','Authored facet gradient');attr.attribute_name='facet_gradient';add=node('ShaderNodeVectorMath','Gradient across each facet');add.operation='ADD';nt.links.new(combine.outputs[0],add.inputs[0]);nt.links.new(attr.outputs['Vector'],add.inputs[1]);nt.links.new(add.outputs[0],tex.inputs['Vector']);em=node('ShaderNodeEmission','PS1 baked reflection');nt.links.new(tex.outputs['Color'],em.inputs['Color']);em.inputs['Strength'].default_value=1;out=node('ShaderNodeOutputMaterial','Output');nt.links.new(em.outputs[0],out.inputs['Surface'])
for i,n in enumerate(nt.nodes):n.location=((i%6)*210,-(i//6)*210)
o.data.materials.clear();o.data.materials.append(mat)
gradient=o.data.attributes.get('facet_gradient') or o.data.attributes.new('facet_gradient','FLOAT_VECTOR','CORNER')
for p in o.data.polygons:
 p.use_smooth=False
 coords=[o.data.vertices[o.data.loops[i].vertex_index].co.copy() for i in p.loop_indices]
 drop=max(range(3),key=lambda a:abs(p.normal[a]));axes=[a for a in range(3) if a!=drop]
 center=[sum(v[a] for v in coords)/len(coords) for a in axes]
 scale=max(max(abs(v[a]-center[j]) for j,a in enumerate(axes)) for v in coords) or 1
 for li,v in zip(p.loop_indices,coords):
  q=[min(3,max(0,round(((v[a]-center[j])/scale+1)*1.5))) for j,a in enumerate(axes)]
  gradient.data[li].vector=((q[0]*2-3)*4/127,(q[1]*2-3)*4/127,0)
s.view_settings.view_transform='Standard';s.view_settings.look='None';s.view_settings.exposure=0;s.view_settings.gamma=1
s.render.resolution_x=640;s.render.resolution_y=900;s.render.resolution_percentage=100;c.data.ortho_scale=2.1;center=Vector((0,.025,.95))
for name,offset in [('crystal-front',(0,-3.5,0)),('crystal-angle',(1.6,-3.1,.1)),('crystal-side',(-2.3,-2.5,.1))]:
 c.location=center+Vector(offset);c.rotation_euler=(center-c.location).to_track_quat('-Z','Y').to_euler();s.render.filepath=str(O/(name+'.png'));bpy.ops.render.render(write_still=True)
c.location=center+Vector((0,-3.5,0));c.rotation_euler=(center-c.location).to_track_quat('-Z','Y').to_euler()
s['material_note']='PS1 faceted matcap preview: same 128x128 4-bit lookup image. Runtime uses dominant-joint Q7 normals and neutral CLUT with authored per-corner gradient offsets; this Blender preview uses exact flat surface normals.'
bpy.ops.wm.save_as_mainfile(filepath=str(O/'aletha-crystal.blend'));print('RESULT saved crystal preview')
