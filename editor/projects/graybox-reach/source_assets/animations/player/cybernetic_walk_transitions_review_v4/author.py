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
walkstep=((23*4096//60)*262//256)*2/4096
# The cooked walk includes a duplicate final sample. Do not insert that hold
# into the pre-roll; approach frame zero along the final moving interval.
def gait(phase,preroll=False):
 period=len(clips['walk'])-1 if preroll else len(clips['walk']);phase%=period;a=int(phase);b=(a+1)%period
 return mix(clips['walk'][a],clips['walk'][b],phase-a)
def step_integral(f,ramp):
 # Integral of smoothstep speed from rest, then constant gait speed.
 if f>=ramp:return f-ramp/2
 t=f/ramp;return ramp*(t**3-.5*t**4)
stats={};sequences={}
for label,N,entry in [('walk_fwd_windup',20,0),('walk_fwd_winddown',28,0),('walk_fwd_winddown_mirror',28,17)]:
 start=label=='walk_fwd_windup';seq=[];checks=[];body=[]
 apply(idle);idlefeet={side:worldmat(side+'Foot').copy() for side in ['Left','Right']}
 apply(gait(entry));entryfeet={side:worldmat(side+'Foot').copy() for side in ['Left','Right']}
 apply(gait(entry+walkstep));velocity={side:worldmat(side+'Foot').translation-entryfeet[side].translation for side in ['Left','Right']}
 first='Left' if entry==0 else 'Right'
 for f in range(N+1):
  t=f/N
  if start:
   phase=(step_integral(f,10)-step_integral(N,10))*walkstep
   moving=gait(phase,preroll=True);weight=smooth(f/12)
   pose=mix(idle,moving,weight)
   # Finish loading before handoff. The last five frames are pure moving gait.
   load=math.sin(math.pi*min(f/15,1))**2 if f<15 else 0
  else:
   # Decelerate the currently moving stride; never reset to another step.
   phase=entry+walkstep*(f-f*f/(2*N))
   moving=gait(phase);weight=smooth(t**1.4)
   pose=mix(moving,idle,weight)
   # One compression after the arriving foot, fading naturally to idle.
   load=.8*pulse(t,.26)+.25*pulse(max(0,(t-.32)/.68),.48)
  apply(pose)
  targets={side:worldmat(side+'Foot').copy() for side in ['Left','Right']}
  if not start:
   # First foot finishes the braking step; the other then closes the stance.
   # Carry the incoming velocity through the first contact instead of a hold.
   for side in ['Left','Right']:
    a=entryfeet[side];b=idlefeet[side]
    if side==first:
     u=max(0,min(1,f/14));u2=u*u;u3=u2*u
     target=a.translation*(2*u3-3*u2+1)+velocity[side]*14*(u3-2*u2+u)+b.translation*(-2*u3+3*u2)
     target.z+=.055*math.sin(math.pi*u)**2
     blend=smooth(u)
    else:
     drift=velocity[side]*(min(f,6)-min(f,6)**2/12)
     u=max(0,min(1,(f-12)/14));blend=smooth(u)
     target=(a.translation+drift).lerp(b.translation,blend)
     target.z+=.045*math.sin(math.pi*u)**2
    _,qa,sa=a.decompose();_,qb,sb=b.decompose()
    targets[side]=Matrix.LocRotScale(target,qa.slerp(qb,blend),sa.lerp(sb,blend))
  if load>0.000001 or not start:
   h=worldmat('Hips');h.translation.z-=(.045 if start else .045)*load;h.translation.y-=.020*load
   if not start:h.translation.x+=(1 if entry==0 else -1)*.018*math.sin(2*math.pi*t)*math.sin(math.pi*t)
   setworld('Hips',h)
   aim('Spine',Quaternion((1,0,0),math.radians(4 if start else 5)*load))
   aim('Neck',Quaternion((1,0,0),math.radians(-1)*load))
   for side in ['Left','Right']:
    upper,lower,foot=side+'UpperLeg',side+'LowerLeg',side+'Foot';um=worldmat(upper);lm=worldmat(lower);fm=worldmat(foot);h=um.translation;k0=lm.translation;f0=fm.translation;target=targets[side].translation;L1=(k0-h).length;L2=(f0-k0).length;axis=(target-h).normalized();dist=(target-h).length;reach=dist/(L1+L2);assert reach<1,(label,f,side,reach)
    pole=k0-h;pole=(pole-axis*pole.dot(axis)).normalized();along=(L1*L1-L2*L2+dist*dist)/(2*dist);k=h+axis*along+pole*math.sqrt(max(0,L1*L1-along*along));aim(upper,(k0-h).normalized().rotation_difference((k-h).normalized()));lm=worldmat(lower);fm=worldmat(foot);aim(lower,(fm.translation-lm.translation).normalized().rotation_difference((target-lm.translation).normalized()));m=targets[side].copy();m.translation=worldmat(foot).translation;setworld(foot,m);checks.append({'frame':f,'side':side,'reach':reach,'target_error':(worldmat(foot).translation-target).length})
  seq.append(capture());body.append({'frame':f,'gait_phase':phase,'blend_weight':weight,'hips':list(worldmat('Hips').translation),'left_foot':list(worldmat('LeftFoot').translation),'right_foot':list(worldmat('RightFoot').translation)})
 sequences[label]=seq;stats[label]={'frames':N+1,'duration':N/30,'max_reach':max(v['reach'] for v in checks),'max_foot_target_error':max(v['target_error'] for v in checks),'samples':checks,'body_samples':body}
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
# Source continuity preview: direct matched start join; stop enters from the
# exact walk phase reached on that release. Runtime crossfade tuning is pending.
preview_info={}
for variant,alt in [('A',False),('B',True)]:
 walkframes=round((2 if not alt else 1.5)*34/walkstep);new=[idle]*12+sequences['walk_fwd_windup'][:-1]
 for f in range(walkframes):new.append(gait(f*walkstep))
 name='walk_fwd_winddown_mirror' if alt else 'walk_fwd_winddown'
 previous=new[-1];sequence=sequences[name]
 # The nearest-stop selection leaves less than one game sample of phase
 # difference in this review. Blend that small mismatch over two frames.
 for f in range(36):
  pose=sequence[min(f,len(sequence)-1)];new.append(mix(previous,pose,min(1,f/2)))
 new += [idle]*12
 act=key_action('new_'+variant,new);bpy.ops.wm.save_as_mainfile(filepath=str(out/('new_'+variant+'.blend')));bpy.data.actions.remove(act);preview_info[variant]={'frames':len(new),'walk_start':32,'stop_start':32+walkframes}
report={'status':'candidate; not installed','fps':30,'transitions':stats,'references':meta,'walk_phase_step_per_preview_frame':walkstep,'preview_sequences':preview_info,'review':'Source continuity preview using a matched, unblended start handoff. Runtime start-to-walk crossfade still needs validating and may need adjustment when baking.'};(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT continuous transitions authored',json.dumps({k:{x:y for x,y in v.items() if x not in ['samples','body_samples']} for k,v in stats.items()}))
