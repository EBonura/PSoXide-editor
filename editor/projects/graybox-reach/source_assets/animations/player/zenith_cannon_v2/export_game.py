"""Export the editable iris to the PS1's three rigid joints and reflection UVs."""
import bpy,struct,math,json,sys
from pathlib import Path
from mathutils import Matrix,Vector
S=Path(__file__).resolve().parent;P=S.parents[3];D=P/'assets/models/zenith_projector'
bpy.ops.wm.open_mainfile(filepath=str(S/'aletha-crystal-iris.blend'))
s=bpy.context.scene;s.frame_set(1);bpy.context.view_layer.update()
root=bpy.data.objects['Cannon / wrist mount'];ri=root.matrix_world.inverted()
# Recover exactly the approved character's authoring unit.
body=bpy.data.objects['Aletha optimized'];raw=(P/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes()
j,p,nv,nf,nm,*_=struct.unpack_from('<8H',raw,12);vo=28+j*4+nm*8+p*16
ys=[struct.unpack_from('<3h',raw,vo+i*8)[1] for i in range(nv)]
zs=[(body.matrix_world@v.co).z for v in body.data.vertices];unit=(max(zs)-min(zs))/(max(ys)-min(ys))
verts=[];faces=[];parts=[]
for ob in sorted(bpy.data.collections['ZENITH IRIS - editable parts'].objects,key=lambda o:o.name):
 if ob.type!='MESH':continue
 joint=1 if ob.parent.name.startswith('Rotor / front') else 2 if ob.parent.name.startswith('Rotor / rear') else 0
 v0=len(verts);f0=len(faces);m=ri@ob.matrix_world
 verts.extend([tuple(round(c/unit) for c in m@v.co) for v in ob.data.vertices]);ob.data.calc_loop_triangles()
 for tri in ob.data.loop_triangles:
  # PSX's front-face convention is opposite Blender's.
  ids=[v0+tri.vertices[k] for k in [0,2,1]];ps=[Vector(verts[i]) for i in ids]
  normal=(ps[2]-ps[0]).cross(ps[1]-ps[0]).normalized();data=[round(c*127)&255 for c in normal]+[joint]
  drop=max(range(3),key=lambda i:abs(normal[i]));axes=[i for i in range(3) if i!=drop]
  center=sum(ps,Vector())/3;extent=max(abs((v-center)[a]) for v in ps for a in axes) or 1
  bits=0x8000
  for k,v in enumerate(ps):
   q=[max(0,min(3,round(((v-center)[a]/extent+1)*1.5))) for a in axes];bits|=(q[0]|q[1]<<2)<<(k*4)
  data.extend([bits&255,bits>>8]);faces.append(b''.join(struct.pack('<HBB',i,*data[k*2:k*2+2]) for k,i in enumerate(ids)))
 parts.append(struct.pack('<6HI',joint,v0,len(verts)-v0,f0,len(faces)-f0,0,0))
b=bytearray(b'PSMD'+struct.pack('<HHI',4,2|4|64,0));b+=struct.pack('<8H',3,len(parts),len(verts),len(faces),1,128,128,98)
b+=b''.join(struct.pack('<HH',parent,0) for parent in [65535,0,0]);b+=struct.pack('<HH4B',0,0,128,128,128,255)
b+=b''.join(parts);b+=b''.join(struct.pack('<3hBB',*v,255,0) for v in verts);b+=b''.join(faces);struct.pack_into('<I',b,8,len(b)-12)
(D/'projector.psxmdl').write_bytes(b)
# Counter-rotation is a clean four-second loop, no canned gunshots in the loop.
clip=bytearray(b'PSXA'+struct.pack('<HHI4H',1,0,8+120*3*30,3,120,30,0))
for f in range(120):
 for sign in [0,1,-1]:
  r=Matrix.Rotation(sign*math.tau*f/120,3,'Z');clip+=struct.pack('<9h3i',*[round(r[row][col]*4096) for col in range(3) for row in range(3)],0,0,0)
(D/'iris-spin.psxanim').write_bytes(clip)
report={'triangles':len(faces),'vertices':len(verts),'joints':3,'unit':unit,'source':'aletha-crystal-iris.blend','animation':'counter-rotating collars, 120 frames / 30 Hz'}
(S/'game-export.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT',json.dumps(report))
