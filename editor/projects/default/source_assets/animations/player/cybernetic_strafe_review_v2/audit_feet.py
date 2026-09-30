import bpy,json
from pathlib import Path
out=Path(__file__).resolve().parent
for stem in ['walk_lft','walk_rgt']:
 bpy.ops.wm.open_mainfile(filepath=str(out/(stem+'.blend')));s=bpy.context.scene;meshes=[o for o in s.objects if o.type=='MESH'];gaps=[]
 for f in range(61):
  s.frame_set(f//2,subframe=(f%2)/2);dg=bpy.context.evaluated_depsgraph_get();feet={side:[] for side in ['Left','Right']}
  for ob in meshes:
   ev=ob.evaluated_get(dg);me=ev.to_mesh()
   for side in feet:
    vg=ob.vertex_groups.get(side+'Foot')
    if vg:
     feet[side].extend(ev.matrix_world@me.vertices[v.index].co for v in ob.data.vertices if any(g.group==vg.index and g.weight>.85 for g in v.groups))
   ev.to_mesh_clear()
  gaps.append(min(v.x for v in feet['Left'])-max(v.x for v in feet['Right']))
 report=json.loads((out/(stem+'-validation.json')).read_text());report['minimum_foot_mesh_lateral_gap_m']=min(gaps);assert min(gaps)>0,(stem,min(gaps));(out/(stem+'-validation.json')).write_text(json.dumps(report,indent=2));print('FOOT_CLEARANCE',stem,min(gaps))
