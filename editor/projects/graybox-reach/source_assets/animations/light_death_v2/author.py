"""Authored fatal stumble followed by offline constrained-body dynamics."""
from pathlib import Path
import ast,runpy,math,json
import bpy,bmesh,numpy as np
from mathutils import Matrix,Vector
O=Path(__file__).resolve().parent;P=O.parents[2];B=P.parents[2]/'build/graybox-reach/light-death-v2'
s=runpy.run_path(str(O.parent/'light_motion_v1/author.py'))
for name in ['bind','ib','parents','turn','ik','keys','pack','mesh','deform','legs']:globals()[name]=s[name]
C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted()
N=84;PRE=18;HZ=30;FLOOR=-20000.
old=np.load(O.parent/'light_death_v1/death.npz')['skin']@bind
start=old[0].copy();prepared=[]
for k in range(PRE+1):
 # Fatal recoil then a failed recovery step on the non-claw side.
 source=min(int(round(k*24/PRE)),24);g=old[source].copy();t=keys(k,[(0,0),(6,0),(18,1)])
 g[:,:3,3]+=np.array([-2000*t,-1800*t,-2500*t])
 turn(g,0,-65*t,'X',range(22))
 turn(g,2,-25*t,'X',range(2,14))
 for i,(side,h,n,a,toe,relative,center,lengths) in enumerate(legs):
  foot=relative.copy();target=center.copy()
  if i==1:
   step=keys(k,[(0,0),(5,0),(13,1),(18,1)]);target+=np.array([-400,0,-2600])*step
   if 5<k<13:target[1]+=2200*math.sin(math.pi*(k-5)/8)**2
  foot[:,:3,3]+=target
  ik(g,h,n,a,foot[0,:3,3],np.array([side*.2,-.2,1.]));g[a],g[toe]=foot
 prepared.append(g)
# Rigid clusters keep neck segments, the claws and feet attached as authored.
groups=[(0,[0,1],5),(2,[2,3,6,10],10),(4,[4,5],2),
 (7,[7],2),(8,[8,9],4),(11,[11],2),(12,[12,13],3),
 (14,[14],4),(15,[15],3),(16,[16,17],1.5),(18,[18],4),(19,[19],3),(20,[20,21],1.5)]
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
scene=bpy.context.scene;scene.render.fps=30;scene.frame_start=1;scene.frame_end=N+1;scene.gravity=(0,0,-18.)
initial_skin=start@ib;vertices=deform(mesh,[(x[:3,:3],x[:3,3]) for x in initial_skin]);bodies={};initial={};group_for={}
for anchor,joints,mass in groups:
 # Full convex hulls of the actual rigid geometry keep the long extremities clear.
 ids=[i for i in mesh['owner'] if mesh['owner'][i] in joints]
 M=C@Matrix(start[anchor].tolist())@CI
 local=[tuple(M.inverted()@(C@Vector(tuple(vertices[i])+(1,))))[:3] for i in ids]
 if len(local)<4 or min(np.ptp(np.array(local),axis=0))<.005:
  # Some structural pivots own no triangles; use a compact joint shell.
  local=[(x,y,z) for x in [-.05,.05] for y in [-.05,.05] for z in [-.05,.05]]
 me=bpy.data.meshes.new(f'Collider_{anchor}');me.from_pydata(local,[],[]);me.update()
 bm=bmesh.new();bm.from_mesh(me);bmesh.ops.convex_hull(bm,input=list(bm.verts),use_existing_faces=False);bm.to_mesh(me);bm.free();me.update()
 ob=bpy.data.objects.new(f'Body_{anchor}',me);scene.collection.objects.link(ob);ob.matrix_world=M
 bpy.context.view_layer.update();bpy.context.view_layer.objects.active=ob;ob.select_set(True);bpy.ops.rigidbody.object_add();ob.select_set(False)
 rb=ob.rigid_body;rb.mass=mass;rb.kinematic=True;rb.collision_shape='CONVEX_HULL';rb.use_margin=True;rb.collision_margin=.008;rb.friction=.9;rb.restitution=0.;rb.linear_damping=.30;rb.angular_damping=.55;rb.use_deactivation=False
 bodies[anchor]=ob;initial[anchor]=M.copy()
 for j in joints:group_for[j]=anchor
 for k,g in enumerate(prepared):
  scene.frame_set(k+1);M=C@Matrix(g[anchor].tolist())@CI;ob.matrix_world=M;ob.rotation_mode='QUATERNION';ob.keyframe_insert(data_path='location');ob.keyframe_insert(data_path='rotation_quaternion')
 rb.kinematic=True;rb.keyframe_insert(data_path='kinematic',frame=1);rb.keyframe_insert(data_path='kinematic',frame=PRE+1);rb.kinematic=False;rb.keyframe_insert(data_path='kinematic',frame=PRE+2)
 # Linear translation carries the last authored momentum into the fall.
 from bpy_extras.anim_utils import action_get_channelbag_for_slot
 for fc in action_get_channelbag_for_slot(ob.animation_data.action,ob.animation_data.action_slot).fcurves:
  for key in fc.keyframe_points:key.interpolation='CONSTANT' if fc.data_path=='rigid_body.kinematic' else 'LINEAR'
# Contact plane is thick and static; visual floor corresponds to model Y=-20000.
bpy.ops.mesh.primitive_cube_add(size=1,location=(0,0,-2.1));ground=bpy.context.object;ground.name='Ground';ground.scale=(30,30,.2);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);bpy.ops.rigidbody.object_add();ground.rigid_body.type='PASSIVE';ground.rigid_body.friction=1.;ground.rigid_body.restitution=0.
# Constraint frames at the actual joint pivots; hinges follow the rig's bend plane.
links=[(0,2,1,'GENERIC',(-35,45)),(2,4,4,'GENERIC',(-45,45)),
 (2,7,7,'GENERIC',(-90,90)),(7,8,8,'HINGE',(-20,85)),
 (2,11,11,'GENERIC',(-90,90)),(11,12,12,'HINGE',(-20,85)),
 (0,14,14,'GENERIC',(-65,65)),(14,15,15,'HINGE',(-25,90)),(15,16,16,'GENERIC',(-30,30)),
 (0,18,18,'GENERIC',(-55,55)),(18,19,19,'HINGE',(-25,90)),(19,20,20,'GENERIC',(-30,30))]
scene.frame_set(1)
for pa,ch,j,typ,limits in links:
 ob=bpy.data.objects.new(f'Joint_{j}',None);scene.collection.objects.link(ob);ob.location=(C@Vector(tuple(start[j,:3,3])+(1,)))[:3]
 if typ=='HINGE':
  nxt={8:9,12:13,15:16,19:20}[j];p=int(parents[j]);u=start[j,:3,3]-start[p,:3,3];v=start[nxt,:3,3]-start[j,:3,3];axis=Vector(np.cross(u,v));axis=C.to_3x3()@axis;axis.normalize();ob.rotation_euler=axis.to_track_quat('Z','Y').to_euler()
 bpy.context.view_layer.update();bpy.context.view_layer.objects.active=ob;ob.select_set(True);bpy.ops.rigidbody.constraint_add();ob.select_set(False);co=ob.rigid_body_constraint;co.type=typ;co.object1=bodies[pa];co.object2=bodies[ch];co.disable_collisions=True
 for ax in 'xyz':
  if typ=='GENERIC':
   setattr(co,'use_limit_lin_'+ax,True);setattr(co,'limit_lin_'+ax+'_lower',0);setattr(co,'limit_lin_'+ax+'_upper',0)
   setattr(co,'use_limit_ang_'+ax,True);setattr(co,'limit_ang_'+ax+'_lower',math.radians(limits[0]));setattr(co,'limit_ang_'+ax+'_upper',math.radians(limits[1]))
 co.use_limit_ang_z=True;co.limit_ang_z_lower=math.radians(limits[0]);co.limit_ang_z_upper=math.radians(limits[1])
world=scene.rigidbody_world;world.substeps_per_frame=10;world.solver_iterations=100;world.point_cache.frame_start=1;world.point_cache.frame_end=N+1
scene.frame_set(1);bpy.context.view_layer.update();frames=[]
for fi in range(1,N+2):
 scene.frame_set(fi);bpy.context.view_layer.update();dg=bpy.context.evaluated_depsgraph_get();g=start.copy()
 for j in range(22):
  anchor=group_for[j];M=bodies[anchor].evaluated_get(dg).matrix_world
  g[j]=np.array(CI@M@initial[anchor].inverted()@C@Matrix(start[j].tolist()))
 frames.append(g@ib)
frames=np.array(frames)
# Keep the corpse exact after residual Bullet jitter has settled.
points=np.array([deform(mesh,[(m[:3,:3],m[:3,3]) for m in row]) for row in frames]);tail_speed=np.linalg.norm(points[-1]-points[-2],axis=1).max()*184/65536
# Ease residual solver motion into a fixed rest pose, then project anchor
# positions along the hierarchy: the exported rig cannot stretch at a joint.
raw_frames=frames.copy()
rest=raw_frames[56]@bind
for fi,f in enumerate(frames):
 g=raw_frames[min(fi,56)]@bind;t=keys(fi,[(0,0),(48,0),(56,1),(84,1)])
 if t:
  for anchor,js,_ in groups:
   current=g[anchor].copy();target=rest[anchor];new=np.eye(4)
   new[:3,:3]=np.array(Matrix(current[:3,:3]).to_quaternion().slerp(Matrix(target[:3,:3]).to_quaternion(),t).to_matrix());new[:3,3]=current[:3,3]*(1-t)+target[:3,3]*t
   delta=new@np.linalg.inv(current)
   for j in js:g[j]=delta@g[j]
 for pa,ch,j,_,_ in links:
  anchor=np.append(start[j,:3,3],1)
  expected=(g[pa]@np.linalg.inv(start[pa])@anchor)[:3]
  actual=(g[ch]@np.linalg.inv(start[ch])@anchor)[:3]
  delta=expected-actual
  for bone in next(js for a,js,_ in groups if a==ch):g[bone,:3,3]+=delta
 frames[fi]=g@ib
frames[56:]=frames[56]
# Correct only the small numerical contact skin, not the body's fall trajectory.
corrections=[]
for f in frames:
 v=deform(mesh,[(m[:3,:3],m[:3,3]) for m in f]);lift=max(0,FLOOR-v[:,1].min());f[:,1,3]+=lift;corrections.append(lift*184/65536)
frames=np.concatenate([frames,frames[-1:]])
pack(O/'death.psxanim',frames);np.savez(O/'death.npz',skin=frames,bind=bind,parents=parents)
meta={'death':{'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'playback_speed_q8':256,'duration_seconds':N/HZ,'loop':False,'speed':0.,'still_from_frame':56,'physics_release_frame':PRE+1,'tail_max_vertex_speed_engine_units_per_frame':float(tail_speed),'maximum_floor_correction_engine_units':max(corrections)}}
(O/'authoring.json').write_text(json.dumps(meta,indent=2));bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(O/'physics-source.blend'));print('RESULT',json.dumps(meta))
