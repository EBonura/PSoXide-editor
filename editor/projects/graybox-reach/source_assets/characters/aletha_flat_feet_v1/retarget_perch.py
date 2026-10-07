"""Refit only the feet to the slab; preserve every approved non-foot pose sample."""
import bpy,json,struct,math,shutil
import numpy as np
from pathlib import Path
from mathutils import Vector,Matrix
O=Path(__file__).resolve().parent;P=O.parents[2];A=P/'source_assets/animations/player/arch_perch_v2';R=P/'validation/aletha-flat-feet-v1';B=Path('/tmp/aletha-flat-feet-originals')
changes=json.loads((O/'vertex-changes.json').read_text());path=A/'arch-perch-a.blend';backup=B/'perch.blend'
if not backup.exists():shutil.copy2(path,backup)
bpy.ops.wm.open_mainfile(filepath=str(backup));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');mesh=bpy.data.objects['Aletha optimized'];world=r.matrix_world.copy();inv=world.inverted();names=[b.name for b in r.data.bones];rest={n:world@r.data.bones[n].matrix_local for n in names};C=Matrix(((1,0,0),(0,0,-1),(0,1,0)))
blob=(B/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes();j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16;raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float);verts=np.array([mesh.matrix_world@v.co for v in mesh.data.vertices]);rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=np.ptp(verts[:,2])/np.ptp(rv[:,2]);center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())
s.frame_set(7);bpy.context.view_layer.update();leg_targets={side:list((world@r.pose.bones[side+'Foot'].matrix).translation) for side in ['Left','Right']};(A/'approved_leg_targets.json').write_text(json.dumps(leg_targets,indent=2)+'\n')
for c in changes:mesh.data.vertices[c['index']].co=mesh.matrix_world.inverted()@Vector(c['new'])
mesh.data.update();model=P/'assets/animations/arch_perch_v2/perch_a.psxanim';old=model.read_bytes();b=bytearray(old);rotations={};soles={};new_contacts={}
for side in ['Left','Right']:
 normal=Vector((0,-1,.3)).normalized();forward=Vector((.1 if side=='Left' else -.1,-.3,-1)).normalized();across=forward.cross(normal).normalized();rotations[side]=Matrix((across,forward,normal)).transposed()@Matrix(((-1,0,0),(0,-1,0),(0,0,1)))
 points=[Vector(c['new']) for c in changes if c['side']==side and c['new'][2]==0];soles[side]=sum(points,Vector())/len(points)
for frame in range(50):
 s.frame_set(frame);bpy.context.view_layer.update();before={n:(world@r.pose.bones[n].matrix).copy() for n in names if n not in ['LeftFoot','RightFoot']}
 for side in ['Left','Right']:
  n=side+'Foot';bone=r.pose.bones[n];head=Vector(leg_targets[side]);rot=rotations[side];offset=rot@(soles[side]-rest[n].translation);head.y=.3*(head.z+offset.z)-offset.y-.001;m=(rot@rest[n].to_3x3()).to_4x4();m.translation=head;bone.matrix=inv@m;bpy.context.view_layer.update()
  for prop in ['location','rotation_quaternion','scale']:bone.keyframe_insert(prop,frame=frame)
  skin=(world@bone.matrix)@rest[n].inverted();rr=C.transposed()@skin.to_3x3()@C;tt=C.transposed()@(skin.translation-center+skin.to_3x3()@center)/unit;flat=[round(rr[row][col]*4096) for col in range(3) for row in range(3)];struct.pack_into('<9h3i',b,20+(frame*26+names.index(n))*30,*flat,*[round(v) for v in tt]);new_contacts[side]=list(head+offset)
 assert max(abs((world@r.pose.bones[n].matrix)[i][k]-m[i][k]) for n,m in before.items() for i in range(4) for k in range(4))<1e-6
for frame in range(50):
 for bone in range(26):
  off=20+(frame*26+bone)*30
  if bone not in [3,6]:assert b[off:off+30]==old[off:off+30]
model.write_bytes(b);s.frame_set(7);bpy.context.view_layer.update();bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(path))
for name,focus,eye,scale in [('perch-feet',(0,.1,.48),(5,-.6,.68),.82),('perch-full',(0,.08,.94),(2,-5,2.8),2.5)]:
 s.render.resolution_x=600;s.render.resolution_y=640 if name.endswith('full') else 480;s.camera.data.ortho_scale=scale;s.camera.location=eye;s.camera.rotation_euler=(Vector(focus)-s.camera.location).to_track_quat('-Z','Y').to_euler();s.render.filepath=str(R/f'{name}.png');bpy.ops.render.render(write_still=True)
def engine_point(p):
 v=C.transposed()@(Vector(p)-center);v.y+=-raw[:,1].min()*unit
 return [round(x*(98/4096/16/unit)) for x in v]
info=json.loads((A/'animation-a.json').read_text())
info['sole_bind_points']={side:[c['new'] for c in changes if c['side']==side and c['new'][2]==0] for side in ['Left','Right']}
info['sole_local_engine']=[engine_point(new_contacts[side]) for side in ['Left','Right']]
info['foot_local_engine']=[engine_point((world@r.pose.bones[side+'Foot'].matrix).translation) for side in ['Left','Right']]
info['sole_alignment']='flat six-vertex heel-to-toe sole, heel above toe, 1mm clearance'
(A/'animation-a.json').write_text(json.dumps(info,indent=2)+'\n')
report={'approved_nonfoot_joint_samples_bit_identical':True,'foot_contacts_world':new_contacts,'perch_frames':50,'bone_count':26,'changed_bones':['LeftFoot','RightFoot']};(O/'perch-validation.json').write_text(json.dumps(report,indent=2)+'\n');print('RESULT',json.dumps(report))
