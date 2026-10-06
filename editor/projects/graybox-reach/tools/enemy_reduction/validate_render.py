import bpy,json,struct,math
import numpy as np
from pathlib import Path
from mathutils import Vector
from mathutils.bvhtree import BVHTree
exec((Path(__file__).parent/'scene_setup.py').read_text())
s.render.resolution_x=480;s.render.resolution_y=640;s.view_settings.view_transform='Standard'
def model(path):
 b=path.read_bytes();jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);po=28+4*jc+8*mc;vo=po+16*pc;fo=vo+8*vc;parts=[struct.unpack_from('<6H',b,po+i*16) for i in range(pc)];v=[struct.unpack_from('<3hBB',b,vo+i*8) for i in range(vc)];fs=[[struct.unpack_from('<HBB',b,fo+i*12+k*4) for k in range(3)] for i in range(fc)];owner={i:p[0] for p in parts for i in range(p[1],p[1]+p[2])};banks=b[fo+fc*12:];return {'b':b,'v':v,'f':fs,'owner':owner,'banks':banks,'joints':jc,'po':po}
def animation(path):
 b=path.read_bytes();ver=struct.unpack_from('<H',b,4)[0];jc,fc,hz,shift=struct.unpack_from('<4H',b,12);pose=[]
 def q(c):return 4096 if c==2047 else (c if c<2048 else c-4096)*2
 for fi in range(fc):
  row=[]
  for ji in range(jc):
   index=fi*jc+ji
   if ver==5:
    di=struct.unpack_from('<H',b,20+index*2)[0];off=(20+jc*fc*2+3)//4*4+di*16
   else:off=20+index*{1:30,2:24,3:20,4:16}[ver]
   if ver<=2:m=list(struct.unpack_from('<9h',b,off));t=struct.unpack_from('<3i' if ver==1 else '<3h',b,off+18)
   else:
    vals=[]
    for pair in range(3 if ver>=4 else 4):
     packed=int.from_bytes(b[off+pair*3:off+pair*3+3],'little');vals.extend([q(packed&4095),q((packed>>12)&4095)])
    if ver>=4:
     cross=np.cross(vals[:3],vals[3:6]);third=[int((x+2048)//4096) if x>=0 else -int((-x+2048)//4096) for x in cross];code=b[off+9];axis=code&3;correction=code>>2;correction=correction if correction<32 else correction-64
     if axis<3:third[axis]+=correction
     m=vals+[max(-4096,min(4096,x)) for x in third];t=struct.unpack_from('<3h',b,off+10)
    else:m=vals+[q(struct.unpack_from('<H',b,off+12)[0]&4095)];t=struct.unpack_from('<3h',b,off+14)
   row.append((np.array(m,dtype=float).reshape(3,3).T/4096,np.array(t,dtype=float)*(1 if ver==1 else 2**shift)))
  pose.append(row)
 return pose

def deform(m,pose):
 out=[]
 for i,v in enumerate(m['v']):
  p=np.array(v[:3]);a,t=pose[m['owner'][i]];p0=a@p+t
  if v[4]:a,t=pose[v[3]];p0=(p0*(255-v[4])+(a@p+t)*v[4])/255
  out.append(p0)
 return np.array(out)
def mesh(m,name):
 me=bpy.data.meshes.new(name);me.from_pydata([v[:3] for v in m['v']],[],[[c[0] for c in f] for f in m['f']]);ob=bpy.data.objects.new(name,me);s.collection.objects.link(ob)
 for mat in mats:me.materials.append(mat)
 uv=me.uv_layers.new(name='UVMap')
 for p,f in zip(me.polygons,m['f']):
  p.material_index=(m['banks'][p.index//4]>>((p.index%4)*2))&3
  for li,(_,u,v) in zip(p.loop_indices,f):uv.data[li].uv=(u/256,1-v/256)
 return ob
report={}
for name,rel,clips in [('light','rust_mantis/rust_mantis.psxmdl',['rust_mantis_starter/idle','rust_mantis_starter/walk','rust_mantis_starter/turn','mantis_combat/horizon_heavy','mantis_combat/charge_volley']),('heavy','tank_boss_animated_model/tank_boss_animated_model.psxmdl',['tank_boss_ai/idle','tank_boss_ai/walk_fwd','tank_boss_ai/turn','tank_boss_ai/heavy_attack','tank_boss_ai/ranged_attack'])]:
 orig=model(D/'assets/models'/rel);new=model(O/f'{name}-reduced.psxmdl');assert orig['b'][28:orig['po']]==new['b'][28:new['po']];height=np.ptp(np.array(orig['v'])[:,1]);objects=[mesh(orig,name+' original'),mesh(new,name+' reduced')]
 tris=[[c[0] for c in f] for f in new['f']];pos=[Vector(v[:3]) for v in new['v']];assert all((pos[t[1]]-pos[t[0]]).cross(pos[t[2]]-pos[t[0]]).length>0 for t in tris);assert len({tuple(sorted(t)) for t in tris})==len(tris)
 distances=[];tested=0;rendered=[]
 for clip in clips:
  poses=animation(D/'assets/animations'/(clip+'.psxanim'));assert len(poses[0])==orig['joints']
  # Every sampled frame keeps finite, bounded geometry; every 1/4 clip gets
  # bidirectional surface-distance checks, including changed blended pieces.
  for fi,pose in enumerate(poses):
   pp=[deform(m,pose) for m in [orig,new]];assert all(np.isfinite(p).all() for p in pp);tested+=1
   if fi not in {0,len(poses)//4,len(poses)//2,len(poses)*3//4,len(poses)-1}:continue
   bvhs=[BVHTree.FromPolygons([Vector(v) for v in p],[[c[0] for c in f] for f in m['f']],all_triangles=True) for m,p in zip([orig,new],pp)]
   dist=[bvhs[1-i].find_nearest(Vector(v))[3] for i in range(2) for v in pp[i]];distances.append(max(dist)/height*100)
  fi=0 if 'idle' in clip else len(poses)//2;pp=[deform(m,poses[fi]) for m in [orig,new]];center=(pp[0].max(0)+pp[0].min(0))/2
  for ob,p in zip(objects,pp):
   xyz=(p-center)/height*2
   for v,(x,y,z) in zip(ob.data.vertices,xyz):v.co=(x,-z,y)
   ob.data.update()
  direction=Vector((.45,-1,.1)).normalized();cam.location=direction*6;cam.rotation_euler=(-direction).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=max(2.45,(np.ptp(pp[0][:,0])/height*2)*640/480*1.15)
  bpy.context.view_layer.update()
  rotation=cam.rotation_euler.to_matrix().transposed();viewpoints=[rotation @ v.co for v in objects[0].data.vertices]
  cam.data.ortho_scale=max(max(abs(v.x) for v in viewpoints)*2*640/480,max(abs(v.y) for v in viewpoints)*2)*1.14
  for i,ob in enumerate(objects):
   for j,other in enumerate(objects):other.hide_render=i!=j
   filename=f'{name}-{clip.split("/")[-1]}-{["before","after"][i]}.png';s.render.filepath=str(O/filename);bpy.ops.render.render(write_still=True);rendered.append(filename)
  for ob in objects:ob.hide_render=True
 report[name]={'animation_frames_checked':tested,'clips':clips,'sampled_surface_max_error_percent_height':max(distances),'sampled_surface_max_errors_percent_height':distances,'renders':rendered}
 for ob in objects:ob.hide_render=True
(O/'validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({k:{a:b for a,b in v.items() if a!='renders'} for k,v in report.items()}))
