from pathlib import Path
import numpy as np,struct
P=Path(__file__).resolve().parents[2]
O=P/'source_assets/characters/aletha_closed_458'
# Pack flat bind normals into otherwise unused UV bytes of a reflection-only model copy.
b=bytearray((P/'assets/models/aletha_closed_458/aletha_closed_458.psxmdl').read_bytes());jc,pc,vc,fc,mc,tw,th,l2w=struct.unpack_from('<8H',b,12);po=28+4*jc+8*mc;vo=po+16*pc;fo=vo+8*vc
parts=[struct.unpack_from('<6H',b,po+i*16) for i in range(pc)];verts=[struct.unpack_from('<3hBB',b,vo+i*8) for i in range(vc)];owners={v:p[0] for p in parts for v in range(p[1],p[1]+p[2])}
for fi in range(fc):
 ids=[struct.unpack_from('<H',b,fo+fi*12+k*4)[0] for k in range(3)];ps=np.array([verts[i][:3] for i in ids],dtype=float);normal=np.cross(ps[2]-ps[0],ps[1]-ps[0]);normal/=np.linalg.norm(normal);normal=np.rint(normal*127).astype(int)
 influence=np.zeros(jc)
 for i in ids:
  _,_,_,secondary,blend=verts[i];influence[owners[i]]+=255-blend
  if secondary<jc:influence[secondary]+=blend
 joint=int(influence.argmax());data=[int(normal[0])&255,int(normal[1])&255,int(normal[2])&255,joint,0,0]
 for k in range(3):struct.pack_into('<BB',b,fo+fi*12+k*4+2,*data[k*2:k*2+2])
model=P/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl';model.write_bytes(b)
# Preserve the normal/joint stream; only use its reserved final UV word.
f=P/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl';b=bytearray(f.read_bytes());jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);po=28+4*jc+8*mc;vo=po+16*pc;fo=vo+8*vc
verts=[struct.unpack_from('<3hBB',b,vo+i*8)[:3] for i in range(vc)]
for fi in range(fc):
 ids=[struct.unpack_from('<H',b,fo+fi*12+k*4)[0] for k in range(3)];p=np.array([verts[i] for i in ids],dtype=float);n=np.cross(p[2]-p[0],p[1]-p[0]);drop=np.abs(n).argmax();axes=[a for a in range(3) if a!=drop];uv=p[:,axes];uv-=uv.mean(axis=0);uv=uv/max(np.abs(uv).max(),1);q=np.rint((uv+1)*1.5).astype(int).clip(0,3);bits=0x8000
 for k in range(3):bits|=(int(q[k,0])|(int(q[k,1])<<2))<<(k*4)
 struct.pack_into('<H',b,fo+fi*12+10,bits)
struct.pack_into('<H',b,6,struct.unpack_from('<H',b,6)[0]|64)
f.write_bytes(b)
print('458 triangles: reflection normals and corner gradients packed; PSMD metadata flag set')
