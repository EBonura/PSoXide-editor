"""Apply the reviewed foot-only vertex correction without reimporting the skeleton."""
import bpy,json,struct,shutil,hashlib
import numpy as np
from pathlib import Path
from mathutils import Vector
O=Path(__file__).resolve().parent;P=O.parents[2];R=P/'validation/aletha-flat-feet-v1';B=Path('/tmp/aletha-flat-feet-originals');B.mkdir(exist_ok=True)
changes=json.loads((O/'vertex-changes.json').read_text());fit={'scale':30913.740766262457,'offset':[-.00022368840526149604,-28068.257188085474,-5538.745611154226]}
def gl(v):return np.array([v[0],v[2],-v[1]])
def backup(p):
 dest=B/p.relative_to(P);dest.parent.mkdir(parents=True,exist_ok=True)
 if not dest.exists():shutil.copy2(p,dest)
 return dest
reports={}
# In-place PSMD vertex edits retain skeleton, weights, indices and texture layout.
for name in ['aletha_closed_458','aletha_mirror_458']:
 p=P/f'assets/models/aletha_closed_458/{name}.psxmdl';old=backup(p).read_bytes();b=bytearray(old)
 jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);vo=28+4*jc+8*mc+16*pc;fo=vo+vc*8
 points=np.array([struct.unpack_from('<3h',b,vo+i*8) for i in range(vc)])
 changed=[]
 for c in changes:
  q=gl(c['old'])*fit['scale']+fit['offset'];dist=np.linalg.norm(points-q,axis=1);i=int(dist.argmin());assert dist[i]<1.5,(c,i,dist[i]);new=np.rint(gl(c['new'])*fit['scale']+fit['offset']).astype(int);struct.pack_into('<3h',b,vo+i*8,*new);changed.append(i)
 assert len(set(changed))==24
 faces=[]
 for fi in range(fc):
  ids=[struct.unpack_from('<H',b,fo+fi*12+k*4)[0] for k in range(3)]
  if not set(ids)&set(changed):continue
  faces.append(fi)
  ps=np.array([struct.unpack_from('<3h',b,vo+i*8) for i in ids],float);n=np.cross(ps[2]-ps[0],ps[1]-ps[0]);assert np.linalg.norm(n)>1
  if name.endswith('mirror_458'):
   normal=np.rint(n/np.linalg.norm(n)*127).astype(int)
   for k in range(3):b[fo+fi*12+(k//2)*4+2+k%2]=int(normal[k])&255
   drop=np.abs(n).argmax();uv=ps[:,[a for a in range(3) if a!=drop]];uv-=uv.mean(0);uv/=max(np.abs(uv).max(),1);q=np.rint((uv+1)*1.5).astype(int).clip(0,3);bits=0x8000
   for k in range(3):bits|=(int(q[k,0])|(int(q[k,1])<<2))<<(k*4)
   struct.pack_into('<H',b,fo+fi*12+10,bits)
 for i in range(vc):
  assert b[vo+i*8+6:vo+i*8+8]==old[vo+i*8+6:vo+i*8+8]
  if i not in changed:assert b[vo+i*8:vo+i*8+8]==old[vo+i*8:vo+i*8+8]
 for fi in range(fc):
  for k in range(3):o=fo+fi*12+k*4;assert b[o:o+2]==old[o:o+2]
 assert b[:vo]==old[:vo] and len(b)==len(old)
 p.write_bytes(b);reports[name]={'vertices_changed':len(changed),'faces_reshaped':len(faces),'triangles':fc,'sha256':hashlib.sha256(b).hexdigest(),'weights_indices_and_header_unchanged':True}
# Patch duplicated glTF positions directly; retain every animation, accessor and skin.
p=P/'source_assets/characters/aletha_closed_458/Aletha-closed-458.glb';old=backup(p).read_bytes();n=struct.unpack_from('<I',old,12)[0];doc=json.loads(old[20:20+n]);binary=bytearray(old[28+n:]);count=0
for prim in doc['meshes'][0]['primitives']:
 a=doc['accessors'][prim['attributes']['POSITION']];v=doc['bufferViews'][a['bufferView']];stride=v.get('byteStride',12);base=v.get('byteOffset',0)+a.get('byteOffset',0);points=[]
 for i in range(a['count']):
  off=base+i*stride;xyz=np.array(struct.unpack_from('<3f',binary,off))
  for c in changes:
   if np.linalg.norm(xyz-gl(c['old']))<1e-6:xyz=gl(c['new']);struct.pack_into('<3f',binary,off,*xyz);count+=1;break
  points.append(xyz)
 a['min']=np.min(points,axis=0).tolist();a['max']=np.max(points,axis=0).tolist()
assert count>24
j=json.dumps(doc,separators=(',',':')).encode();j+=b' '*((-len(j))%4);p.write_bytes(struct.pack('<III',0x46546c67,2,28+len(j)+len(binary))+struct.pack('<II',len(j),0x4e4f534a)+j+struct.pack('<II',len(binary),0x004e4942)+binary);reports['glb_position_corners_updated']=count
# Keep editable master meshes in step. The rig, action keys and ankle seam stay intact.
for name in ['Aletha-closed-458','Aletha-crystal-458']:
 p=P/f'source_assets/characters/aletha_closed_458/{name}.blend';backup(p);bpy.ops.wm.open_mainfile(filepath=str(p));mesh=bpy.data.objects['Aletha optimized'];inv=mesh.matrix_world.inverted()
 for c in changes:
  v=mesh.data.vertices[c['index']];pos=mesh.matrix_world@v.co
  assert min((pos-Vector(c['old'])).length,(pos-Vector(c['new'])).length)<1e-5
  v.co=inv@Vector(c['new'])
 mesh.data.update();bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(p))
(O/'installation.json').write_text(json.dumps(reports,indent=2)+'\n');print('RESULT',json.dumps(reports))
