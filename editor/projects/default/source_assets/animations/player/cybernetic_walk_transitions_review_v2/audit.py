import bpy,json,math
from pathlib import Path
out=Path(__file__).resolve().parent;old=out.parent/'cybernetic_walk_transitions_review_v1';report={}
def vertices():
 dg=bpy.context.evaluated_depsgraph_get();points=[]
 for o in bpy.context.scene.objects:
  if o.type!='MESH':continue
  ev=o.evaluated_get(dg);me=ev.to_mesh();points.extend(ev.matrix_world@v.co for v in me.vertices);ev.to_mesh_clear()
 return points
for stem in ['walk_fwd_windup','walk_fwd_winddown','walk_fwd_winddown_mirror']:
 bpy.ops.wm.open_mainfile(filepath=str(old/(stem+'.blend')));s=bpy.context.scene;reference=[]
 for f in [0,s.frame_end]:s.frame_set(f);reference.append(vertices())
 bpy.ops.wm.open_mainfile(filepath=str(out/(stem+'.blend')));s=bpy.context.scene;ends=[]
 for f in [0,s.frame_end]:s.frame_set(f);ends.append(vertices())
 drift=max((a-b).length for pa,pb in zip(reference,ends) for a,b in zip(pa,pb));assert drift<1e-5,drift
 lowest=100;max_step=0;prev={};rig=next(o for o in s.objects if o.type=='ARMATURE')
 for f in range(s.frame_end*2+1):
  s.frame_set(f//2,subframe=(f%2)*.5);lowest=min(lowest,min(v.z for v in vertices()))
  for b in rig.pose.bones:
   q=b.rotation_quaternion.copy()
   if b.name in prev:max_step=max(max_step,math.degrees(q.rotation_difference(prev[b.name]).angle))
   prev[b.name]=q
 report[stem]={'endpoint_vertex_difference_vs_v1_m':drift,'half_frame_lowest_vertex_z':lowest,'max_local_rotation_half_frame_degrees':max_step}
p=out/'validation.json';d=json.loads(p.read_text());d['polish_audit']=report;p.write_text(json.dumps(d,indent=2)+'\n');print('RESULT',json.dumps(report))
