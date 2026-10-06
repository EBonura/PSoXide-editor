import bpy,bmesh,struct,json,math,collections
from pathlib import Path
from mathutils import Vector
import numpy as np
P=Path(__file__).resolve().parents[2];O=P/'source_assets/characters/enemy_reduced';D=P.parent/'default'
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
s=bpy.context.scene;s.render.engine='BLENDER_EEVEE_NEXT';s.render.resolution_x=700;s.render.resolution_y=620;s.render.resolution_percentage=100;s.render.image_settings.file_format='PNG';s.world.color=(.13,.13,.13);s.view_settings.view_transform='Standard';s.view_settings.look='None';s.render.film_transparent=False
cam=bpy.data.objects.new('Review camera',bpy.data.cameras.new('Review camera'));s.collection.objects.link(cam);s.camera=cam;cam.data.type='ORTHO'
for name,pos,power,size in [('Key',(-3,-4,6),950,5),('Fill',(4,-1,3),600,4),('Rim',(1,4,4),900,3)]:
 l=bpy.data.objects.new(name,bpy.data.lights.new(name,'AREA'));s.collection.objects.link(l);l.location=pos;l.data.energy=power;l.data.shape='DISK';l.data.size=size;l.rotation_euler=(-l.location).to_track_quat('-Z','Y').to_euler()
b=(D/'assets/models/shared_enemy_01/shared_enemy_01.psxt').read_bytes();bpp,_,w,h,cc,ps,cs=struct.unpack_from('<BBHHHII',b,12);data=np.frombuffer(b[28:28+ps],dtype=np.uint8);idx=np.empty(ps*2,dtype=np.uint8);idx[::2]=data&15;idx[1::2]=data>>4;idx=idx.reshape(h,w);pal=np.frombuffer(b[28+ps:28+ps+cs],dtype='<u2');mats=[]
for bank in range(cc//16):
 p=pal[bank*16:bank*16+16];rgba=np.array([[(v&31)/31,((v>>5)&31)/31,((v>>10)&31)/31,1.] for v in p],dtype=np.float32);im=bpy.data.images.new('Enemy palette '+str(bank),width=w,height=h);im.pixels.foreach_set(rgba[idx][::-1].reshape(-1));im.pack();mat=bpy.data.materials.new('Enemy bank '+str(bank));mat.use_nodes=True;nt=mat.node_tree;nt.nodes.clear();t=nt.nodes.new('ShaderNodeTexImage');t.image=im;t.interpolation='Closest';e=nt.nodes.new('ShaderNodeEmission');nt.links.new(t.outputs['Color'],e.inputs[0]);out=nt.nodes.new('ShaderNodeOutputMaterial');nt.links.new(e.outputs[0],out.inputs[0]);mats.append(mat)
clay=bpy.data.materials.new('Neutral clay');clay.diffuse_color=(.43,.53,.58,1)
black=bpy.data.materials.new('Topology edges');black.diffuse_color=(.012,.018,.022,1)
