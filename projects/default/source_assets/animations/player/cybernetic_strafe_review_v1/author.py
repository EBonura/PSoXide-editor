import bpy,math,json,sys
from pathlib import Path
from mathutils import Vector,Quaternion,Matrix
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent;project=out.parents[3]
direction=-1 if '--right' in sys.argv else 1;stem='walk_lft' if direction==1 else 'walk_rgt'
bpy.ops.wm.open_mainfile(filepath=str(out.parent/'cybernetic_walk/walk_fwd.blend'))
s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');s.frame_set(0)
base={b.name:b.matrix_basis.copy() for b in r.pose.bones};r.animation_data_clear();world=r.matrix_world.copy();inv=world.inverted();rest={b.name:world@b.matrix_local for b in r.data.bones}
for b in r.pose.bones:b.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update();meshes=[o for o in s.objects if o.type=='MESH'];sole={}
for side in ['Left','Right']:
 offsets=[]
 for o in meshes:
  group=o.vertex_groups.get(side+'Foot')
  if not group:continue
  for v in o.data.vertices:
   if any(g.group==group.index and g.weight>.85 for g in v.groups):offsets.append(o.matrix_world@v.co-rest[side+'Foot'].translation)
 sole[side]=offsets
 assert offsets

def setw(n,q,p=None):
 b=r.pose.bones[n];m=q.to_matrix().to_4x4();m.translation=p if p is not None else (world@b.matrix).translation;b.matrix=inv@m;bpy.context.view_layer.update()
def smooth(x):return x*x*(3-2*x)
def hermite(a,b,ma,mb,t):return (2*t**3-3*t*t+1)*a+(t**3-2*t*t+t)*ma+(-2*t**3+3*t*t)*b+(t**3-t*t)*mb
N=42;stance=.60;stride=.28;speed=stride/(stance*N/30);poses=[];measure=[]
for f in range(N):
 for b in r.pose.bones:b.matrix_basis=base[b.name] if ('Hand' in b.name or 'Thumb' in b.name or 'Index' in b.name) else Matrix.Identity(4)
 bpy.context.view_layer.update();t=f/N;phase=2*math.pi*t
 # Move the pelvis onto the supporting leg, compress after contact, rise at passing.
 hip=Vector((direction*.027*math.cos(phase-1.9),-.018+.008*math.sin(2*phase-.4),.888-.023*math.cos(2*phase-1.25)))
 hipq=Quaternion((0,1,0),math.radians(direction*(2.5+2*math.sin(phase-.3))))@Quaternion((0,0,1),math.radians(direction*4)*math.cos(phase))@Quaternion((1,0,0),math.radians(3+2*math.sin(2*phase-.4)));setw('Hips',hipq@rest['Hips'].to_quaternion(),hip)
 for n,fac,delay in [('Spine',.65,0),('Chest',1.0,.12),('Neck',.24,.25),('LeftShoulder',1.0,.17),('RightShoulder',1.0,.17)]:
  bodyphase=phase-delay
  q=Quaternion((0,0,1),math.radians(-direction*4)*fac*math.cos(bodyphase-.2))@Quaternion((0,1,0),math.radians(direction*(-1+2.5*math.sin(bodyphase-.45)))*fac)@Quaternion((1,0,0),math.radians(5+2.5*math.sin(2*bodyphase-.7))*fac);setw(n,q@rest[n].to_quaternion())
 stats={'frame':f,'feet':{}}
 for side,sign,offset in [('Left',1,0 if direction==1 else .5),('Right',-1,.5 if direction==1 else 0)]:
  p=(t+offset)%1
  if p<stance:
   y=stride/2-stride*p/stance;lift=0;roll=math.radians(5)*(1-smooth(min(p/.16,1)))-math.radians(5)*smooth(max(0,(p-(stance-.13))/.13))
  else:
   u=(p-stance)/(1-stance);m=-stride*(1-stance)/stance;y=hermite(-stride/2,stride/2,m,m,u);swingphase=u+.16*math.sin(math.pi*u)/math.pi;lift=.075*math.sin(math.pi*swingphase)**2;roll=math.radians(-5+10*smooth(u))
  dq=Quaternion((0,0,1),math.radians(sign*6+direction*2*math.sin(2*math.pi*p)))@Quaternion((1,0,0),roll);q=dq@rest[side+'Foot'].to_quaternion()
  # Roll around the supporting sole vertex, not around the ankle in space.
  # Translation cancels the contact-point drift introduced by rotating the shoe.
  pivot_index=min(range(len(sole[side])),key=lambda i:(dq@sole[side][i]).z)
  pivot=sole[side][pivot_index];rotated=dq@pivot;low=rotated.z
  target=Vector((sign*.17+direction*y-(rotated.x-pivot.x),-.015-(rotated.y-pivot.y),.001-low+lift))
  a,b,c=[side+n for n in ['UpperLeg','LowerLeg','Foot']];h=(world@r.pose.bones[a].matrix).translation;u=(rest[b].translation-rest[a].translation).length;v=(rest[c].translation-rest[b].translation).length;d=target-h;length=d.length;axis=d.normalized()
  assert length/(u+v)<.99,(f,side,length/(u+v))
  pole=Vector((sign*.05,-1,0));pole=(pole-axis*pole.dot(axis)).normalized();along=(u*u-v*v+length*length)/(2*length);knee=h+axis*along+pole*math.sqrt(max(0,u*u-along*along))
  orig=rest[b].translation-rest[a].translation;setw(a,orig.normalized().rotation_difference((knee-h).normalized())@rest[a].to_quaternion());actual=(world@r.pose.bones[b].matrix).translation;orig=rest[c].translation-rest[b].translation;setw(b,orig.normalized().rotation_difference((target-actual).normalized())@rest[b].to_quaternion());setw(c,q)
  stats['feet'][side]={'phase':p,'ankle':list(target),'support':p<stance,'extension':length/(u+v),'target_error':((world@r.pose.bones[c].matrix).translation-target).length,'pivot_index':pivot_index,'moving_contact':list(target+rotated+Vector((direction*speed*f/30,0,0)))}
  swing=math.radians(-2)+math.radians(12)*math.cos(2*math.pi*p-.30);bend=math.radians(24)+math.radians(6)*math.sin(2*math.pi*p-.80);ab=math.radians(11+3*math.sin(2*math.pi*p-.2));chest=(world@r.pose.bones['Chest'].matrix).to_quaternion()@rest['Chest'].to_quaternion().inverted()
  # The forearm reverses about 1.3 frames later than the upper arm.
  fore_swing=math.radians(-2)+math.radians(12)*math.cos(2*math.pi*p-.50)
  for part,nextpart,angle in [('UpperArm','LowerArm',swing),('LowerArm','Hand',fore_swing+bend)]:
   n=side+part;arm_direction=chest@Vector((sign*math.sin(ab),-math.sin(angle)*math.cos(ab),-math.cos(angle)*math.cos(ab)));rd=(rest[side+nextpart].translation-rest[n].translation).normalized();setw(n,rd.rotation_difference(arm_direction)@rest[n].to_quaternion())
   # Small wrist follow-through after the elbow, preserving the relaxed fingers.
  hand=side+'Hand';hm=world@r.pose.bones[hand].matrix
  setw(hand,Quaternion((1,0,0),math.radians(3)*math.sin(2*math.pi*p-.90))@hm.to_quaternion())
 poses.append({b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy()) for b in r.pose.bones});measure.append(stats)
poses.append({n:tuple(v.copy() for v in values) for n,values in poses[0].items()});r.animation_data_create();act=bpy.data.actions.new('aletha_cybernetic_'+stem+'_v1');r.animation_data.action=act;prev={}
for f,pose in enumerate(poses):
 for n,(loc,q,scale) in pose.items():
  b=r.pose.bones[n];b.rotation_mode='QUATERNION'
  if n in prev and q.dot(prev[n])<0:q.negate()
  prev[n]=q.copy();b.location=loc;b.rotation_quaternion=q;b.scale=scale;b.keyframe_insert('location',frame=f,group=n);b.keyframe_insert('rotation_quaternion',frame=f,group=n)
for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
 for k in fc.keyframe_points:k.interpolation='LINEAR'
for a in list(bpy.data.actions):
 if a!=act:bpy.data.actions.remove(a)
s.frame_start=0;s.frame_end=N;s.render.fps=30;s.frame_set(0);bpy.context.preferences.filepaths.save_version=0;bpy.ops.object.select_all(action='DESELECT')
for ob in [r]+meshes:ob.select_set(True)
bpy.context.view_layer.objects.active=r
bpy.ops.export_scene.gltf(filepath=str(out/(stem+'.glb')),export_format='GLB',use_selection=True,export_animations=True,export_animation_mode='ACTIVE_ACTIONS',export_force_sampling=True,export_frame_range=True,export_optimize_animation_size=False)
bpy.ops.wm.save_as_mainfile(filepath=str(out/(stem+'.blend')))
slips=[]
for previous,current in zip(measure,measure[1:]):
 for side in ['Left','Right']:
  a=previous['feet'][side];b=current['feet'][side]
  if a['support'] and b['support'] and b['phase']>a['phase'] and a['pivot_index']==b['pivot_index']:
   slips.append((Vector(a['moving_contact'])-Vector(b['moving_contact'])).length)
assert max(slips)<1e-5,max(slips)
report={'moving_contact_max_step_m':max(slips),'status':'candidate; not installed','fps':30,'cycle_frames':N,'duration':N/30,'nominal_lateral_speed_m_s':speed,'stance_fraction':stance,'max_extension':max(d['extension'] for m in measure for d in m['feet'].values()),'max_ankle_target_error':max(d['target_error'] for m in measure for d in m['feet'].values()),'samples':measure};(out/(stem+'-validation.json')).write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({k:v for k,v in report.items() if k!='samples'}))
