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
clip=next(folder.glob('clip_06_*.psxanim'));R,T,n,hz=decode(clip);assert (n,hz)==(9,15)
poses=[];worldposes=[]
for f in range(n):
 delta=T[f,0]-T[0,0];delta[1]=0
 for k,name in enumerate(names):
  rot=C@Matrix(R[f,k].tolist())@C.transposed();skin=rot.to_4x4();skin.translation=center-rot@center+C@Vector(((T[f,k]-delta)*unit).tolist());r.pose.bones[name].matrix=inv@skin@rest[name];bpy.context.view_layer.update()
 poses.append({b.name:b.matrix_basis.copy() for b in r.pose.bones});worldposes.append({b.name:(world@b.matrix).copy() for b in r.pose.bones})
# Reconstruct the existing cycle first; all edits are layered onto these poses.
N=18;prev={};r.animation_data_create();act=bpy.data.actions.new('Aletha original-based forward run v2');r.animation_data.action=act;metrics=[]
for f in range(N+1):
 phase=(f%N)/2;a=int(phase);b=(a+1)%n;u=phase-a
 for name in names:
  la,qa,sa=poses[a][name].decompose();lb,qb,sb=poses[b][name].decompose();bone=r.pose.bones[name];bone.rotation_mode='QUATERNION';bone.location=la.lerp(lb,u);bone.rotation_quaternion=qa.slerp(qb,u);bone.scale=sa.lerp(sb,u)
 bpy.context.view_layer.update()
 baseline={name:(world@r.pose.bones[name].matrix).copy() for name in names if not any(x in name for x in ['Arm','Hand','Thumb','Index'])}
 # Preserve the original lean, pelvis path, legs, shoulder drive and timing.
 # Open the elbows by just 3 degrees; keep the original opposing arm action.
 for side,sign in [('Left',1),('Right',-1)]:
  bone=r.pose.bones[side+'UpperArm'];m=world@bone.matrix;rot=Quaternion((0,1,0),math.radians(-sign*3));q=rot@m.to_quaternion();mat=q.to_matrix().to_4x4();mat.translation=m.translation;bone.matrix=inv@mat;bpy.context.view_layer.update()
 # Small wrist smoothing in local space, retaining the original hand attitude.
 for side in ['Left','Right']:
  name=side+'Hand';bone=r.pose.bones[name];q=bone.rotation_quaternion.copy();neighbor=poses[(a-1)%n][name].to_quaternion().slerp(poses[(b+1)%n][name].to_quaternion(),.5);bone.rotation_quaternion=q.slerp(neighbor,.08)
 bpy.context.view_layer.update();preserved=max(max(abs((world@r.pose.bones[name].matrix)[i][j]-m[i][j]) for i in range(4) for j in range(4)) for name,m in baseline.items());assert preserved<1e-5,preserved
 metrics.append({'frame':f,'unchanged_body_max_matrix_delta':preserved,'hips':list((world@r.pose.bones['Hips'].matrix).translation),'chest':list((world@r.pose.bones['Chest'].matrix).translation),'neck':list((world@r.pose.bones['Neck'].matrix).translation)})
 for bone in r.pose.bones:
  q=bone.rotation_quaternion.copy()
  if bone.name in prev and q.dot(prev[bone.name])<0:q.negate()
  prev[bone.name]=q.copy();bone.rotation_quaternion=q
  for prop in ['location','rotation_quaternion','scale']:bone.keyframe_insert(prop,frame=f,group=bone.name)
for fc in action_get_channelbag_for_slot(act,r.animation_data.action_slot).fcurves:
 for k in fc.keyframe_points:k.interpolation='LINEAR'
for a in list(bpy.data.actions):
 if a!=act:bpy.data.actions.remove(a)
s.frame_start=0;s.frame_end=N;s.render.fps=30;s.frame_set(0);bpy.context.preferences.filepaths.save_version=0
bpy.ops.object.select_all(action='DESELECT')
for ob in [r]+meshes:ob.select_set(True)
bpy.context.view_layer.objects.active=r
bpy.ops.export_scene.gltf(filepath=str(out/'run_fwd.glb'),export_format='GLB',use_selection=True,export_animations=True,export_animation_mode='ACTIVE_ACTIONS',export_force_sampling=True,export_frame_range=True,export_optimize_animation_size=False)
bpy.ops.wm.save_as_mainfile(filepath=str(out/'run_fwd.blend'))
report={'status':'candidate; not installed','base':'Current original Run clip 77, reconstructed from its cooked skin matrices','base_sha256':hashlib.sha256(clip.read_bytes()).hexdigest(),'fps':30,'cycle_frames':N,'duration':.6,'coordinate_unit':unit,'coordinate_center':list(center),'changes':'3-degree elbow clearance; 8-percent neighboring-key wrist smoothing. Original body pitch, pelvis, leg poses and cadence retained.','samples':metrics};(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT original-based run reconstructed',unit,list(center))
