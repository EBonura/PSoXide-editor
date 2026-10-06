from pathlib import Path
import struct,json,numpy as np
P=Path(__file__).resolve().parents[3];OUT=Path(__file__).resolve().parent;src=P/'assets/models/enemy_reduced/light.psxmdl';b=bytearray(src.read_bytes());jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);vo=28+jc*4+mc*8+pc*16;verts=np.array([struct.unpack_from('<3h',b,vo+i*8) for i in range(vc)],dtype=float);cc=json.loads((OUT/'components.json').read_text());changed=[]
def component(cid,fn):
 ids=cc[cid]['verts'];before=verts[ids].copy();verts[ids]=np.array([fn(p.copy()) for p in before]);changed.append({'component':cid,'vertices':len(ids),'max_move':float(np.linalg.norm(verts[ids]-before,axis=1).max())})
cx=-2600.5
# Lift and taper the lower cuirass, keeping the collar connection in place.
def chest(p):
 x,y,z=p;level=np.clip((y-7612)/(14442-7612),0,1);x=cx+(x-cx)*(.67+.48*level);y=14442+(y-14442)*.85
 # Forward upper breast planes, recessed lower point.
 z=z+500*level-280*(1-level)
 return x,y,z
component(3,chest)
# Keep the small collar insert tucked inside the reshaped chest.
component(4,lambda p:np.array([cx+(p[0]-cx)*1.05,14442+(p[1]-14442)*.94,p[2]+180]))
# A narrower waist makes the existing pipes and the gap below the chest read.
component(2,lambda p:np.array([cx+(p[0]-cx)*.83,p[1],-1500+(p[2]+1500)*.85]))
component(0,lambda p:np.array([cx+(p[0]-cx)*.91,p[1],-1400+(p[2]+1400)*.92]))
# Longer cranium, slimmer cheeks. The round optic is scaled uniformly.
component(6,lambda p:np.array([cx+(p[0]-cx)*.94,16250+(p[1]-16250)*1.04,3000+(p[2]-3000)*1.08]))
component(7,lambda p:np.array([cx+(p[0]-cx)*.96,16277+(p[1]-16277)*.96,5200+(p[2]-5200)*.96+175]))
# Armour only; joint housings and exposed connector rods remain unchanged.
# Give the shoulder/upper-arm guard a raised, swept ridge and a narrow elbow end.
for cid,pivot,side in [(9,2233,1),(30,-6329,-1)]:
 lo,hi=cc[cid]['min'][0],cc[cid]['max'][0];length=hi-lo
 def armour(p):
  u=np.clip(abs(p[0]-pivot)/length,0,1);bulk=1.28-.4*u;return np.array([p[0],12200+(p[1]-12200)*bulk+380*(1-u),200+(p[2]-200)*(1.12-.2*u)])
 component(cid,armour)
# Expand the upper edge of the large thigh plate and sharpen its tapered hem.
for cid,pivotx in [(43,-5775),(35,800)]:
 lo,hi=cc[cid]['min'][1],cc[cid]['max'][1]
 def thigh(p):
  u=np.clip((p[1]-lo)/(hi-lo),0,1);factor=(.78+.4*u) if cid==43 else (.86+.19*u);return np.array([pivotx+(p[0]-pivotx)*factor,p[1],p[2]+(450*u if cid==43 else 120*u)])
 component(cid,thigh)
verts=np.rint(verts).astype(int);assert np.abs(verts).max()<32768
for i,p in enumerate(verts):struct.pack_into('<3h',b,vo+i*8,*p)
fo=vo+vc*8
for i in range(fc):
 idx=[struct.unpack_from('<H',b,fo+i*12+k*4)[0] for k in range(3)];a,c,d=verts[idx];assert np.linalg.norm(np.cross(c-a,d-a))>0
(OUT/'light-body-study.psxmdl').write_bytes(b);(OUT/'changes.json').write_text(json.dumps({'triangles':fc,'vertices':vc,'changes':changed,'unchanged':'Skeleton, weights, face indices, UVs, palette banks, cannon, claw, feet, joints and connector rods'},indent=2));print('RESULT',fc,vc)
