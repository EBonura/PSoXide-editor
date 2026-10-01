import bpy,sys,json,math,re,struct,hashlib
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent;project=out.parents[3];sys.path.insert(0,str(out));from decode_clip import decode
repo=project.parents[2];folder=out/'reference'
bpy.ops.wm.open_mainfile(filepath=str(out.parent/'cybernetic_walk/walk_fwd.blend'))
s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');meshes=[o for o in s.objects if o.type=='MESH'];r.animation_data_clear()
for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update();world=r.matrix_world.copy();inv=world.inverted();rest={b.name:world@b.matrix_local for b in r.data.bones}
names=json.loads('['+re.search(r'joint_names: \[([^]]+)\]',(project/'project.ron').read_text())[1]+']');assert len(names)==26
blob=(folder/'mesh.psxmdl').read_bytes();j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16;raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float)
# Recover the affine coordinate normalization from the same character's bind mesh.
dg=bpy.context.evaluated_depsgraph_get();verts=[]
for ob in meshes:
 ev=ob.evaluated_get(dg);me=ev.to_mesh();verts.extend(ev.matrix_world@v.co for v in me.vertices);ev.to_mesh_clear()
verts=np.array(verts);C=Matrix(((1,0,0),(0,0,-1),(0,1,0)));rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=(verts[:,2].max()-verts[:,2].min())/(rv[:,2].max()-rv[:,2].min());center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())
def capture():return {b.name:b.matrix_basis.copy() for b in r.pose.bones}
def apply(pose):
 for n,m in pose.items():r.pose.bones[n].matrix_basis=m
 bpy.context.view_layer.update()
def mix(a,b,t):
 result={}
 for n in names:
  la,qa,sa=a[n].decompose();lb,qb,sb=b[n].decompose();result[n]=Matrix.LocRotScale(la.lerp(lb,t),qa.slerp(qb,t),sa.lerp(sb,t))
 return result
def smooth(t):t=max(0,min(1,t));return t*t*(3-2*t)
clips={};meta={}
for label,index,inplace in [('idle',21,True),('walk',2,False),('start_original',3,True),('stop_original',4,True),('stop_alt_original',5,True)]:
 path=next(folder.glob(f'clip_{index:02}_*'));R,T,n,hz=decode(path);seq=[]
 for f in range(n):
  delta=T[f,0]-T[0,0] if inplace else np.zeros(3);delta[1]=0
  for k,name in enumerate(names):
   rot=C@Matrix(R[f,k].tolist())@C.transposed();skin=rot.to_4x4();skin.translation=center-rot@center+C@Vector(((T[f,k]-delta)*unit).tolist());r.pose.bones[name].matrix=inv@skin@rest[name];bpy.context.view_layer.update()
  seq.append(capture())
 clips[label]=seq;meta[label]={'frames':n,'hz':hz,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
def sample(label,phase):
 seq=clips[label];a=min(int(phase),len(seq)-1);b=min(a+1,len(seq)-1);return mix(seq[a],seq[b],phase-a)
idle=clips['idle'][0];walkA=clips['walk'][0];walkB=sample('walk',len(clips['walk'])/2)
# Neutral -> first walk pose; each stop starts at the exact corresponding walk phase.
# Feet use separate trajectories, instead of inheriting a sliding FK crossfade.
def worldmat(n):return world@r.pose.bones[n].matrix
def setworld(n,m):r.pose.bones[n].matrix=inv@m;bpy.context.view_layer.update()
def aim(n,delta):
 m=worldmat(n);loc,q,scale=m.decompose();setworld(n,Matrix.LocRotScale(loc,delta@q,scale))
def pulse(t,peak):
 if t<=peak:return math.sin(math.pi*.5*t/peak)**2
 return math.cos(math.pi*.5*(t-peak)/(1-peak))**2
stats={};sequences={}
for label,A,B,N,alt in [('walk_fwd_windup',idle,walkA,14,False),('walk_fwd_winddown',walkA,idle,24,False),('walk_fwd_winddown_mirror',walkB,idle,24,True)]:
 apply(A);aw={n:worldmat(n).copy() for n in names};apply(B);bw={n:worldmat(n).copy() for n in names};sequence=[];checks=[];body=[]
 for f in range(N+1):
  t=f/N;blend=smooth(t);apply(mix(A,B,blend))
  if f not in [0,N]:
   start=label=='walk_fwd_windup';direction=-1 if alt else 1
   load=pulse(t,.30 if start else .25);follow=pulse(t,.43 if start else .38)
   h=worldmat('Hips');h.translation.z-=(.060 if start else .075)*load
   h.translation.y-=(.035 if start else .045)*load
   h.translation.x-=direction*.030*math.sin(2*math.pi*t)*math.sin(math.pi*t)
   setworld('Hips',h)
   aim('Hips',Quaternion((1,0,0),math.radians(3 if start else 4)*load))
   aim('Spine',Quaternion((1,0,0),math.radians(6 if start else 8)*follow))
   aim('Chest',Quaternion((0,0,1),math.radians(direction*3)*follow))
   # Gaze follows the body with a smaller delayed nod; arms trail the load.
   aim('Neck',Quaternion((1,0,0),math.radians(-3)*pulse(t,.53)))
   for side,sign in [('Left',1),('Right',-1)]:
    lag=pulse(t,.55);arm=math.radians(sign*direction*9)*lag
    aim(side+'UpperArm',Quaternion((1,0,0),arm))
    aim(side+'LowerArm',Quaternion((1,0,0),math.radians(5)*pulse(t,.63)))
   for side,sgn in [('Left',1),('Right',-1)]:
    n=side+'Foot'
    if label=='walk_fwd_windup':
     u=smooth((t-.16)/.72) if side=='Right' else blend;lift=(.085*math.sin(math.pi*u)**2 if side=='Right' else .008*math.sin(math.pi*t)**2)
    else:
     first=('Right' if alt else 'Left');u=smooth(t/.55) if side==first else smooth((t-.40)/.50);lift=(.055 if side==first else .035)*math.sin(math.pi*u)**2
    target=aw[n].translation.lerp(bw[n].translation,u);target.z+=lift
    upper,lower=side+'UpperLeg',side+'LowerLeg';um=worldmat(upper);lm=worldmat(lower);fm=worldmat(n);h=um.translation;oldk=lm.translation;oldf=fm.translation;L1=(oldk-h).length;L2=(oldf-oldk).length;d=target-h;dist=d.length;reach=dist/(L1+L2)
    assert reach<1,(label,f,side,reach)
    axis=d.normalized();pole=oldk-h;pole=(pole-axis*pole.dot(axis)).normalized();along=(L1*L1-L2*L2+dist*dist)/(2*dist);k=h+axis*along+pole*math.sqrt(max(0,L1*L1-along*along))
    aim(upper,(oldk-h).normalized().rotation_difference((k-h).normalized()));lm=worldmat(lower);fm=worldmat(n);aim(lower,(fm.translation-lm.translation).normalized().rotation_difference((target-lm.translation).normalized()))
    fm=worldmat(n);_,qa,sa=aw[n].decompose();_,qb,sb=bw[n].decompose();setworld(n,Matrix.LocRotScale(fm.translation,qa.slerp(qb,u),sa.lerp(sb,u)))
    checks.append({'frame':f,'side':side,'reach':reach,'target_error':(worldmat(n).translation-target).length})
  body.append({'frame':f,'hips':list(worldmat('Hips').translation),'chest':list(worldmat('Chest').translation),'head':list(worldmat('Neck').translation),'left_hand':list(worldmat('LeftHand').translation),'right_hand':list(worldmat('RightHand').translation)})
  sequence.append(capture())
 sequences[label]=sequence;stats[label]={'frames':N+1,'duration':N/30,'max_reach':max(v['reach'] for v in checks),'max_foot_target_error':max(v['target_error'] for v in checks),'samples':checks,'body_samples':body}
# Save individual editable actions and a full review sequence with both stop variants.
def key_action(label,seq):
 r.animation_data_clear();r.animation_data_create();act=bpy.data.actions.new(label);r.animation_data.action=act;prev={}
 for f,pose in enumerate(seq):
  apply(pose)
  for b in r.pose.bones:
   loc,q,scale=b.matrix_basis.decompose();b.rotation_mode='QUATERNION'
   if b.name in prev and q.dot(prev[b.name])<0:q.negate()
   prev[b.name]=q.copy();b.location=loc;b.rotation_quaternion=q;b.scale=scale
   for prop in ['location','rotation_quaternion','scale']:b.keyframe_insert(prop,frame=f,group=b.name)
 for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
  for k in fc.keyframe_points:k.interpolation='LINEAR'
 s.frame_start=0;s.frame_end=len(seq)-1;s.render.fps=30;s.frame_set(0);return act
bpy.context.preferences.filepaths.save_version=0;bpy.ops.object.select_all(action='DESELECT')
for ob in [r]+meshes:ob.select_set(True)
bpy.context.view_layer.objects.active=r
for name,seq in sequences.items():
 act=key_action(name,seq)
 bpy.ops.export_scene.gltf(filepath=str(out/(name+'.glb')),export_format='GLB',use_selection=True,export_animations=True,export_animation_mode='ACTIVE_ACTIONS',export_force_sampling=True,export_frame_range=True,export_optimize_animation_size=False)
 bpy.ops.wm.save_as_mainfile(filepath=str(out/(name+'.blend')))
 bpy.data.actions.remove(act)
# At 30 fps the game walk advances twice its 60 Hz fixed-point phase step.
walkstep=((23*4096//60)*262//256)*2/4096
for variant,alt in [('A',False),('B',True)]:
 walkframes=round((2 if not alt else 1.5)*34/walkstep);new=[idle]*12+sequences['walk_fwd_windup'][:-1];old=[idle]*12
 orig=clips['start_original'];old += [sample('start_original',min(f/30*meta['start_original']['hz'],len(orig)-1)) for f in range(14)]
 start_old=old[-1];start_new=new[-1]
 for f in range(walkframes):
  phase=(f*walkstep)%34;a=int(phase);b=(a+1)%34;pose=mix(clips['walk'][a],clips['walk'][b],phase-a);new.append(mix(start_new,pose,min(1,f/3)));old.append(mix(start_old,pose,min(1,f/3)))
 stopname='walk_fwd_winddown_mirror' if alt else 'walk_fwd_winddown';oldname='stop_alt_original' if alt else 'stop_original'
 # Emulate the engine's short frozen-outgoing gait-to-stop crossfade.
 previous=new[-1]
 for f in range(36):
  np=sequences[stopname][min(f,len(sequences[stopname])-1)];op=sample(oldname,min(f/30*meta[oldname]['hz'],len(clips[oldname])-1))
  end_tick=(len(clips[oldname])-1)*30/meta[oldname]['hz'];op=mix(op,idle,smooth((f-end_tick)/3))
  new.append(mix(previous,np,min(1,f/3)));old.append(mix(previous,op,min(1,f/3)))
 new += [idle]*12;old += [idle]*12
 for kind,seq in [('new',new),('original',old)]:
  act=key_action(kind+'_'+variant,seq);bpy.ops.wm.save_as_mainfile(filepath=str(out/(kind+'_'+variant+'.blend')));bpy.data.actions.remove(act)
report={'status':'candidate; not installed','fps':30,'transitions':stats,'references':meta,'walk_phase_step_per_preview_frame':walkstep,'review':'Current approved walk between original or new transitions; source reconstruction studio preview, not an engine capture.'};(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT transitions authored',json.dumps({k:{x:y for x,y in v.items() if x not in ['samples','body_samples']} for k,v in stats.items()}))
