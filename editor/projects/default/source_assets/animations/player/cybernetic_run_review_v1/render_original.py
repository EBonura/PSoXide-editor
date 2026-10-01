import bpy,struct,re,json,math
import numpy as np
from pathlib import Path
from mathutils import Vector
out=Path(__file__).resolve().parent;project=out.parents[3];p=project/'review/run-v1/original';p.mkdir(parents=True,exist_ok=True);repo=project.parents[2];gen=repo/'engine/examples/editor-playtest/generated';folder=gen/'models/model_003_aletha_delivered'
# Decode the actual cooked mesh and animation poses, not unused GLB takes.
b=(folder/'mesh.psxmdl').read_bytes();nj,np_,nv,nf,nm,tw,th,scale=struct.unpack_from('<8H',b,12);off=28+nj*4+nm*8;parts=[struct.unpack_from('<8H',b,off+i*16) for i in range(np_)];off+=np_*16
verts=np.array([struct.unpack_from('<3h',b,off+i*8) for i in range(nv)],float);secondary=np.array([b[off+i*8+6] for i in range(nv)]);weights=np.array([b[off+i*8+7]/255 for i in range(nv)]);off+=nv*8;faces=[tuple(struct.unpack_from('<H',b,off+i*12+k*4)[0] for k in range(3)) for i in range(nf)];primary=np.zeros(nv,int)
for j,v,n,*_ in parts:primary[v:v+n]=j
secondary=np.where(secondary==255,primary,secondary);height=verts[:,1].max()-verts[:,1].min();units=1.878/height

def decode(path):
 b=path.read_bytes();ver=struct.unpack_from('<H',b,4)[0];j,n,hz,shift=struct.unpack_from('<4H',b,12);stride={1:30,2:24,3:20,4:16,5:16}[ver];dstart=20+((n*j*2+3)//4)*4 if ver==5 else 20;arr=[]
 def q11(c):return 4096 if c==2047 else (c if c<2048 else c-4096)*2
 def rnd(x):return int(math.copysign((abs(int(x))+2048)//4096,x))
 for i in range(n*j):
  idx=struct.unpack_from('<H',b,20+i*2)[0] if ver==5 else i;o=dstart+idx*stride
  if ver<=2:flat=list(struct.unpack_from('<9h',b,o));to=o+18
  else:
   flat=[]
   for k in range(3 if ver>=4 else 4):
    x=int.from_bytes(b[o+k*3:o+k*3+3],'little');flat.extend([q11(x&4095),q11((x>>12)&4095)])
   if ver==3:flat.append(q11(int.from_bytes(b[o+12:o+14],'little')&4095));to=o+14
   else:
    cross=np.cross(flat[:3],flat[3:6]);third=[rnd(v) for v in cross];axis=b[o+9]&3;c=b[o+9]>>2;c=c if c<32 else c-64
    if axis<3:third[axis]+=c
    flat.extend([max(-4096,min(4096,v)) for v in third]);to=o+10
  t=np.array(struct.unpack_from('<3i' if ver==1 else '<3h',b,to),float)*(1 if ver==1 else 1<<shift);arr.append((np.array(flat,float).reshape(3,3).T/4096,t))
 return np.array([v[0] for v in arr]).reshape(n,j,3,3),np.array([v[1] for v in arr]).reshape(n,j,3),n,hz
m=(gen/'level_manifest.cooked.rs').read_text();line=next(l for l in m.splitlines() if 'LevelCharacterRecord { model: ModelIndex(3)' in l)
def array(name):return [int(x.strip()) for x in re.search(name+r': \[([^]]+)\]',line)[1].split(',')]
flags=array('action_flags');speeds=array('action_speeds');ranges=[tuple(map(int,a)) for a in re.findall(r'CharacterActionFrameRange \{ start: (\d+), end: (\d+) \}',line)]
choices=[('run',6,2)]
clips={}
for name,cid,aid in choices:
 f=next(folder.glob(f'clip_{cid:02}_*.psxanim'));rot,trans,n,hz=decode(f);start,end=ranges[aid];end=min(end,n-1);start=min(start,end);inplace=bool(flags[aid]&4) if flags[aid]&2 else True
 def deform(frame,rot=rot,trans=trans,n=n,start=start,end=end,inplace=inplace):
  a=int(frame);z=min(a+1,end) if not flags[aid]&1 else (a+1 if a<end else start);u=frame-a;R=rot[a]*(1-u)+rot[z]*u;T=trans[a]*(1-u)+trans[z]*u
  if inplace:
   delta=T[0]-trans[0,0];delta[1]=0;T=T-delta
  v1=np.einsum('nij,nj->ni',R[primary],verts)+T[primary];v2=np.einsum('nij,nj->ni',R[secondary],verts)+T[secondary];v=v1*(1-weights[:,None])+v2*weights[:,None];return v[:,[0,2,1]]*np.array([1,-1,1])*units
 # Freeze closure's loop mode as an explicit argument to avoid late binding.
 clips[name]={'path':str(f),'rot':rot,'trans':trans,'n':n,'hz':hz,'start':start,'end':end,'inplace':inplace,'loop':bool(flags[aid]&1),'speed':speeds[aid]}

def points(c,phase):
 a=int(phase);z=(a+1 if a<c['end'] else c['start']) if c['loop'] else min(a+1,c['end']);u=phase-a;R=c['rot'][a]*(1-u)+c['rot'][z]*u;T=c['trans'][a]*(1-u)+c['trans'][z]*u
 if c['inplace']:
  delta=T[0]-c['trans'][0,0];delta[1]=0;T=T-delta
 v1=np.einsum('nij,nj->ni',R[primary],verts)+T[primary];v2=np.einsum('nij,nj->ni',R[secondary],verts)+T[secondary];v=v1*(1-weights[:,None])+v2*weights[:,None];return v[:,[0,2,1]]*np.array([1,-1,1])*units

bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False);s=bpy.context.scene;s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=480;s.render.resolution_y=480;s.render.resolution_percentage=100;s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.display.shading.cavity_type='BOTH';s.display.shading.background_type='WORLD';s.world.color=(.055,.065,.085);s.view_settings.view_transform='Standard';s.render.image_settings.file_format='PNG'
mesh=bpy.data.meshes.new('Cooked Aletha');mesh.from_pydata(points(clips['run'],0),[],faces);mesh.update();ob=bpy.data.objects.new('Cooked Aletha',mesh);s.collection.objects.link(ob);ob.color=(.22,.64,.74,1)
bpy.ops.mesh.primitive_plane_add(size=20);floor=bpy.context.object;floor.color=(.24,.27,.32,1)
for axis in [0,1]:
 for i in range(-8,9):
  bpy.ops.mesh.primitive_cube_add(size=1,location=(i*.25 if axis==0 else 0,i*.25 if axis==1 else 0,.001));g=bpy.context.object;g.scale=(.006,4,.001) if axis==0 else (4,.006,.001);g.color=(.39,.43,.48,1)
bpy.ops.object.camera_add();cam=bpy.context.object;s.camera=cam;cam.data.type='ORTHO';cam.data.ortho_scale=2.8
report={}
for name,c in clips.items():
 samples=[points(c,f) for f in range(c['start'],c['end']+1)];floorz=min(v[:,2].min() for v in samples);center=np.mean(samples[0][:,[0,1]],axis=0);step=((c['hz']*4096//60)*c['speed'])//256
 report[name]={k:c[k] for k in ['path','n','hz','start','end','inplace','loop','speed']};report[name].update(period_seconds=(c['end']-c['start']+(1 if c['loop'] else 0))*4096/(step*60),seam_max_vertex_displacement_m=float(np.linalg.norm(samples[-1]-samples[0],axis=1).max()),body_height_range_m=[float(v[:,2].max()-v[:,2].min()) for v in samples])
 for view,d in [('front',(0,-6,1.1)),('quarter',(3.8,-5.4,1.3)),('side',(6,0,1.0))]:
  target=Vector((0,0,.96));cam.location=target+Vector(d);cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();folderout=p/(name+'-'+view);folderout.mkdir(exist_ok=True)
  count=18
  for f in range(count):
   raw=f*2*step/4096
   if c['loop']:phase=c['start']+raw%(c['end']-c['start']+1)
   else:phase=c['start']+min(raw,c['end']-c['start'])
   v=points(c,phase);v[:,2]-=floorz;v[:,0]-=center[0];v[:,1]-=center[1];mesh.vertices.foreach_set('co',v.ravel());mesh.update();s.render.filepath=str(folderout/f'{f:03}.png');bpy.ops.render.render(write_still=True)
 print('CLIP_DONE',name,report[name]['period_seconds'],flush=True)
(p/'measurements.json').write_text(json.dumps(report,indent=2));print('RESULT cooked movement review complete')
