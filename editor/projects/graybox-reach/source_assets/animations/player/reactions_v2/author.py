"""Aletha reaction studies. Review candidates only: does not change project bindings."""
import bpy,sys,json,math,re,struct
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
O=Path(__file__).resolve().parent;P=O.parents[3];REPO=P.parents[2];V=P/'validation/player-reactions-v2';B=REPO/'build/graybox-reach/player-reactions-v2'
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
# A frontal off-centre strike drives the torso BACK before balance is recovered.
# Beat times below are 30 Hz authoring frames; export samples twice as densely.
# There is no anticipatory crouch. Pelvis, head, arms and recovery foot trail
# the chest impulse at different offsets.
basehands={side:wm(side+'Hand').copy() for side in ['Left','Right']}
beats={'poise':{'fps':30,'end':40,'beats':{'strike':1,'recoil':4,'loss_of_balance':8,'catch_step':11,'absorb':15,'reorient':23,'recover':33,'ready':40}},'hit':{'fps':30,'end':21,'beats':{'strike':1,'recoil':3,'head_lag':5,'arrest':8,'recover':15,'ready':21}}}
(O/'beats.json').write_text(json.dumps(beats,indent=2))
sequences={};report={'model':{'vertices':nv,'triangles':nf,'joints':j},'candidate_only':True,'clips':{}}
for label,N in [('poise',40),('hit',21)]:
 seq=[];checks=[];samples=[]
 for sample in range(2*N+1):
  f=sample/2;apply(base);large=label=='poise'
  if large:
   # Short hard impulse, then continuing momentum. Recovery is much slower.
   pitch=keys(f,[(0,0),(1,-18),(3,-32),(5,-35),(8,-31),(11,-19),(15,5),(20,9),(26,4),(35,0),(40,0)])
   yaw=keys(f,[(0,0),(1,16),(3,28),(6,33),(10,27),(15,15),(22,-5),(29,-3),(37,0),(40,0)])
   lean=keys(f,[(0,0),(2,-11),(5,-16),(9,-14),(15,-5),(22,3),(30,1),(38,0),(40,0)])
   back=keys(f,[(0,0),(1,.010),(3,.065),(7,.17),(11,.21),(15,.19),(23,.15),(32,.04),(38,0),(40,0)])
   lateral=keys(f,[(0,0),(3,-.025),(7,-.065),(11,-.080),(17,-.065),(25,-.025),(36,0),(40,0)])
   down=keys(f,[(0,0),(2,.003),(6,.027),(10,.055),(14,.090),(18,.065),(26,.035),(35,0),(40,0)])
   hp=keys(f,[(0,0),(2,0),(6,-7),(11,-6),(17,4),(26,2),(35,0),(40,0)])
   hy=keys(f,[(0,0),(2,0),(7,10),(13,13),(21,5),(30,0),(40,0)])
   neck=keys(f,[(0,0),(1,8),(3,-10),(5,-20),(8,-18),(12,-7),(17,11),(23,5),(32,0),(40,0)])
   headturn=keys(f,[(0,0),(2,-6),(5,14),(9,18),(14,3),(21,-7),(30,0),(40,0)])
  else:
   pitch=keys(f,[(0,0),(1,-16),(2,-23),(4,-25),(6,-17),(9,-5),(12,4),(16,2),(21,0)])
   yaw=keys(f,[(0,0),(1,-13),(3,-23),(5,-22),(8,-14),(12,3),(17,1),(21,0)])
   lean=keys(f,[(0,0),(2,7),(4,11),(7,7),(12,-2),(17,0),(21,0)])
   back=keys(f,[(0,0),(1,.005),(3,.035),(5,.052),(8,.043),(12,.02),(17,0),(21,0)])
   lateral=keys(f,[(0,0),(3,.018),(5,.025),(9,.015),(15,0),(21,0)])
   down=keys(f,[(0,0),(2,0),(5,.012),(8,.026),(13,.01),(18,0),(21,0)])
   hp=keys(f,[(0,0),(2,0),(5,-3),(8,-2),(13,1),(18,0),(21,0)])
   hy=keys(f,[(0,0),(2,0),(5,-5),(8,-5),(14,1),(20,0),(21,0)])
   neck=keys(f,[(0,0),(1,6),(3,-8),(5,-13),(8,-3),(11,6),(16,0),(21,0)])
   headturn=keys(f,[(0,0),(2,4),(5,-9),(8,-6),(12,3),(17,0),(21,0)])
  m=wm('Hips');m.translation+=Vector((lateral,back,-down));setwm('Hips',m)
  turn('Hips',(1,0,0),hp);turn('Hips',(0,0,1),hy)
  turn('Spine',(1,0,0),pitch*.42);turn('Chest',(1,0,0),pitch*.58)
  turn('Spine',(0,0,1),yaw*.25);turn('Chest',(0,0,1),yaw*.75)
  turn('Spine',(0,1,0),lean*.4);turn('Chest',(0,1,0),lean*.6)
  turn('Neck',(1,0,0),neck);turn('Neck',(0,0,1),headturn)
  # Arms initially lag at the original world position. They then fly apart
  # with asymmetric arcs; the free hand catches itself before the weapon arm.
  for side in ['Left','Right']:
   sign=1 if side=='Left' else -1
   if large:
    delay=0 if side=='Left' else 1.5
    t=max(0,f-delay)
    spread=keys(t,[(0,0),(2,.035),(5,.13),(9,.22),(13,.19),(19,.08),(28,.025),(36,0),(40,0)])
    lift=keys(t,[(0,0),(2,.035),(5,.15),(9,.23),(13,.24),(20,.13),(29,.02),(36,0),(40,0)])
    depth=keys(t,[(0,0),(2,-.045),(5,-.075),(9,.015),(14,.08),(21,.08),(30,.02),(36,0),(40,0)])
    if side=='Left':
     spread*=.7;lift*=1.10;depth-=keys(f,[(0,0),(10,0),(16,.15),(24,.09),(34,0),(40,0)])
    settle=keys(f,[(0,0),(30,0),(40,1)])
   else:
    t=max(0,f-(1 if side=='Left' else 0))
    spread=keys(t,[(0,0),(2,.018),(5,.075),(8,.08),(13,.025),(19,0),(21,0)])
    lift=keys(t,[(0,0),(2,.025),(5,.085),(8,.10),(13,.035),(19,0),(21,0)])
    depth=keys(t,[(0,0),(2,-.028),(5,-.05),(8,.015),(12,.028),(18,0),(21,0)])
    settle=keys(f,[(0,0),(15,0),(21,1)])
   target=basehands[side].translation+Vector((sign*spread,depth,lift))
   target=target.lerp(wm(side+'Hand').translation,settle)
   if f>0 and f<N:
    arm(side,target,wm(side+'UpperArm').translation+Vector((sign*.4,.07,-.26)))
  for side in ['Left','Right']:
   target=feet[side].copy()
   if large and side=='Right':
    # The already trailing foot catches the backwards fall; the lead stays fixed.
    step=keys(f,[(0,0),(4,0),(11,1),(24,1),(36,0),(40,0)])
    lift=keys(f,[(0,0),(4,0),(7,1),(11,0),(24,0),(29,.8),(36,0),(40,0)])
    target.translation+=Vector((-.055*step,.25*step,.065*lift))
   checks.append(leg(side,target));samples.append({'frame':f,'foot':side,'position':list(wm(side+'Foot').translation),'target_error':(wm(side+'Foot').translation-target.translation).length})
  seq.append(capture())
 sequences[label]=seq;report['clips'][label]={'frames':len(seq),'sample_hz':60,'seconds':N/30,'max_leg_extension':max(checks),'max_target_error':max(v['target_error'] for v in samples),'foot_samples':samples}
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
