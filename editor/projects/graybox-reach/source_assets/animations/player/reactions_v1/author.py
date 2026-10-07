"""Aletha reaction studies. Review candidates only: does not change project bindings."""
import bpy,sys,json,math,re,struct
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
O=Path(__file__).resolve().parent;P=O.parents[3];REPO=P.parents[2];V=P/'validation/player-reactions-v1';B=REPO/'build/graybox-reach/player-reactions-v1'
sys.path.insert(0,str(O.parent/'cybernetic_walk_transitions_review_v4'));from decode_clip import decode
print('BLENDER',bpy.app.version_string,flush=True)
bpy.ops.wm.open_mainfile(filepath=str(P/'source_assets/characters/aletha_closed_458/Aletha-closed-458.blend'))
s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');mesh=bpy.data.objects['Aletha optimized'];r.animation_data_clear()
for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update();world=r.matrix_world.copy();inv=world.inverted();names=[b.name for b in r.data.bones];rest={n:world@r.data.bones[n].matrix_local for n in names}
blob=(P/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes();j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16
raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float);verts=np.array([mesh.matrix_world@v.co for v in mesh.data.vertices]);C=Matrix(((1,0,0),(0,0,-1),(0,1,0)));rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=np.ptp(verts[:,2])/np.ptp(rv[:,2]);center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())
text=(P/'project.ron').read_text()
def path(id):return P/re.search(r'            id: \('+str(id)+r'\),.*?psxanim_path: "([^"]+)"',text,re.S)[1]
def apply_source(R,T):
 for i,n in enumerate(names):
  rr=C@Matrix(R[i].tolist())@C.transposed();skin=rr.to_4x4();skin.translation=C@Vector(T[i].tolist())*unit+center-rr@center
  r.pose.bones[n].matrix=inv@skin@rest[n];bpy.context.view_layer.update()
def wm(n):return world@r.pose.bones[n].matrix
def setwm(n,m):r.pose.bones[n].matrix=inv@m;bpy.context.view_layer.update()
def capture():return {n:r.pose.bones[n].matrix_basis.copy() for n in names}
def apply(p):
 for n,m in p.items():r.pose.bones[n].matrix_basis=m
 bpy.context.view_layer.update()
def turn(n,axis,degrees):
 m=wm(n);l,q,sc=m.decompose();setwm(n,Matrix.LocRotScale(l,Quaternion(axis,math.radians(degrees))@q,sc))
def aim(n,dq):
 m=wm(n);l,q,sc=m.decompose();setwm(n,Matrix.LocRotScale(l,dq@q,sc))
def keys(f,knots):
 if f<=knots[0][0]:return knots[0][1]
 for (a,x),(b,y) in zip(knots,knots[1:]):
  if f<=b:
   t=(f-a)/(b-a);t=t*t*(3-2*t);return x+(y-x)*t
 return knots[-1][1]
def arm(side,target,pole):
 a,b,c=[side+t for t in ['UpperArm','LowerArm','Hand']];h=wm(a).translation;k0=wm(b).translation;f0=wm(c).translation
 L1=(k0-h).length;L2=(f0-k0).length;d=target-h;dist=min(d.length,.975*(L1+L2));axis=d.normalized();bend=pole-h;bend=(bend-axis*bend.dot(axis)).normalized()
 along=(L1*L1-L2*L2+dist*dist)/(2*dist);k=h+axis*along+bend*math.sqrt(max(0,L1*L1-along*along))
 aim(a,(k0-h).normalized().rotation_difference((k-h).normalized()));km=wm(b).translation;fm=wm(c).translation;aim(b,(fm-km).normalized().rotation_difference((h+axis*dist-km).normalized()))
def leg(side,target):
 a,b,c=[side+t for t in ['UpperLeg','LowerLeg','Foot']];h=wm(a).translation;k0=wm(b).translation;f0=wm(c).translation
 L1=(k0-h).length;L2=(f0-k0).length;d=target.translation-h;dist=d.length;axis=d.normalized();reach=dist/(L1+L2)
 assert reach<.9999,(side,reach)
 pole=Vector((0,-1,0));pole=(pole-axis*pole.dot(axis)).normalized();along=(L1*L1-L2*L2+dist*dist)/(2*dist);k=h+axis*along+pole*math.sqrt(max(0,L1*L1-along*along))
 aim(a,(k0-h).normalized().rotation_difference((k-h).normalized()));km=wm(b).translation;fm=wm(c).translation;aim(b,(fm-km).normalized().rotation_difference((target.translation-km).normalized()));setwm(c,target)
 return reach
R,T,n,hz=decode(path(45));apply_source(R[min(84,n-1)],T[min(84,n-1)]);base=capture();feet={side:wm(side+'Foot') for side in ['Left','Right']}
# Smoothstep beats: chest impact first; pelvis and head trail. One recovery foot.
beats={'poise':{'fps':30,'end':24,'beats':{'impact':3,'collapse':7,'brace':11,'recover':18,'ready':24}},'hit':{'fps':30,'end':18,'beats':{'impact':3,'recoil':5,'recover':11,'ready':18}}}
(O/'beats.json').write_text(json.dumps(beats,indent=2))
sequences={};report={'model':{'vertices':nv,'triangles':nf,'joints':j},'candidate_only':True,'clips':{}}
for label,N in [('poise',24),('hit',18)]:
 seq=[];checks=[];samples=[]
 for sample in range(2*N+1):
  f=sample/2
  apply(base);large=label=='poise'
  force=keys(f,[(0,0),(3,1),(6,.92),(11,.66),(18,.15),(24,0)]) if large else keys(f,[(0,0),(3,1),(5,.85),(10,.35),(15,.04),(18,0)])
  weight=keys(f,[(0,0),(2,.1),(7,1),(11,1),(18,.25),(24,0)]) if large else keys(f,[(0,0),(2,0),(5,1),(9,.6),(14,.1),(18,0)])
  drag=keys(f,[(0,0),(2,0),(5,1),(10,.9),(17,.22),(24,0)]) if large else keys(f,[(0,0),(2,0),(5,1),(10,.4),(18,0)])
  m=wm('Hips');m.translation+=Vector((-.038*weight, .085*weight, -.115*weight)) if large else Vector((-.010*weight,.025*weight,-.025*weight));setwm('Hips',m)
  turn('Hips',(0,0,1),-5*weight if large else -2*weight)
  pitch=keys(f,[(0,0),(2,-10),(6,29),(10,25),(17,8),(24,0)]) if large else 11*force
  turn('Spine',(1,0,0),pitch*.4);turn('Chest',(1,0,0),pitch*.6)
  turn('Chest',(0,0,1),-16*force if large else -8*force);turn('Chest',(0,1,0),-6*force)
  turn('Neck',(1,0,0),-10*drag);turn('Neck',(0,0,1),7*drag)
  # Free hand reflexively shields the ribs; weapon arm swings off line,
  # then collects before returning to guard. Each elbow follows a real arc.
  gather=keys(f,[(0,0),(2,.08),(6,1),(11,.95),(18,.28),(24,0)]) if large else drag*.45
  chest=wm('Chest').translation
  if gather>1e-7:
   left=wm('LeftHand').translation.lerp(chest+Vector((.08,-.235,-.13)),gather)
   right=wm('RightHand').translation.lerp(chest+Vector((-.34,-.16,-.30)),gather*.85)
   arm('Left',left,chest+Vector((.40,-.01,-.22)))
   arm('Right',right,chest+Vector((-.47,.04,-.15)))
  for side in ['Left','Right']:
   target=feet[side].copy()
   if large and side=='Left':
    step=keys(f,[(0,0),(3,0),(8,1),(13,1),(22,0),(24,0)])
    lift=keys(f,[(0,0),(3,0),(5,1),(8,0),(13,0),(17,.8),(22,0),(24,0)])
    target.translation+=Vector((.01*step,.18*step,.06*lift))
   checks.append(leg(side,target));samples.append({'frame':f,'foot':side,'position':list(wm(side+'Foot').translation),'target_error':(wm(side+'Foot').translation-target.translation).length})
  seq.append(capture())
 sequences[label]=seq;report['clips'][label]={'frames':len(seq),'sample_hz':60,'seconds':N/30,'max_leg_extension':max(checks),'max_target_error':max(v['target_error'] for v in samples),'foot_samples':samples}
# Old clip posed at the actual game playback rate, and the disconnected Stun.
for label,id,speed in [('old_active',87,4),('old_stun',54,1)]:
 R,T,n,hz=decode(path(id));duration=(n-2)/hz/speed;N=max(1,math.ceil(duration*30));seq=[]
 for f in range(N+1):
  k=min(f/30*hz*speed,n-2);i=int(k);t=k-i;rot=[]
  for jn in range(j):rot.append(np.array(Matrix(R[i,jn].tolist()).to_quaternion().slerp(Matrix(R[min(i+1,n-1),jn].tolist()).to_quaternion(),t).to_matrix()))
  trans=T[i]*(1-t)+T[min(i+1,n-1)]*t
  if label=='old_active':trans=trans.copy();delta=trans[0]-T[0,0];delta[1]=0;trans-=delta
  apply_source(np.array(rot),trans);seq.append(capture())
 sequences[label]=seq;report['clips'][label]={'stored_frames':n,'sample_hz':hz,'speed':speed,'seconds':duration,'source':str(path(id))}
# Save editable rigs for review; candidate exports include final guard sentinel.
bpy.context.preferences.filepaths.save_version=0
for ob in s.objects:
 if ob.type=='MESH':ob.hide_render=ob!=mesh;ob.hide_set(ob!=mesh)
mesh.hide_render=False;mesh.hide_set(False)
for label,seq in sequences.items():
 fps=60 if label in ('poise','hit') else 30
 r.animation_data_clear();prev={};data=bytearray(struct.pack('<4sHHI4H',b'PANM',1,0,0,j,len(seq)+1,fps,0))
 for f,pose in enumerate(seq+[seq[-1]]):
  apply(pose)
  for name in names:
   b=r.pose.bones[name];loc,q,sc=b.matrix_basis.decompose();b.rotation_mode='QUATERNION'
   if name in prev and prev[name].dot(q)<0:q.negate()
   prev[name]=q.copy();b.location=loc;b.rotation_quaternion=q;b.scale=sc
   for prop in ['location','rotation_quaternion','scale']:b.keyframe_insert(prop,frame=f,group=name)
   skin=wm(name)@rest[name].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C;tt=C.transposed()@(skin.translation-center+rot@center)/unit
   data.extend(struct.pack('<9h3i',*[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)],*[round(v) for v in tt]))
 act=r.animation_data.action;act.name='Aletha_'+label;act.use_fake_user=True
 for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
  for k in fc.keyframe_points:k.interpolation='LINEAR'
 s.frame_start=0;s.frame_end=len(seq)-1;s.render.fps=fps;s.frame_set(0)
 bpy.ops.wm.save_as_mainfile(filepath=str(O/(label+'.blend')))
 if label in ('poise','hit'):
  data[:4]=path(87).read_bytes()[:4];struct.pack_into('<I',data,8,len(data)-12);(O/(label+'.psxanim')).write_bytes(data)
(V/'audit.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({k:{x:y for x,y in v.items() if x!='foot_samples'} for k,v in report['clips'].items()}),flush=True)
