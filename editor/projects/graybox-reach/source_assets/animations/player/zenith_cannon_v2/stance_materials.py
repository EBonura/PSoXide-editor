"""Editable Blender colour/transition study. HRZ is melee; no orange cannon.
Scarf meshes are preview proxies for the game's procedural ribbons, not body parts.
"""
import bpy, math, json
from pathlib import Path
from mathutils import Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
S=Path(__file__).resolve().parent; O=S.parents[3]/'validation/zenith-cannon-v2'
bpy.ops.wm.open_mainfile(filepath=str(S/'aletha-crystal-iris.blend'))
s=bpy.context.scene;s.frame_set(31);bpy.context.view_layer.update()
body=bpy.data.objects['Aletha optimized'];rig=next(o for o in s.objects if o.type=='ARMATURE')
root=bpy.data.objects['Cannon / wrist mount']
ready={b.name:b.matrix_basis.copy() for b in rig.pose.bones};rig.animation_data_clear()
world=rig.matrix_world.copy();inv=world.inverted()
def point(name,target):
 b=rig.pose.bones[name];m=world@b.matrix
 q=(m.to_3x3()@Vector((0,1,0))).rotation_difference(Vector(target)-m.translation)
 n=(q.to_matrix()@m.to_3x3()).to_4x4();n.translation=m.translation;b.matrix=inv@n;bpy.context.view_layer.update()
def arm(side,target,pole):
 u=side+'UpperArm';l=side+'LowerArm';h=side+'Hand';start=(world@rig.pose.bones[u].matrix).translation
 axis=(target-start).normalized();a=rig.data.bones[u].length;b=rig.data.bones[l].length;d=min((target-start).length,(a+b)*.97)
 along=(a*a-b*b+d*d)/(2*d);bend=pole-start;bend=(bend-axis*bend.dot(axis)).normalized()
 point(u,start+axis*along+bend*math.sqrt(max(0,a*a-along*along)));point(l,start+axis*d)
 wrist=(world@rig.pose.bones[h].matrix).translation;point(h,wrist+Vector((0,-.2,-.5)))
c=(world@rig.pose.bones['Chest'].matrix).translation
arm('Right',c+Vector((-.24,-.14,-.33)),c+Vector((-.4,.05,-.12)))
arm('Left',c+Vector((.23,-.21,-.24)),c+Vector((.4,.05,-.18)))
guard={b.name:b.matrix_basis.copy() for b in rig.pose.bones}
def ease(f,a,b):
 t=max(0,min(1,(f-a)/(b-a)));return t*t*(3-2*t)
for f in range(1,151):
 t=ease(f,39,63)
 for b in rig.pose.bones:
  b.matrix_basis=ready[b.name].lerp(guard[b.name],t);b.rotation_mode='QUATERNION'
  for prop in ['location','rotation_quaternion','scale']:b.keyframe_insert(prop,frame=f)
 root.scale=(1-ease(f,35,51),)*3;root.keyframe_insert('scale',frame=f)
# Smooth alpha compositing for the real-time review, avoiding stochastic grain.
for m in [body.data.materials[0],bpy.data.materials['Scarf mint / indexed gradient'],bpy.data.materials['Core / pale sea glass']]:
 m.surface_render_method='BLENDED';bs=m.node_tree.nodes.get('Principled BSDF')
 bs.inputs['Transmission Weight'].default_value=.25;bs.inputs['Alpha'].default_value=.80
 bs.inputs['Roughness'].default_value=.21
 if m==body.data.materials[0]:bs.inputs['Alpha'].default_value=.92
mint=bpy.data.images.get('iris-gradient-128-4bit.png')
amber=bpy.data.images.load(str(S/'scarf-hrz-gradient-128-4bit.png'));amber.pack()
mat=bpy.data.materials['Scarf mint / indexed gradient'].copy();mat.name='Scarf and transition / ZTH to HRZ crystal'
nt=mat.node_tree;bs=nt.nodes.get('Principled BSDF');tex=next(n for n in nt.nodes if n.type=='TEX_IMAGE')
t2=nt.nodes.new('ShaderNodeTexImage');t2.image=amber;t2.interpolation='Linear'
mix=nt.nodes.new('ShaderNodeMixRGB');mix.name='Stance palette';mix.blend_type='MIX'
nt.links.new(tex.outputs['Color'],mix.inputs[1]);nt.links.new(t2.outputs['Color'],mix.inputs[2])
nt.links.new(mix.outputs[0],bs.inputs['Base Color']);nt.links.new(mix.outputs[0],bs.inputs['Emission Color'])
bs.inputs['Emission Strength'].default_value=.45;bs.inputs['Alpha'].default_value=.7
for f,t in [(1,0),(45,0),(61,1),(150,1)]:mix.inputs[0].default_value=t;mix.inputs[0].keyframe_insert('default_value',frame=f)
coll=bpy.data.collections.new('Procedural scarf and stance effect PREVIEW ONLY');s.collection.children.link(coll)
def mesh(name,verts,faces):
 me=bpy.data.meshes.new(name);me.from_pydata(verts,[],faces);me.update();me.materials.append(mat)
 ob=bpy.data.objects.new(name,me);coll.objects.link(ob);uv=me.uv_layers.new()
 for p in me.polygons:
  for li in p.loop_indices:
   v=me.vertices[me.loops[li].vertex_index].co;uv.data[li].uv=(.12+v.y*.67,.18+v.z*.25)
 return ob
s.frame_set(1);bpy.context.view_layer.update();neck=(world@rig.pose.bones['Neck'].matrix).translation
for j in range(2):
 verts=[]
 for i in range(9):
  t=i/8;center=neck+Vector(((j-.5)*.09+(j-.5)*.5*t,.025+.76*t,-.015-.17*t+.09*math.sin(t*math.pi)))
  w=.042*(1-t)+.003
  verts.extend([tuple(center+Vector((-w,0,0))),tuple(center+Vector((w,0,.012*math.sin(t*5))) )])
 faces=[]
 for i in range(8):a=2*i;faces.extend([(a,a+1,a+2),(a+1,a+3,a+2)])
 ob=mesh('Scarf proxy / ribbon '+str(j+1),verts,faces);ob['export_to_body']=False
 ob.shape_key_add(name='Basis');wave=ob.shape_key_add(name='Trailing flutter')
 for i,v in enumerate(wave.data):
  t=(i//2)/8;v.co.z+=.065*t*math.sin(t*6+j);v.co.x+=.035*t*math.cos(t*5+j)
 for f in range(1,152,10):wave.value=.5+.5*math.sin((f-1)*math.tau/60+j);wave.keyframe_insert('value',frame=f)
# Short crystalline fragments carry the destination palette through the swap.
for i in range(18):
 angle=math.tau*i/18;z=.45+1.15*((i*7)%18)/17
 ob=mesh('Swap crystal / '+str(i),[(-.018,0,-.055),(.020,0,-.024),(.007,.012,.061),(-.005,-.012,.02)],[(0,1,2),(0,3,1),(0,2,3),(1,3,2)])
 for f in [1,39,47,55,65,73,150]:
  a=angle+(f-39)*.045;r=.26+.32*ease(f,39,73)
  ob.location=(math.cos(a)*r,math.sin(a)*r,z+.1*ease(f,39,73));ob.rotation_euler=(a,.2,a)
  visible=ease(f,39,47)*(1-ease(f,55,73));ob.scale=(visible,)*3
  for prop in ['location','rotation_euler','scale']:ob.keyframe_insert(prop,frame=f)
s.timeline_markers.clear()
for f,label in [(1,'ZTH / ranged'),(35,'Retract cannon'),(45,'Crystal palette handover'),(63,'HRZ / melee'),(73,'Settle scarf')]:s.timeline_markers.new(label,frame=f)
s.frame_start=1;s.frame_end=150;s.render.fps=30;s.camera=bpy.data.objects['Camera / character'];body.hide_render=False
s.camera.location=(3,-5,2.15);s.camera.data.ortho_scale=2.35
s.render.resolution_x=720;s.render.resolution_y=960;s.render.resolution_percentage=100
s.render.image_settings.file_format='PNG';s.eevee.taa_render_samples=96
for f,label in [(1,'zth-crystal'),(53,'stance-crystal-swap'),(100,'hrz-melee-crystal')]:
 s.frame_set(f);s.render.filepath=str(O/(label+'.png'));bpy.ops.render.render(write_still=True)
s.frame_set(1);bpy.context.preferences.filepaths.save_version=0
s['study_only']=True;s['HRZ']='Melee. Amber scarf and transition only.';s['ZTH']='Ranged. Sea-green iris cannon and scarf.'
bpy.ops.wm.save_as_mainfile(filepath=str(S/'aletha-stance-crystal.blend'))
s.render.resolution_x=540;s.render.resolution_y=720;s.render.image_settings.file_format='FFMPEG'
s.render.ffmpeg.format='MPEG4';s.render.ffmpeg.codec='H264';s.render.ffmpeg.constant_rate_factor='MEDIUM';s.render.filepath=str(O/'crystal-stance-transition.mp4')
bpy.ops.render.render(animation=True)
s.frame_set(100);assert max(root.scale)==0
report={'study_only':True,'game_installed':False,'HRZ':'melee; cannon fully retracted','ZTH':'ranged; green cannon','scarf':'procedural proxy; excluded from character mesh','texture_size':[128,128],'palette_entries_each':16,'frames':150,'fps':30,'cannon_scale_in_HRZ':list(root.scale)}
(O/'stance-material-validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT',json.dumps(report))
