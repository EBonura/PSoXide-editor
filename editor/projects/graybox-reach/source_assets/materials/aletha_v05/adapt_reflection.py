from pathlib import Path
import numpy as np,struct
import sys
source=Path(sys.argv[1]); destination=Path(sys.argv[2]); original=source.read_bytes()
destination.parent.mkdir(parents=True,exist_ok=True)
# Pack flat bind normals into otherwise unused UV bytes of a reflection-only model copy.
b=bytearray(original);jc,pc,vc,fc,mc,tw,th,l2w=struct.unpack_from('<8H',b,12);po=28+4*jc+8*mc;vo=po+16*pc;fo=vo+8*vc
parts=[struct.unpack_from('<6H',b,po+i*16) for i in range(pc)];verts=[struct.unpack_from('<3hBB',b,vo+i*8) for i in range(vc)];owners={v:p[0] for p in parts for v in range(p[1],p[1]+p[2])}
for fi in range(fc):
 ids=[struct.unpack_from('<H',b,fo+fi*12+k*4)[0] for k in range(3)];ps=np.array([verts[i][:3] for i in ids],dtype=float);normal=np.cross(ps[2]-ps[0],ps[1]-ps[0]);length=np.linalg.norm(normal); normal=normal/length if length else np.array([0.,0.,-1.]);normal=np.rint(normal*127).astype(int)
 influence=np.zeros(jc)
 for i in ids:
  _,_,_,secondary,blend=verts[i];influence[owners[i]]+=255-blend
  if secondary<jc:influence[secondary]+=blend
 joint=int(influence.argmax());data=[int(normal[0])&255,int(normal[1])&255,int(normal[2])&255,joint,0,0]
 for k in range(3):struct.pack_into('<BB',b,fo+fi*12+k*4+2,*data[k*2:k*2+2])
model=destination;model.write_bytes(b)
# Preserve the normal/joint stream; only use its reserved final UV word.
f=destination;b=bytearray(f.read_bytes());jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);po=28+4*jc+8*mc;vo=po+16*pc;fo=vo+8*vc
verts=[struct.unpack_from('<3hBB',b,vo+i*8)[:3] for i in range(vc)]
for fi in range(fc):
 ids=[struct.unpack_from('<H',b,fo+fi*12+k*4)[0] for k in range(3)];p=np.array([verts[i] for i in ids],dtype=float);n=np.cross(p[2]-p[0],p[1]-p[0]);drop=np.abs(n).argmax();axes=[a for a in range(3) if a!=drop];uv=p[:,axes];uv-=uv.mean(axis=0);uv=uv/max(np.abs(uv).max(),1);q=np.rint((uv+1)*1.5).astype(int).clip(0,3);bits=0x8000
 for k in range(3):bits|=(int(q[k,0])|(int(q[k,1])<<2))<<(k*4)
 struct.pack_into('<H',b,fo+fi*12+10,bits)
struct.pack_into('<H',b,6,struct.unpack_from('<H',b,6)[0]|64)
f.write_bytes(b)
# Only reflection metadata and the facet flag may change.
assert len(b)==len(original)
assert b[:6]==original[:6] and b[8:fo]==original[8:fo]
assert b[fo+fc*12:]==original[fo+fc*12:]
for fi in range(fc):
 for k in range(3):
  o=fo+fi*12+k*4; assert b[o:o+2]==original[o:o+2]
print(f'{fc} triangles: reflection metadata packed; geometry, skinning, face indices and palette data unchanged')
