"""Aletha knockdown candidates. Writes next to this script.
Shipped: A_hand_down (assets/animations/knockdown_v1/aletha_knockdown.psxanim).
Built the same way as reactions_v3/poise (keyed poses over her idle frame 84,
leg and arm IK, 30 Hz beats exported at 60 Hz samples).
Args after --: [candidate ...] (default all)."""
import bpy,sys,json,math,re,struct
sys.dont_write_bytecode=True
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
OUT=Path(__file__).resolve().parent
P=OUT.parents[2]
sys.path.insert(0,str(P/'source_assets/animations/player/cybernetic_walk_transitions_review_v4'));from decode_clip import decode
bpy.ops.wm.open_mainfile(filepath=str(P/'source_assets/characters/aletha_closed_458/Aletha-closed-458.blend'))
s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');mesh=bpy.data.objects['Aletha optimized'];r.animation_data_clear()
for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update();world=r.matrix_world.copy();inv=world.inverted();names=[b.name for b in r.data.bones];rest={n:world@r.data.bones[n].matrix_local for n in names}
blob=(P/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes();j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12)
off=28+j*4+nm*8+parts*16
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
def vkeys(f,knots):
 """Like keys but with a sharp (linear-out) attack on the first segment: the impact snaps."""
 a,x=knots[0];b,y=knots[1]
 if a<f<=b:t=(f-a)/(b-a);t=1-(1-t)**2;return x+(y-x)*t
 return keys(f,knots)
def arm(side,target,pole):
 a,b,c=[side+t for t in ['UpperArm','LowerArm','Hand']];h=wm(a).translation;k0=wm(b).translation;f0=wm(c).translation
 L1=(k0-h).length;L2=(f0-k0).length;d=target-h;dist=min(d.length,.975*(L1+L2));axis=d.normalized();bend=pole-h;bend=(bend-axis*bend.dot(axis)).normalized()
 along=(L1*L1-L2*L2+dist*dist)/(2*dist);k=h+axis*along+bend*math.sqrt(max(0,L1*L1-along*along))
 aim(a,(k0-h).normalized().rotation_difference((k-h).normalized()));km=wm(b).translation;fm=wm(c).translation;aim(b,(fm-km).normalized().rotation_difference((h+axis*dist-km).normalized()))
 return d.length/(L1+L2)
def leg(side,target):
 a,b,c=[side+t for t in ['UpperLeg','LowerLeg','Foot']];h=wm(a).translation;k0=wm(b).translation;f0=wm(c).translation
 L1=(k0-h).length;L2=(f0-k0).length;d=target.translation-h;dist=d.length;axis=d.normalized();reach=dist/(L1+L2)
 assert reach<.9999,(side,reach)
 pole=Vector((0,-1,0));pole=(pole-axis*pole.dot(axis)).normalized();along=(L1*L1-L2*L2+dist*dist)/(2*dist);k=h+axis*along+pole*math.sqrt(max(0,L1*L1-along*along))
 aim(a,(k0-h).normalized().rotation_difference((k-h).normalized()));km=wm(b).translation;fm=wm(c).translation;aim(b,(fm-km).normalized().rotation_difference((target.translation-km).normalized()));setwm(c,target)
 return reach
R,T,n,hz=decode(path(45));apply_source(R[min(84,n-1)],T[min(84,n-1)]);base=capture();feet={side:wm(side+'Foot') for side in ['Left','Right']}
basehands={side:wm(side+'Hand').copy() for side in ['Left','Right']}
apply(base);dg=bpy.context.evaluated_depsgraph_get();ev=mesh.evaluated_get(dg);me=ev.to_mesh()
group_points={}
for gname in ['LeftFoot','RightFoot','LeftHand','RightHand','LeftLowerLeg','RightLowerLeg']:
 vg=mesh.vertex_groups.get(gname).index
 group_points[gname]=[v.index for v in mesh.data.vertices if any(g.group==vg and g.weight>.5 for g in v.groups)]
toes={}
for side in ['Left','Right']:
 pts=[ev.matrix_world@me.vertices[i].co for i in group_points[side+'Foot']];mn=min(v.z for v in pts);low=[v for v in pts if v.z<mn+.025];toes[side]=(sum(low,Vector())/len(low),pts,mn)
ev.to_mesh_clear()
FLOOR=0.0
# ---------------------------------------------------------------------------
# Candidates. Every channel is a function of f (30 Hz beat time). Each returns
# a dict: body channels, feet {side:(dx,dy,lift,toe_deg)}, hands {side:target or None}.
# Facing -Y: +Y is backward, +X is her left. Positive pitch folds forward.
def breathe(f,a,b,amp):
 if a<f<b:
  w=math.sin(math.pi*(f-a)/(b-a));return amp*w*math.sin(2*math.pi*(f-a)/9.)
 return 0.
def cand_A(f):
 """Hand to the floor: bigger recoil, stagger step, right knee drops and the free
 (left) hand slaps the floor in front, head hangs, laboured push up."""
 N=78
 c={}
 c['pitch']=vkeys(f,[(0,0),(2,-46),(5,-52),(9,-30),(14,10),(20,58),(24,68),(30,64),(40,63),(46,58),(52,34),(58,16),(64,8),(70,-2),(74,1),(78,0)])+breathe(f,24,46,1.5)
 c['yaw']=vkeys(f,[(0,0),(2,26),(5,46),(9,44),(15,24),(22,10),(40,8),(50,4),(62,-4),(70,1),(78,0)])
 c['lean']=keys(f,[(0,0),(2,-14),(6,-22),(12,-14),(20,10),(28,16),(42,15),(52,4),(64,-2),(78,0)])
 c['back']=vkeys(f,[(0,0),(2,.04),(5,.15),(9,.27),(14,.26),(20,.16),(26,.10),(44,.10),(54,.11),(62,.08),(70,.01),(78,0)])
 c['lateral']=keys(f,[(0,0),(5,-.04),(10,-.08),(20,-.02),(26,.03),(44,.03),(56,-.01),(66,0),(78,0)])
 c['down']=keys(f,[(0,0),(3,.01),(7,.05),(11,.19),(16,.44),(20,.56),(23,.575),(28,.565),(44,.56),(50,.47),(56,.33),(62,.15),(68,.04),(72,-.008),(75,.004),(78,0)])+breathe(f,24,46,.008)
 c['hp']=keys(f,[(0,0),(3,-4),(7,-10),(12,-2),(20,20),(26,26),(44,25),(52,12),(62,4),(78,0)])
 c['hy']=keys(f,[(0,0),(3,4),(8,15),(16,14),(26,8),(44,7),(58,2),(78,0)])
 c['neck']=keys(f,[(0,0),(1,10),(4,-24),(7,-26),(11,-6),(18,22),(24,34),(32,36),(42,32),(48,6),(54,-10),(62,-6),(70,2),(78,0)])
 c['headturn']=keys(f,[(0,0),(2,-8),(6,20),(10,24),(18,8),(30,-6),(42,-4),(50,4),(62,-3),(78,0)])
 feetd={
  'Right':(keys(f,[(0,0),(4,0),(11,-.05),(60,-.05),(70,0),(78,0)]),
           keys(f,[(0,0),(4,0),(11,.38),(58,.38),(70,0),(78,0)]),
           keys(f,[(0,0),(4,0),(7.5,1),(11,0),(58,0),(64,1),(70,0),(78,0)])*.075,
           keys(f,[(0,0),(10,0),(19,66),(52,66),(58,30),(64,0),(78,0)])),
  'Left':(0,0,0,0)}
 arms={}
 slap=keys(f,[(0,0),(13,0),(20,1),(46,1),(52,.0),(78,0)])
 brace=keys(f,[(0,0),(48,0),(53,1),(60,1),(68,0),(78,0)])
 flail_l=keys(f,[(0,0),(2,.05),(6,.24),(10,.30),(15,.12),(22,0),(78,0)])
 arms['Left']=dict(flail=flail_l,floor=(Vector((.30,-.34,.105)),slap),brace=(Vector((.14,-.235,.49)),brace))
 flail_r=keys(max(0,f-1.5),[(0,0),(2,.05),(6,.24),(10,.30),(15,.12),(24,0),(78,0)])
 hang=keys(f,[(0,0),(12,0),(20,1),(50,1),(62,.4),(70,0),(78,0)])
 arms['Right']=dict(flail=flail_r,hang=(Vector((-.30,-.18,.20)),hang))
 return N,c,feetd,arms
def cand_B(f):
 """Doubled over: hit drives straight into the gut. She folds forward, two short
 stumbling steps back, knees sag into a deep crouch with one hand on the knee
 and the other forearm across the stomach; a failed first try to rise."""
 N=72
 c={}
 c['pitch']=vkeys(f,[(0,0),(2,34),(5,52),(9,48),(14,56),(20,62),(26,60),(38,58),(44,46),(48,50),(54,34),(60,16),(66,4),(70,-1),(72,0)])+breathe(f,20,40,3)
 c['yaw']=vkeys(f,[(0,0),(2,-10),(5,-16),(10,-8),(18,-6),(36,-6),(50,-3),(62,2),(72,0)])
 c['lean']=keys(f,[(0,0),(3,6),(8,-6),(13,5),(20,2),(40,3),(56,-1),(72,0)])
 c['back']=vkeys(f,[(0,0),(2,.07),(5,.16),(9,.22),(13,.29),(18,.30),(40,.30),(50,.27),(58,.18),(64,.06),(72,0)])
 c['lateral']=keys(f,[(0,0),(6,.03),(11,-.03),(16,.0),(40,0),(72,0)])
 c['down']=keys(f,[(0,0),(2,.04),(5,.09),(9,.13),(13,.22),(18,.31),(22,.33),(38,.32),(43,.24),(47,.27),(52,.21),(58,.11),(64,.03),(68,-.006),(72,0)])+breathe(f,20,40,.006)
 c['hp']=keys(f,[(0,0),(3,12),(8,18),(14,24),(20,28),(40,27),(48,22),(56,12),(64,4),(72,0)])
 c['hy']=keys(f,[(0,0),(3,-6),(9,-4),(20,-3),(56,0),(72,0)])
 c['neck']=keys(f,[(0,0),(1,-8),(3,-12),(6,10),(10,20),(18,26),(36,28),(42,6),(46,10),(52,-8),(60,-6),(68,1),(72,0)])
 c['headturn']=keys(f,[(0,0),(3,6),(8,-6),(20,-4),(40,-3),(56,3),(72,0)])
 feetd={
  'Right':(keys(f,[(0,0),(72,0)]),
           keys(f,[(0,0),(3,0),(8,.24),(56,.24),(64,0),(72,0)]),
           keys(f,[(0,0),(3,0),(5.5,1),(8,0),(56,0),(60,1),(64,0),(72,0)])*.07,
           keys(f,[(0,0),(3,0),(8,8),(20,10),(52,10),(60,0),(72,0)])),
  'Left':(keys(f,[(0,0),(72,0)]),
          keys(f,[(0,0),(8,0),(13,.17),(60,.17),(67,0),(72,0)]),
          keys(f,[(0,0),(8,0),(10.5,1),(13,0),(60,0),(63.5,1),(67,0),(72,0)])*.06,0)}
 arms={}
 belly=keys(f,[(0,0),(2,.6),(5,1),(46,1),(58,.3),(66,0),(72,0)])
 arms['Right']=dict(belly=(Vector((.02,-.14,.86)),belly))
 knee=keys(f,[(0,0),(9,0),(16,1),(52,1),(60,.3),(66,0),(72,0)])
 flail_l=keys(f,[(0,0),(3,.10),(7,.20),(11,.05),(72,0)])
 arms['Left']=dict(flail=flail_l,kneehand=(Vector((.16,-.17,.52)),knee))
 return N,c,feetd,arms
def cand_C(f):
 """Knees give out: a heavy hit, one stagger step back, then the second leg
 folds too and she drops onto BOTH knees, slumped, arms hanging. She gets the
 left foot back under her, pushes off that thigh and stands."""
 N=84
 c={}
 c['pitch']=vkeys(f,[(0,0),(2,-44),(5,-50),(9,-26),(15,4),(22,20),(28,26),(46,24),(52,32),(58,30),(64,18),(70,6),(76,-2),(80,1),(84,0)])+breathe(f,26,48,2.5)
 c['yaw']=vkeys(f,[(0,0),(2,22),(5,40),(9,38),(15,18),(24,4),(46,3),(60,6),(72,-2),(84,0)])
 c['lean']=keys(f,[(0,0),(2,-12),(6,-20),(12,-10),(20,-4),(30,-6),(46,-5),(56,4),(70,-1),(84,0)])
 c['back']=vkeys(f,[(0,0),(2,.04),(5,.15),(9,.25),(14,.27),(20,.27),(24,.26),(46,.26),(52,.20),(58,.16),(66,.08),(76,.01),(84,0)])
 c['lateral']=keys(f,[(0,0),(5,-.04),(10,-.07),(18,-.02),(24,0),(42,0),(47,-.05),(53,.02),(64,.02),(74,0),(84,0)])
 c['down']=keys(f,[(0,0),(3,.01),(7,.05),(11,.17),(15,.36),(18,.48),(21,.575),(23,.565),(42,.56),(47,.45),(55,.44),(59,.42),(63,.30),(68,.14),(73,.03),(77,-.008),(80,.003),(84,0)])+breathe(f,26,48,.008)
 c['hp']=keys(f,[(0,0),(3,-4),(7,-10),(12,-4),(20,-6),(46,-6),(52,6),(60,10),(68,4),(84,0)])
 c['hy']=keys(f,[(0,0),(3,4),(8,14),(16,10),(24,2),(46,1),(56,6),(70,1),(84,0)])
 c['neck']=keys(f,[(0,0),(1,10),(4,-22),(7,-24),(11,-4),(18,16),(23,32),(30,38),(44,36),(50,10),(56,-8),(64,-4),(74,2),(84,0)])
 c['headturn']=keys(f,[(0,0),(2,-8),(6,18),(10,22),(18,6),(28,-10),(44,-8),(52,2),(64,-3),(84,0)])
 feetd={
  # Right: stagger step back, knee down, stays until the very end.
  'Right':(keys(f,[(0,0),(4,0),(11,-.04),(66,-.04),(76,0),(84,0)]),
           keys(f,[(0,0),(4,0),(11,.40),(66,.40),(76,0),(84,0)]),
           keys(f,[(0,0),(4,0),(7.5,1),(11,0),(66,0),(71,1),(76,0),(84,0)])*.075,
           keys(f,[(0,0),(10,0),(19,66),(60,66),(66,30),(72,0),(84,0)])),
  # Left: slides back as the second knee folds, then is brought forward and planted to stand.
  'Left':(keys(f,[(0,0),(13,0),(21,.03),(47,.03),(55,0),(84,0)]),
          keys(f,[(0,0),(13,0),(21,.64),(47,.64),(55,0),(84,0)]),
          keys(f,[(0,0),(13,0),(17,.05),(21,0),(47,0),(51,.07),(55,0),(84,0)]),
          keys(f,[(0,0),(13,0),(21,66),(44,66),(47,52),(51,15),(55,0),(84,0)]))}
 arms={}
 flail_l=keys(f,[(0,0),(2,.05),(6,.24),(10,.30),(15,.12),(22,0),(84,0)])
 hang_l=keys(f,[(0,0),(14,0),(22,1),(46,1),(52,0),(84,0)])
 brace=keys(f,[(0,0),(48,0),(54,1),(64,1),(72,0),(84,0)])
 arms['Left']=dict(flail=flail_l,hang=(Vector((.31,-.14,.20)),hang_l),brace=(Vector((.15,-.27,.50)),brace))
 flail_r=keys(max(0,f-1.5),[(0,0),(2,.05),(6,.24),(10,.30),(15,.12),(24,0),(84,0)])
 hang_r=keys(f,[(0,0),(14,0),(22,1),(56,1),(68,.3),(76,0),(84,0)])
 arms['Right']=dict(flail=flail_r,hang=(Vector((-.31,-.14,.20)),hang_r))
 return N,c,feetd,arms
CANDS={'A_hand_down':cand_A,'B_doubled_over':cand_B,'C_both_knees':cand_C}
want=sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else []
want=[k for k in CANDS if not want or any(k.startswith(w) for w in want)]
report={}
def lowest(gnames):
 dg=bpy.context.evaluated_depsgraph_get();ev=mesh.evaluated_get(dg);me=ev.to_mesh();out={}
 for g in gnames:out[g]=min((ev.matrix_world@me.vertices[i].co).z for i in group_points[g])
 allz=min((ev.matrix_world@v.co).z for v in me.vertices);ev.to_mesh_clear();return out,allz
for label in want:
 fn=CANDS[label];N=fn(0)[0];seq=[];legrep=[];armrep=[];contact=[];footpos=[]
 for sample in range(2*N+1):
  f=sample/2;apply(base);_,c,feetd,arms=fn(f)
  m=wm('Hips');m.translation+=Vector((c['lateral'],c['back'],-c['down']));setwm('Hips',m)
  turn('Hips',(1,0,0),c['hp']);turn('Hips',(0,0,1),c['hy'])
  turn('Spine',(1,0,0),c['pitch']*.42);turn('Chest',(1,0,0),c['pitch']*.58)
  turn('Spine',(0,0,1),c['yaw']*.25);turn('Chest',(0,0,1),c['yaw']*.75)
  turn('Spine',(0,1,0),c['lean']*.4);turn('Chest',(0,1,0),c['lean']*.6)
  turn('Neck',(1,0,0),c['neck']);turn('Neck',(0,0,1),c['headturn'])
  for side in ['Left','Right']:
   sign=1 if side=='Left' else -1;spec=arms[side]
   fl=spec.get('flail',0)
   target=basehands[side].translation+Vector((sign*.27*fl,-.10*fl,.30*fl))
   # Hands follow the torso by default (the FK result after the body turns),
   # mixed toward the base hand by the flail amount.
   fk=wm(side+'Hand').translation.copy()
   target=fk.lerp(target,min(1,fl*3))
   for key in ['hang','belly','kneehand','floor','brace']:
    if key in spec:
     pt,w=spec[key];target=target.lerp(pt,w)
   if 0<f<N:armrep.append(arm(side,target,wm(side+'UpperArm').translation+Vector((sign*.4,.04,-.26))))
  for side in ['Left','Right']:
   dx,dy,lift,ang=feetd[side];toe,points,minimum=toes[side];target=feet[side].copy()
   q=Quaternion((1,0,0),math.radians(ang)).to_matrix().to_4x4()
   rotated=Matrix.Translation(toe)@q@Matrix.Translation(-toe)
   target=rotated@target
   target.translation.z+=minimum-min((rotated@v).z for v in points)
   target.translation+=Vector((dx,dy,lift))
   legrep.append(leg(side,target))
  lo,allz=lowest(['LeftFoot','RightFoot','LeftHand','RightHand','LeftLowerLeg','RightLowerLeg']);contact.append({'f':f,**{k:round(v,4) for k,v in lo.items()},'all':round(allz,4)})
  seq.append(capture())
 report[label]={'beats_30hz':N,'samples_60hz':len(seq),'seconds':N/30,'max_leg_reach':max(legrep),'max_arm_reach':max(armrep),
  'min_vertex_z':min(x['all'] for x in contact),'min_hand_z':min(min(x['LeftHand'],x['RightHand']) for x in contact),
  'min_knee_z':min(min(x['LeftLowerLeg'],x['RightLowerLeg']) for x in contact)}
 (OUT/f'{label}_contacts.json').write_text(json.dumps(contact))
 # Save editable rig and the cooked candidate.
 for ob in s.objects:
  if ob.type=='MESH':ob.hide_render=ob!=mesh;ob.hide_set(ob!=mesh)
 r.animation_data_clear();prev={};data=bytearray(struct.pack('<4sHHI4H',b'PANM',1,0,0,j,len(seq)+1,60,0))
 for fi,pose in enumerate(seq+[seq[-1]]):
  apply(pose)
  for name in names:
   b=r.pose.bones[name];loc,q,sc=b.matrix_basis.decompose();b.rotation_mode='QUATERNION'
   if name in prev and prev[name].dot(q)<0:q.negate()
   prev[name]=q.copy();b.location=loc;b.rotation_quaternion=q;b.scale=sc
   for prop in ['location','rotation_quaternion','scale']:b.keyframe_insert(prop,frame=fi,group=name)
   skin=wm(name)@rest[name].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C;tt=C.transposed()@(skin.translation-center+rot@center)/unit
   data.extend(struct.pack('<9h3i',*[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)],*[round(v) for v in tt]))
 act=r.animation_data.action;act.name='Aletha_knockdown_'+label;act.use_fake_user=True
 for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
  for k in fc.keyframe_points:k.interpolation='LINEAR'
 s.frame_start=0;s.frame_end=len(seq)-1;s.render.fps=60;s.frame_set(0)
 bpy.context.preferences.filepaths.save_version=0
 bpy.ops.wm.save_as_mainfile(filepath=str(OUT/(label+'.blend')),copy=True)
 data[:4]=path(87).read_bytes()[:4];struct.pack_into('<I',data,8,len(data)-12);(OUT/(label+'.psxanim')).write_bytes(data)
 print('RESULT',label,json.dumps(report[label]),flush=True)
(OUT/'author_report.json').write_text(json.dumps(report,indent=2))
