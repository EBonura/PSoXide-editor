import bpy,math,json
from pathlib import Path
from mathutils import Matrix,Vector,Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
project=Path(__file__).resolve().parents[4];p=project/'review/heavy-attack-v1';out=Path(__file__).resolve().parent
print('VERSION',bpy.app.version_string)
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
s=bpy.context.scene;s.render.fps=30
bpy.ops.import_scene.gltf(filepath=str(project/'source_assets/animations/player/direct_shoulders/r2_horizon_heavy.glb'))
r=next(o for o in s.objects if o.type=='ARMATURE');a=bpy.data.actions[0];meshes=[o for o in s.objects if o.type=='MESH']
for t in list(r.animation_data.nla_tracks):r.animation_data.nla_tracks.remove(t)
r.animation_data.action=a;r.animation_data.action_slot=a.slots[0]
rest={b.name:r.matrix_world@b.matrix_local for b in r.data.bones}
def sample(f):
 s.frame_set(int(f),subframe=f-int(f));return {'local':{b.name:b.matrix_basis.copy() for b in r.pose.bones},'world':{b.name:r.matrix_world@b.matrix for b in r.pose.bones}}
base=sample(186);start=sample(18)
foot_local={}
for side in ['Left','Right']:
 name=side+'Foot';points=[]
 for ob in meshes:
  group=ob.vertex_groups.get(name)
  if group:
   points.extend(rest[name].inverted()@(ob.matrix_world@v.co) for v in ob.data.vertices if any(g.group==group.index and g.weight>.8 for g in v.groups))
 assert points,side
 foot_local[side]=points
floor_z={side:min((base['world'][side+'Foot']@v).z for v in points) for side,points in foot_local.items()}

keys=[(4,18),(7,38),(11,54),(17,80),(21,106),(23,110),(26,120),(29,144),(36,152),(49,172),(60,186)]
def source_pose(f):
 if f<4:
  q=max(f,0)/4;q=q*q*(3-2*q);return {'local':{n:m.lerp(start['local'][n],q) for n,m in base['local'].items()}}
 for (fa,va),(fb,vb) in zip(keys,keys[1:]):
  if fa<=f<=fb:return sample(va+(vb-va)*(f-fa)/(fb-fa))
 return base
raw=[]
for f in range(61):
 data=source_pose(f)
 # The torso starts the cut; the sword arm and free arm follow its drive.
 lag=math.sin(math.pi*max(0,min(1,(f-16)/22))) if 16<f<38 else 0
 for side,delay in [('Right',.3),('Left',.45)]:
  arm=source_pose(f-delay*lag)
  for n in data['local']:
   if n.startswith(side) and any(part in n for part in ['Shoulder','Arm','Hand','Thumb','Index']):data['local'][n]=arm['local'][n]
 raw.append(data)
r.animation_data_clear()
world=r.matrix_world.copy();inv=world.inverted()
def update():bpy.context.view_layer.update()
def wm(n):return world@r.pose.bones[n].matrix
def setworld(n,q,pos=None):
 m=q.to_matrix().to_4x4();m.translation=wm(n).translation if pos is None else pos;r.pose.bones[n].matrix=inv@m;update()
def damp_delta(q,baseq,weights):
 d=q@baseq.inverted()
 if d.w<0:d.negate()
 v=d.to_exponential_map();return Quaternion(Vector([v[i]*weights[i] for i in range(3)]))@baseq
def curve(f,keys):
 # Monotone Hermite spacing: continuous speed through intermediate keys.
 if f<=keys[0][0]:return keys[0][1]
 if f>=keys[-1][0]:return keys[-1][1]
 slopes=[(y-x)/(b-a) for (a,x),(b,y) in zip(keys,keys[1:])]
 tangents=[0.0]
 for x,y in zip(slopes,slopes[1:]):tangents.append(0.0 if x*y<=0 else 2*x*y/(x+y))
 tangents.append(0.0)
 for i,((a,x),(b,y)) in enumerate(zip(keys,keys[1:])):
  if a<=f<=b:
   u=(f-a)/(b-a);h=b-a
   return (2*u**3-3*u*u+1)*x+(u**3-2*u*u+u)*h*tangents[i]+(-2*u**3+3*u*u)*y+(u**3-u*u)*h*tangents[i+1]
poses=[];foot_errors=[];hand_positions=[];pelvis=[];foot_paths={s:[] for s in ['Left','Right']};drops=[]
for f,data in enumerate(raw):
 for n,m in data['local'].items():r.pose.bones[n].matrix_basis=m
 update();w={b.name:wm(b.name).copy() for b in r.pose.bones}
 hip=base['world']['Hips'].translation.copy()
 hip.z+=curve(f,[(0,0),(6,-.10),(13,-.34),(18,-.35),(22,-.14),(23,-.15),(27,-.33),(32,-.30),(39,-.26),(45,-.22),(51,-.10),(56,-.015),(60,0)])
 # Load the supporting left leg before the right foot leaves the ground.
 hip.x+=curve(f,[(0,0),(10,.14),(16,.155),(20,.05),(25,-.17),(32,-.15),(40,.02),(46,.12),(60,0)])
 hip.y+=curve(f,[(0,0),(12,.11),(17,.085),(21,-.17),(25,-.51),(30,-.55),(37,-.46),(43,-.39),(49,-.19),(55,-.025),(60,0)])
 coil=math.radians(curve(f,[(0,0),(17,-10),(21,-12),(27,14),(31,15),(45,4),(60,0)]))
 setworld('Hips',Quaternion(Vector((0,0,1)),coil)@damp_delta(w['Hips'].to_quaternion(),base['world']['Hips'].to_quaternion(),(1.05,1.0,1.0)),hip)
 # Chest follows the hip release, then carries on as the front knee catches it.
 twist=math.radians(curve(f,[(0,0),(18,-10),(22,-12),(29,16),(32,18),(44,6),(60,0)]))
 lean=math.radians(curve(f,[(0,0),(13,-6),(21,-9),(27,25),(32,21),(44,7),(60,0)]))
 for n,weights,amount in [('Spine',(1.02,.92,1.12),.5),('Chest',(1.02,.92,1.12),1),('Neck',(.82,.8,.95),.8)]:
  extra=Quaternion(Vector((0,0,1)),twist*amount)@Quaternion(Vector((1,0,0)),lean*amount)
  setworld(n,extra@damp_delta(w[n].to_quaternion(),base['world'][n].to_quaternion(),weights))
 # Open the silhouette: both arms reach farther through wind-up and cut.
 expansion=curve(f,[(0,0),(12,0),(18,.8),(23,1),(29,.9),(37,.35),(46,0),(60,0)])
 for side,gain in [('Right',.18),('Left',.22)]:
  aa,bb,cc=[side+x for x in ['UpperArm','LowerArm','Hand']]
  aw,bw,cw=[wm(n).copy() for n in [aa,bb,cc]];h=aw.translation;oldk=bw.translation;oldhand=cw.translation
  u=(oldk-h).length;v=(oldhand-oldk).length;d=oldhand-h;direction=d.normalized()
  length=min(d.length*(1+gain*expansion),(u+v)*.97);target=h+direction*length
  pole=oldk-h;pole=(pole-direction*pole.dot(direction)).normalized()
  along=(u*u-v*v+length*length)/(2*length);elbow=h+direction*along+pole*math.sqrt(max(0,u*u-along*along))
  setworld(aa,(oldk-h).normalized().rotation_difference((elbow-h).normalized())@aw.to_quaternion())
  current=wm(bb).translation;setworld(bb,(oldhand-oldk).normalized().rotation_difference((target-current).normalized())@bw.to_quaternion());setworld(cc,cw.to_quaternion())
 # A right-foot step drives the cut; the left foot pivots on the forefoot.
 targets={};footq={}
 step=curve(f,[(0,0),(16,0),(23,1),(45,1),(58,0),(60,0)])
 lift=0
 if 16<f<23:lift=.15*math.sin(math.pi*(f-16)/7)
 if 45<f<58:lift=.07*math.sin(math.pi*(f-45)/13)
 for side in ['Left','Right']:
  bw=base['world'];name=side+'Foot';target=bw[name].translation.copy()
  if side=='Right':
   target.y-=.76*step;target.x-=.075*step;target.z+=lift
   yaw=math.radians(curve(f,[(0,0),(16,0),(23,-20),(45,-20),(58,0),(60,0)]))
   landing=math.radians(curve(f,[(0,0),(18,0),(22,-12),(25,0),(60,0)]))
   delta=Quaternion(Vector((0,0,1)),yaw)@Quaternion(Vector((1,0,0)),landing)
  else:
   yaw=math.radians(curve(f,[(0,0),(16,-12),(25,60),(32,65),(45,20),(60,0)]))
   pitch=math.radians(curve(f,[(0,0),(18,0),(24,29),(33,32),(44,0),(60,0)]))
   delta=Quaternion(Vector((0,0,1)),yaw)@Quaternion(Vector((1,0,0)),pitch)
   toe=Vector((0,-.105,-.13));target+=toe-delta@toe
  footq[side]=delta@bw[name].to_quaternion()
  # Keep the actual lowest sole vertex on the floor, including heel rise.
  minz=min((footq[side]@v).z for v in foot_local[side])
  target.z=floor_z[side]+(lift if side=='Right' else 0)-minz
  targets[side]=target
 # Lower the centre only if an ankle target would overextend a leg.
 drop=0
 for side in ['Left','Right']:
  aa,bb,cc=[side+x for x in ['UpperLeg','LowerLeg','Foot']];bw=base['world'];h=wm(aa).translation;target=targets[side]
  reach=((bw[bb].translation-bw[aa].translation).length+(bw[cc].translation-bw[bb].translation).length)*.992
  zmax=target.z+math.sqrt(max(0,reach*reach-(h.x-target.x)**2-(h.y-target.y)**2));drop=min(drop,zmax-h.z)
 if drop<0:
  h=wm('Hips');pos=h.translation;pos.z+=drop;setworld('Hips',h.to_quaternion(),pos)
 drops.append(drop)
 for side in ['Left','Right']:
  aa,bb,cc=[side+x for x in ['UpperLeg','LowerLeg','Foot']];bw=base['world'];h=wm(aa).translation;target=targets[side]
  u=(bw[bb].translation-bw[aa].translation).length;v=(bw[cc].translation-bw[bb].translation).length
  d=target-h;length=d.length;direction=d.normalized();assert length<u+v,(f,side,length,u+v)
  # The knee follows the planted foot's toe direction through the pivot.
  delta=footq[side]@bw[cc].to_quaternion().inverted();pole=delta@Vector((0,-1,0));pole=(pole-direction*pole.dot(direction)).normalized()
  along=(u*u-v*v+length*length)/(2*length);knee=h+direction*along+pole*math.sqrt(max(0,u*u-along*along))
  old=(bw[bb].translation-bw[aa].translation).normalized();setworld(aa,old.rotation_difference((knee-h).normalized())@bw[aa].to_quaternion())
  actual=wm(bb).translation;old=(bw[cc].translation-bw[bb].translation).normalized();setworld(bb,old.rotation_difference((target-actual).normalized())@bw[bb].to_quaternion());setworld(cc,footq[side])
  foot_errors.append((wm(cc).translation-target).length);foot_paths[side].append(list(wm(cc).translation))
 poses.append({b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy()) for b in r.pose.bones});hand_positions.append(list(wm('RightHand').translation));pelvis.append(list(wm('Hips').translation))
# Explicit matching neutral endpoints, with quaternion sign continuity.
poses[-1]={n:tuple(x.copy() for x in v) for n,v in poses[0].items()}
# Align the broad YZ blade faces with the cutting planes through forearm
# pronation. Keep each hand's authored local grip and wrist path intact.
for f,pose in enumerate(poses):
 for n,(loc,q,scale) in pose.items():
  b=r.pose.bones[n];b.location=loc;b.rotation_quaternion=q;b.scale=scale
 update()
 for side,rollkeys in [('Right',[(0,0),(11,0),(21,-35),(24,-54),(26,-65),(28,-84),(36,-84),(51,0),(60,0)]),('Left',[(0,0),(17,0),(23,50),(25,74),(29,65),(36,65),(51,0),(60,0)])]:
  n=side+'LowerArm';axis=(wm(side+'Hand').translation-wm(n).translation).normalized()
  setworld(n,Quaternion(axis,math.radians(curve(f,rollkeys)))@wm(n).to_quaternion())
 poses[f]={b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy()) for b in r.pose.bones}
r.animation_data_create();act=bpy.data.actions.new('aletha_cybernetic_heavy_attack_v1');r.animation_data.action=act;previous={}
for f,pose in enumerate(poses):
 for n,(loc,q,scale) in pose.items():
  b=r.pose.bones[n];b.rotation_mode='QUATERNION'
  if n in previous and q.dot(previous[n])<0:q=-q
  previous[n]=q.copy();b.location=loc;b.rotation_quaternion=q;b.scale=scale
  for prop in ['location','rotation_quaternion','scale']:b.keyframe_insert(prop,frame=f,group=n)
for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
 for k in fc.keyframe_points:k.interpolation='LINEAR'
for old in list(bpy.data.actions):
 if old!=act:bpy.data.actions.remove(old)
s.frame_start=0;s.frame_end=60;s.frame_set(0)

# Character-only candidate export. The runtime attachments are preview geometry.
bpy.ops.object.select_all(action='DESELECT')
for ob in [r]+meshes:ob.select_set(True)
bpy.context.view_layer.objects.active=r;bpy.context.preferences.filepaths.save_version=0
bpy.ops.wm.save_as_mainfile(filepath=str(out/'heavy_attack.blend'))
bpy.ops.export_scene.gltf(filepath=str(out/'heavy_attack.glb'),export_format='GLB',use_selection=True,export_animations=True,export_animation_mode='ACTIVE_ACTIONS',export_frame_range=True,export_force_sampling=True,export_optimize_animation_size=False)
# Reuse the exact original dual-weapon attachments for the review stage.
bpy.ops.wm.open_mainfile(filepath=str(p/'original-preview.blend'));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');r.animation_data_clear();r.animation_data_create();act=bpy.data.actions.new('heavy_attack_v1_studio');r.animation_data.action=act;previous={}
for f,pose in enumerate(poses):
 for n,(loc,q,scale) in pose.items():
  b=r.pose.bones[n];b.rotation_mode='QUATERNION'
  if n in previous and q.dot(previous[n])<0:q=-q
  previous[n]=q.copy();b.location=loc;b.rotation_quaternion=q;b.scale=scale
  for prop in ['location','rotation_quaternion','scale']:b.keyframe_insert(prop,frame=f,group=n)
for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
 for k in fc.keyframe_points:k.interpolation='LINEAR'
for side,cast in [('Right',11),('Left',17)]:
 for suffix,events in [('wire',[(0,True),(cast-5,False),(cast,True),(45,False),(51,True)]),('blade',[(0,True),(cast,False),(45,True)])]:
  ob=bpy.data.objects[side+' '+suffix];ob.animation_data_clear()
  for f,v in events:ob.hide_render=v;ob.keyframe_insert('hide_render',frame=f)
s.frame_start=0;s.frame_end=60;s.render.fps=30;s.frame_set(0);s.render.resolution_x=720;s.render.resolution_y=720
bpy.ops.wm.save_as_mainfile(filepath=str(p/'preview.blend'))
for label,offset in [('front',(0,-6,1.5)),('quarter',(3.8,-5.4,1.65))]:
 cam=s.camera;target=Vector((0,-.3,1.03));cam.location=target+Vector(offset);cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=4.65
 s.render.image_settings.media_type='IMAGE';s.render.image_settings.file_format='PNG'
 for f in [0,11,17,21,23,25,26,29,33,40,50,60]:
  s.frame_set(f);s.render.filepath=str(p/(label+f'-{f:03}.png'));bpy.ops.render.render(write_still=True)
report={'status':'review candidate; not installed','fps':30,'frames':[0,60],'cast_right':[6,11],'cast_left':[12,17],'strike_proposal':[23,29],'dissolve':[45,51],'max_ankle_target_error_m':max(foot_errors),'foot_paths':foot_paths,'pelvis_positions':pelvis,'hand_positions':hand_positions,'source':'direct_shoulders/r2_horizon_heavy.glb','preview':'Existing heavy/right and light/left meshes; schematic materialization'}
(out/'validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps({'max_ankle_target_error_m':max(foot_errors),'candidate':str(out)}))
