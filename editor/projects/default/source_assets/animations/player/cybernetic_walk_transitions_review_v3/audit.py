import bpy,json
from pathlib import Path
out=Path(__file__).resolve().parent;report={}
for version in [2,3]:
 path=out.parent/('cybernetic_walk_transitions_review_v'+str(version))/'new_A.blend';bpy.ops.wm.open_mainfile(filepath=str(path));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');points=[]
 for f in range(23,30):
  s.frame_set(f);points.append({side:(r.matrix_world@r.pose.bones[side+'Foot'].matrix).translation.copy() for side in ['Left','Right']})
 report[str(version)]={side:[(b[side]-a[side]).length for a,b in zip(points,points[1:])] for side in ['Left','Right']}
assert min(report['3']['Left'][1:5])>.02,report
f=out/'validation.json';d=json.loads(f.read_text());d['handoff_audit']={'join_frame':26,'step_frame_pairs':[[f,f+1] for f in range(23,29)],'foot_displacement_metres_per_frame':report,'scope':'Source joined preview. Current engine crossfade not applied to v3 start seam.'};f.write_text(json.dumps(d,indent=2)+'\n');print('RESULT',json.dumps(report))
