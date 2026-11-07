"""Blender 4.4: an editable, low-poly crystal iris cannon with baked object animation."""
import bpy,bmesh,math,json,sys
from pathlib import Path
from mathutils import Vector,Matrix
from bpy_extras.anim_utils import action_get_channelbag_for_slot
S=Path(__file__).resolve().parent;P=S.parents[3];O=P/'validation/zenith-cannon-v2'
bpy.ops.wm.open_mainfile(filepath=str(S.parent/'zenith_ranged_v1/zenith-ranged.blend'))
scene=bpy.context.scene;scene.frame_set(0)
body=bpy.data.objects['Aletha optimized'];rig=next(o for o in scene.objects if o.type=='ARMATURE')
old=bpy.data.objects['Zenith arm cannon'];bpy.context.view_layer.update()
base=old.matrix_world.to_quaternion().to_matrix().to_4x4();base.translation=old.matrix_world.translation
# Only the approved character and its rig are retained in this new study file.
for ob in list(bpy.data.objects):
 if ob not in (body,rig):bpy.data.objects.remove(ob,do_unlink=True)
collection=bpy.data.collections.new('ZENITH IRIS - editable parts');scene.collection.children.link(collection)
def empty(name,parent=None):
 o=bpy.data.objects.new(name,None);collection.objects.link(o);o.parent=parent;return o
root=empty('Cannon / wrist mount');root.parent=rig;root.parent_type='BONE';root.parent_bone='RightHand';root.matrix_world=base
front=empty('Rotor / front - clockwise',root);rear=empty('Rotor / rear - counterclockwise',root)
img=bpy.data.images.load(str(S/'iris-gradient-128-4bit.png'),check_existing=True);img.pack()
def material(name,color,emission=0,texture=False):
 m=bpy.data.materials.new(name);m.diffuse_color=(*color,1);m.use_nodes=True
 bs=m.node_tree.nodes.get('Principled BSDF');bs.inputs['Base Color'].default_value=(*color,1);bs.inputs['Roughness'].default_value=.17;bs.inputs['Metallic'].default_value=.12;bs.inputs['Transmission Weight'].default_value=.62;bs.inputs['IOR'].default_value=1.28
 m.surface_render_method='DITHERED';bs.inputs['Alpha'].default_value=.72
 m.use_transparent_shadow=True
 bs.inputs['Emission Color'].default_value=(*color,1);bs.inputs['Emission Strength'].default_value=emission
 if texture:
  t=m.node_tree.nodes.new('ShaderNodeTexImage');t.image=img;t.interpolation='Linear';t.label='128 x 128 / 16 colours / broad gradient'
  m.node_tree.links.new(t.outputs['Color'],bs.inputs['Base Color']);m.node_tree.links.new(t.outputs['Color'],bs.inputs['Emission Color'])
 return m
shell=material('Scarf mint / indexed gradient',(.16,.65,.52),.30,True)
coremat=material('Core / pale sea glass',(.40,.79,.74),.35,True)
dark=material('Bore / deep teal',(.008,.037,.043),0)
glow=material('Contained charge / mint white',(.55,1,.83),1.4)
bodymat=material('Aletha / neutral crystal study',(.5,.68,.73),.06)
body.data.materials.clear();body.data.materials.append(bodymat)
for f in body.data.polygons:f.material_index=0
parts=[]
def mesh(name,verts,faces,parent,mat):
 me=bpy.data.meshes.new(name);me.from_pydata(verts,[],faces);me.validate();me.update()
 bm=bmesh.new();bm.from_mesh(me);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(me);bm.free()
 ob=bpy.data.objects.new(name,me);collection.objects.link(ob);ob.parent=parent;me.materials.append(mat);ob.color=mat.diffuse_color
 uv=me.uv_layers.new(name='GradientAtlas')
 for poly in me.polygons:
  for loop in poly.loop_indices:
   v=me.vertices[me.loops[loop].vertex_index].co
   uv.data[loop].uv=(.08+(poly.index%6)/8+v.y*.4,.15+.65*max(0,min(1,(v.z+.1)/.55)))
 parts.append(ob);return ob

def tube(name,rings,parent,mat,n=6,close=True):
 v=[(r*math.cos(i*math.tau/n),r*math.sin(i*math.tau/n),z) for z,r in rings for i in range(n)];f=[]
 for k in range(len(rings)-1):
  for i in range(n):a=k*n+i;b=k*n+(i+1)%n;f.append((a,b,b+n,a+n))
 if close:f.extend([tuple(range(n-1,-1,-1)),tuple((len(rings)-1)*n+i for i in range(n))])
 return mesh(name,v,f,parent,mat)
core=tube('Fixed / six-sided core',[(-.075,.058),(.30,.058),(.35,.045)],root,coremat)
bore=tube('Fixed / recessed muzzle',[(.351,.038),(.342,.022)],root,dark)
light=tube('Fixed / charge lens',[(.344,.017),(.346,.017)],root,glow)
cuff=tube('Fixed / wrist collar',[(-.09,.066),(-.055,.068)],root,shell)
# Triangular wedge petals: 6 vertices / 8 triangles each. The two rings are axially separated.
petals=[]
for group,z0,length,phase in [(rear,-.042,.166,0),(front,.151,.215,math.pi/3)]:
 for i in range(3):
  theta=phase+i*math.tau/3
  verts=[(r,side*w,z0+z) for side in [-1,1] for r,w,z in [(.078,.020,0),(.121,.033,length*.35),(.116,.028,length*.64),(.069,.002,length)]]
  o=mesh(f'{group.name} / petal {i+1}',verts,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)],group,shell);o.rotation_euler.z=theta;petals.append((o,theta))
# Stationary support saddle behind the rotors; left hand never rides a rotating surface.
verts=[(x,y,z) for z in [-.09,-.049] for y in [-.10,-.061] for x in [-.04,.04]]
saddle=mesh('Fixed / supporting-hand saddle',verts,[(0,1,3,2),(4,6,7,5),(0,4,5,1),(2,3,7,6),(0,2,6,4),(1,5,7,3)],root,dark)
# Sampled choreography: unfold, rotate, anticipate, snap, settle, second shot, coast home.
beats=[(1,'DORMANT'),(13,'UNFOLD'),(31,'READY'),(47,'ANTICIPATE'),(53,'SHOT'),(61,'SETTLE'),(73,'SHOT 2'),(83,'SETTLE'),(97,'COAST'),(121,'DORMANT')]
for f,label in beats:scene.timeline_markers.new(label,frame=f)
def lerp(a,b,t):return a+(b-a)*t
def smooth(t):return t*t*(3-2*t)
def interp(keys,f):
 for (fa,a),(fb,b) in zip(keys,keys[1:]):
  if fa<=f<=fb:return lerp(a,b,smooth((f-fa)/(fb-fa)))
 return keys[-1][1]
for f in range(1,122):
 opening=interp([(1,0),(9,0),(19,1),(89,1),(113,0),(121,0)],f)
 angle=interp([(1,0),(13,0),(31,.6),(49,1.4),(53,1.68),(61,2.1),(73,2.85),(83,3.35),(101,4.05),(121,math.tau)],f)
 # Integral rotation finishes at an exact whole turn, avoiding a cycle seam.
 rear.rotation_euler.z=-angle;front.rotation_euler.z=angle
 for o in (rear,front):o.keyframe_insert('rotation_euler',frame=f)
 recoil=max(0,1-abs(f-53)/6,max(0,1-abs(f-73)/6))
 for ob,theta in petals:
  spread=.012*opening+.013*recoil
  ob.location=(spread*math.cos(theta),spread*math.sin(theta),-.022*recoil)
  ob.keyframe_insert('location',frame=f)
 core.location.z=-.017*recoil;core.keyframe_insert('location',frame=f)
 pulse=interp([(1,.4),(31,1.4),(49,2.8),(53,7),(55,1.5),(69,2.8),(73,7),(75,1.5),(97,.8),(121,.4)],f)
 bs=glow.node_tree.nodes.get('Principled BSDF');bs.inputs['Emission Strength'].default_value=pulse;bs.inputs['Emission Strength'].keyframe_insert('default_value',frame=f)
# Baked linear samples avoid Bezier overshoot, plugins or external drivers.
for ob in [rear,front,core]+[p[0] for p in petals]:
 ad=ob.animation_data
 if ad and ad.action:
  ad.action.name=ob.name+' / ready-fire-coast';cb=action_get_channelbag_for_slot(ad.action,ad.action_slot)
  for fc in cb.fcurves:
   for k in fc.keyframe_points:k.interpolation='LINEAR'
scene.frame_start=1;scene.frame_end=120;scene.render.fps=30
scene.render.engine='BLENDER_EEVEE_NEXT';scene.eevee.taa_render_samples=64;scene.eevee.use_raytracing=True
scene.render.resolution_x=960;scene.render.resolution_y=720;scene.render.resolution_percentage=100
scene.world=bpy.data.worlds.new('Cannon study studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.035,.045,.055,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.4
scene.view_settings.view_transform='AgX';scene.view_settings.look='AgX - Medium High Contrast'
def camera(name,pos,target,scale):
 o=bpy.data.objects.new(name,bpy.data.cameras.new(name));scene.collection.objects.link(o);o.location=pos;o.rotation_euler=(Vector(target)-o.location).to_track_quat('-Z','Y').to_euler();o.data.type='ORTHO';o.data.ortho_scale=scale;return o
def area(name,pos,power,size,target):
 o=bpy.data.objects.new(name,bpy.data.lights.new(name,'AREA'));scene.collection.objects.link(o);o.location=pos;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector(target)-o.location).to_track_quat('-Z','Y').to_euler()
scene.frame_set(31);bpy.context.view_layer.update();target=root.matrix_world@Vector((0,0,.15))
area('Key softbox',target+Vector((1,-2,2)),180,2,target);area('Mint rim',target+Vector((-1.5,.5,1)),220,1.5,target);area('Fill',target+Vector((2,1,.5)),90,2,target)
wide=camera('Camera / character',(3,-5,2.15),(0,-.18,1.02),2.3)
close=camera('Camera / mechanism',target+Vector((1.1,-1.25,.6)),target,.82)
scene.use_nodes=True;nt=scene.node_tree;nt.nodes.clear();rl=nt.nodes.new('CompositorNodeRLayers');g=nt.nodes.new('CompositorNodeGlare');g.glare_type='FOG_GLOW';g.threshold=1.5;g.quality='MEDIUM';g.mix=-.94;c=nt.nodes.new('CompositorNodeComposite');nt.links.new(rl.outputs['Image'],g.inputs['Image']);nt.links.new(g.outputs['Image'],c.inputs[0])
body.hide_render=False;scene.camera=wide;scene.render.resolution_x=720;scene.render.resolution_y=960;scene.render.filepath=str(O/'iris-on-aletha.png');bpy.ops.render.render(write_still=True)
body.hide_render=True;scene.camera=close;scene.render.resolution_x=960;scene.render.resolution_y=720
for frame,label in [(1,'closed'),(31,'ready'),(53,'fire'),(83,'settle')]:
 scene.frame_set(frame);scene.render.filepath=str(O/f'iris-{label}.png');bpy.ops.render.render(write_still=True)
report={'blender':bpy.app.version_string,'texture':'iris-gradient-128-4bit.png','palette_entries':16,'parts':{},'frames':[1,120],'fps':30,'beats':beats,'game_installed':False}
for ob in parts:
 ob.data.calc_loop_triangles();report['parts'][ob.name]=len(ob.data.loop_triangles)
 bm=bmesh.new();bm.from_mesh(ob.data);assert all(e.is_manifold for e in bm.edges),ob.name;assert all(f.calc_area()>1e-9 for f in bm.faces),ob.name;bm.free()
report['triangles']=sum(report['parts'].values());assert report['triangles']<=180
scene.frame_set(31);scene.camera=wide;body.hide_render=False
for screen in bpy.data.screens:
 for a in screen.areas:
  if a.type=='VIEW_3D':a.spaces.active.region_3d.view_perspective='CAMERA'
bpy.ops.wm.save_as_mainfile(filepath=str(S/'aletha-crystal-iris.blend'))
# A separate mechanism scene references the same live rig and animated parts.
scene.camera=close;body.hide_render=True;scene.render.resolution_x=800;scene.render.resolution_y=600
scene.render.image_settings.file_format='FFMPEG';scene.render.ffmpeg.format='MPEG4';scene.render.ffmpeg.codec='H264';scene.render.ffmpeg.constant_rate_factor='MEDIUM';scene.render.filepath=str(O/'iris-mechanism.mp4')
bpy.ops.render.render(animation=True)
(O/'study-validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT',json.dumps(report))
