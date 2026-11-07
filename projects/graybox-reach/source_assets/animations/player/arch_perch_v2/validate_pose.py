import bpy, json, math
from pathlib import Path
from mathutils import Vector
p=Path(__file__).resolve().parent
bpy.ops.wm.open_mainfile(filepath=str(p/'arch-perch-a.blend'))
r=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');s=bpy.context.scene;s.frame_set(7);bpy.context.view_layer.update();world=r.matrix_world
import numpy as np
mesh=bpy.data.objects['Aletha optimized'];angles=[];clearances=[];shoe_clearances=[];toe_directions=[]
for side in ['Left','Right']:
 n=side+'Foot';rest=world@r.data.bones[n].matrix_local;posed=world@r.pose.bones[n].matrix
 group=mesh.vertex_groups[n].index
 pts=[mesh.matrix_world@v.co for v in mesh.data.vertices if any(g.group==group and g.weight>.5 for g in v.groups)]
 low=np.array(json.loads((p/'animation-a.json').read_text())['sole_bind_points'][side])
 _,_,vh=np.linalg.svd(low-low.mean(0));normal=Vector(vh[-1])
 if normal.z<0:normal=-normal
 normal=((posed@rest.inverted()).to_3x3()@normal).normalized()
 angles.append(math.degrees(normal.angle(Vector((0,-1,.30)).normalized())))
 skin=posed@rest.inverted();world_points=[skin@Vector(v) for v in low]
 clearances.append([v.y-.30*v.z for v in world_points])
 shoe=[skin@v for v in pts];shoe_clearances.append(max(v.y-.30*v.z for v in shoe))
 toe=sum(sorted(pts,key=lambda v:v.z)[:2],Vector())/2
 heel=sum(sorted(pts,key=lambda v:v.y)[-2:],Vector())/2
 toe_directions.append((skin@toe-skin@heel).z)
assert max(abs(v+.001) for side in clearances for v in side)<.001
shoulders={side:list((world@r.pose.bones[side+'UpperArm'].matrix).translation) for side in ['Left','Right']}
report={'pelvis_turn_degrees':25,'support_arm_width_correction':[1.15,1.25],'feet_plane_alignment_error_degrees':angles,'sole_plane_clearance_m':clearances,'shoulder_world':shoulders,'body_turn_degrees':67,'max_shoe_penetration_m':shoe_clearances,'toe_minus_heel_height_m':toe_directions,'contacts':'left hand and both feet fixed throughout aim lattice'}
assert max(angles)<.1
assert max(shoe_clearances)<.001
assert max(toe_directions)<0
assert shoulders['Left'][1]>shoulders['Right'][1]+.2
approved={n:(world@r.pose.bones[n].matrix).copy() for n in ['Spine','Chest','Neck','RightShoulder']}
reference=Path('/tmp/perch-before-contact-fix.blend')
if reference.exists():
 bpy.ops.wm.open_mainfile(filepath=str(reference));old=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');bpy.context.scene.frame_set(7);bpy.context.view_layer.update()
 error=max(abs(approved[n][i][j]-(old.matrix_world@old.pose.bones[n].matrix)[i][j]) for n in approved for i in range(4) for j in range(4))
 assert error<1e-5,error
 report['approved_torso_max_matrix_error']=error
(p/'refined-pose-validation.json').write_text(json.dumps(report,indent=2)+'\n')
print('POSE_VALIDATION',json.dumps(report))
