from pathlib import Path
import struct,json,numpy as np
O=Path(__file__).resolve().parent;P=Path(__file__).resolve().parents[3]
def read(path):
 b=path.read_bytes();jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);po=28+jc*4+mc*8;vo=po+pc*16;fo=vo+vc*8;parts=[struct.unpack_from('<6H',b,po+i*16) for i in range(pc)];owners={i:p[0] for p in parts for i in range(p[1],p[1]+p[2])};fowners={i:p[0] for p in parts for i in range(p[3],p[3]+p[4])}
 return b,po,parts,[{'p':list(struct.unpack_from('<3h',b,vo+i*8)),'sec':b[vo+i*8+6],'w':b[vo+i*8+7],'owner':owners[i]} for i in range(vc)],[{'corners':[struct.unpack_from('<HBB',b,fo+i*12+k*4) for k in range(3)],'bank':(b[fo+fc*12+i//4]>>((i%4)*2))&3,'owner':fowners[i]} for i in range(fc)]
b,po,parts,verts,faces=read(O/'light-body-study.psxmdl');_,_,_,ov,of=read(P.parent/'default/assets/models/rust_mantis/rust_mantis.psxmdl');cc=json.loads((O/'components.json').read_text());old=json.loads((P/'tools/enemy_reduction/components.json').read_text())['light'];drop=set();added=[]
for cid in [11,33]:
 drop.update(cc[cid]['faces']);mapping={i:len(verts)+k for k,i in enumerate(old[cid]['verts'])}
 verts.extend(ov[i] for i in old[cid]['verts'])
 for fi in old[cid]['faces']:
  f=of[fi];added.append({**f,'corners':[(mapping[i],u,v) for i,u,v in f['corners']]})
# Two folded breast plates meet at a recessed centre; the bottom seam rises
# into a chevron. Split adjacent faces too, so the cuirass stays watertight.
edge_new={}
for a,c,dy,dz in [(45,31,0,-550),(41,32,1150,-450)]:
 p=np.rint((np.array(verts[a]['p'])+verts[c]['p'])/2).astype(int);p[1]+=dy;p[2]+=dz;idx=len(verts);verts.append({**verts[a],'p':p.tolist()});edge_new[frozenset([a,c])]=idx
A=edge_new[frozenset([45,31])];B=edge_new[frozenset([41,32])];uv={45:(147,67),31:(173,67),41:(181,119),32:(138,117),A:(160,67),B:(160,118)}
for indices in [(45,A,32),(A,B,32),(A,31,B),(31,41,B)]:added.append({'corners':[(i,*uv[i]) for i in indices],'bank':3,'owner':3})
drop.update([62,63]);outfaces=[]
for fi,f in enumerate(faces):
 if fi in drop:continue
 corners=f['corners'];split=False
 for k in range(3):
  a,c,d=corners[k],corners[(k+1)%3],corners[(k+2)%3];idx=edge_new.get(frozenset([a[0],c[0]]))
  if idx is not None:
   mid=(idx,round((a[1]+c[1])/2),round((a[2]+c[2])/2));outfaces.extend([{**f,'corners':[a,mid,d]},{**f,'corners':[mid,c,d]}]);split=True;break
 if not split:outfaces.append(f)
faces=outfaces+added;used={i for f in faces for i,u,v in f['corners']};vv=[];ff=[];pp=[];mapping={}
for part in parts:
 j=part[0];start=len(vv)
 for i,v in enumerate(verts):
  if i in used and v['owner']==j:mapping[i]=len(vv);vv.append(v)
 fst=len(ff);ff.extend(f for f in faces if f['owner']==j);pp.append([j,start,len(vv)-start,fst,len(ff)-fst,part[5]])
out=bytearray(b[:po]);struct.pack_into('<HH',out,16,len(vv),len(ff))
for i,p in enumerate(pp):out+=struct.pack('<6H',*p)+b[po+i*16+12:po+i*16+16]
for v in vv:out+=struct.pack('<3hBB',*v['p'],v['sec'],v['w'])
for f in ff:
 pts=np.array([verts[i]['p'] for i,u,v in f['corners']]);assert np.linalg.norm(np.cross(pts[1]-pts[0],pts[2]-pts[0]))>0
 for i,u,v in f['corners']:out+=struct.pack('<HBB',mapping[i],u,v)
pal=bytearray((len(ff)+3)//4)
for i,f in enumerate(ff):pal[i//4]|=f['bank']<<((i%4)*2)
out+=pal;struct.pack_into('<I',out,8,len(out)-12);(O/'light-body-study.psxmdl').write_bytes(out);print('RESULT',len(vv),len(ff))
