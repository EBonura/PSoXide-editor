"""Aletha reaction studies. Review candidates only: does not change project bindings."""
import bpy,sys,json,math,re,struct
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
O=Path(__file__).resolve().parent;P=O.parents[3];REPO=P.parents[2];V=P/'validation/player-reactions-v3';B=REPO/'build/graybox-reach/player-reactions-v3'
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
beats={'poise':{'fps':30,'end':62,'beats':{'strike':1,'recoil':5,'legs_fail':10,'knee_contact':19,'stunned':27,'push_up':38,'stand':49,'ready':62}},'hit':{'source':'reactions_v2/poise','unchanged':True,'seconds':40/30}}
(O/'beats.json').write_text(json.dumps(beats,indent=2))
sequences={};report={'model':{'vertices':nv,'triangles':nf,'joints':j},'candidate_only':True,'clips':{}}
# Toe pivot from the actual right-foot mesh, in the initial idle pose.
apply(base);dg=bpy.context.evaluated_depsgraph_get();ev=mesh.evaluated_get(dg);me=ev.to_mesh()
vg=mesh.vertex_groups.get('RightFoot').index
ids=[v.index for v in mesh.data.vertices if any(g.group==vg and g.weight>.5 for g in v.groups)]
points=[ev.matrix_world@me.vertices[i].co for i in ids];ev.to_mesh_clear()
minimum=min(v.z for v in points);low=[v for v in points if v.z<minimum+.025]
toe=sum(low,Vector())/len(low)
for label,N in [('poise',62)]:
 seq=[];checks=[];samples=[];knees=[]
 for sample in range(2*N+1):
  f=sample/2;apply(base)
  # Same direction as the chosen hit, but the catch fails: she sinks to a knee.
  pitch=keys(f,[(0,0),(1,-24),(3,-40),(5,-43),(8,-29),(12,-3),(18,24),(22,29),(28,26),(35,20),(43,13),(51,3),(60,0),(62,0)])
  yaw=keys(f,[(0,0),(1,18),(4,38),(7,42),(12,27),(20,12),(28,9),(37,4),(49,-3),(60,0),(62,0)])
  lean=keys(f,[(0,0),(2,-13),(6,-20),(11,-14),(19,-9),(28,-6),(39,3),(50,2),(60,0),(62,0)])
  back=keys(f,[(0,0),(1,.014),(4,.10),(8,.21),(13,.20),(19,.115),(28,.11),(38,.11),(47,.07),(56,0),(62,0)])
  lateral=keys(f,[(0,0),(4,-.035),(9,-.075),(19,-.07),(28,-.065),(38,-.025),(49,0),(62,0)])
  down=keys(f,[(0,0),(3,.006),(7,.045),(11,.18),(16,.38),(19,.48),(22,.475),(28,.46),(34,.455),(43,.27),(51,.07),(57,0),(62,0)])
  hp=keys(f,[(0,0),(2,0),(6,-9),(11,-3),(19,10),(29,8),(40,6),(51,0),(62,0)])
  hy=keys(f,[(0,0),(2,0),(7,13),(14,16),(24,7),(36,4),(49,0),(62,0)])
  neck=keys(f,[(0,0),(1,10),(4,-20),(7,-23),(11,-7),(18,14),(24,19),(30,16),(37,1),(44,-8),(54,0),(62,0)])
  headturn=keys(f,[(0,0),(2,-7),(6,18),(10,22),(18,7),(28,4),(36,-7),(48,0),(62,0)])
  m=wm('Hips');m.translation+=Vector((lateral,back,-down));setwm('Hips',m)
  turn('Hips',(1,0,0),hp);turn('Hips',(0,0,1),hy)
  turn('Spine',(1,0,0),pitch*.42);turn('Chest',(1,0,0),pitch*.58)
  turn('Spine',(0,0,1),yaw*.25);turn('Chest',(0,0,1),yaw*.75)
  turn('Spine',(0,1,0),lean*.4);turn('Chest',(0,1,0),lean*.6)
  turn('Neck',(1,0,0),neck);turn('Neck',(0,0,1),headturn)
  # Disordered arm recoil gives way to a dropped weapon arm and the free
  # hand bracing the forward thigh. Recovery starts with the head, then hips.
  for side in ['Left','Right']:
   sign=1 if side=='Left' else -1;t=max(0,f-(0 if side=='Left' else 1.5))
   spread=keys(t,[(0,0),(2,.04),(6,.18),(10,.27),(15,.20),(22,.05),(35,.035),(47,.015),(57,0),(62,0)])
   lift=keys(t,[(0,0),(2,.05),(6,.22),(10,.29),(15,.13),(21,-.23),(29,-.28),(36,-.26),(44,-.15),(54,0),(62,0)])
   depth=keys(t,[(0,0),(2,-.055),(6,-.09),(10,.02),(15,.06),(22,-.19),(30,-.22),(38,-.16),(48,-.07),(58,0),(62,0)])
   target=basehands[side].translation+Vector((sign*spread,depth,lift))
   brace=keys(f,[(0,0),(12,0),(20,1),(32,1),(40,.7),(49,0),(62,0)])
   if side=='Left':target=target.lerp(Vector((.14,-.235,.49)),brace)
   else:target=target.lerp(Vector((-.30,-.12,.24)),brace)
   settle=keys(f,[(0,0),(49,0),(60,1),(62,1)]);target=target.lerp(wm(side+'Hand').translation,settle)
   if f>0 and f<N:arm(side,target,wm(side+'UpperArm').translation+Vector((sign*.4,.04,-.26)))
  for side in ['Left','Right']:
   target=feet[side].copy()
   if side=='Right':
    step=keys(f,[(0,0),(4,0),(12,1),(43,1),(57,0),(62,0)])
    lift=keys(f,[(0,0),(4,0),(8,1),(12,0),(43,0),(48,.9),(57,0),(62,0)])
    angle=keys(f,[(0,0),(10,0),(19,56),(33,56),(43,30),(49,0),(62,0)])
    q=Quaternion((1,0,0),math.radians(angle)).to_matrix().to_4x4()
    rotated=Matrix.Translation(toe)@q@Matrix.Translation(-toe)
    target=rotated@target
    target.translation.z+=minimum-min((rotated@v).z for v in points)
    target.translation+=Vector((-.04*step,.33*step,.065*lift))
   checks.append(leg(side,target));samples.append({'frame':f,'foot':side,'position':list(wm(side+'Foot').translation),'target_error':(wm(side+'Foot').translation-target.translation).length})
  knees.append({'frame':f,'right':list(wm('RightLowerLeg').translation),'left':list(wm('LeftLowerLeg').translation)})
  seq.append(capture())
 sequences[label]=seq;report['clips'][label]={'frames':len(seq),'sample_hz':60,'seconds':N/30,'max_leg_extension':max(checks),'max_target_error':max(v['target_error'] for v in samples),'foot_samples':samples,'knees':knees}
# Keep the chosen hit animation exactly as reviewed; do not re-author it.
import shutil
for ext in ['blend','psxanim']:shutil.copyfile(O.parent/'reactions_v2'/('poise.'+ext),O/('hit.'+ext))
previous=json.loads((P/'validation/player-reactions-v2/audit.json').read_text())
report['clips']['hit']=dict(previous['clips']['poise'],source='reactions_v2/poise',unchanged=True)
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
(V/'audit.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({k:{x:y for x,y in v.items() if x not in ('foot_samples','knees')} for k,v in report['clips'].items()}),flush=True)
