from pathlib import Path
import ast,struct,json
import numpy as np
from mathutils.bvhtree import BVHTree
o=Path(__file__).resolve().parent;p=o.parents[2];t=ast.parse((p/'tools/enemy_reduction/validate_render.py').read_text());exec(compile(ast.Module(body=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'model','deform'}],type_ignores=[]),'helper','exec'));m=model(p/'assets/models/light_body_v1/light.psxmdl');faces=[]
for joints in [[14,15,16,17],[18,19,20,21]]:faces.append([[v[0] for v in f] for f in m['f'] if all(m['owner'][v[0]] in joints for v in f)])
report={}
for name in ['strafe_left','strafe_right']:
 d=np.load(o/f'{name}.npz');hits=[]
 for k in np.arange(0,len(d['skin']),.25):
  a=int(k);t=k-a;f=d['skin'][a]*(1-t)+d['skin'][(a+1)%len(d['skin'])]*t;v=deform(m,[(m[:3,:3],m[:3,3]) for m in f]);trees=[BVHTree.FromPolygons(v.tolist(),fs,all_triangles=True) for fs in faces];overlap=trees[0].overlap(trees[1])
  if overlap:hits.append((float(k),len(overlap)))
 assert not hits,(name,hits)
 g=d['skin']@d['bind'];span=g[:,16,0,3]-g[:,20,0,3];knees=g[:,15,0,3]-g[:,19,0,3]
 assert span.min()>0 and span.max()<20000,(name,span.min(),span.max())
 report[name]={'intersections':hits,'min_ankle_span':float(span.min()),'max_ankle_span':float(span.max()),'max_knee_span':float(knees.max()),'quarter_frame_checks':len(d['skin'])*4}
(o/'narrow-stance-validation.json').write_text(json.dumps(report,indent=2)+'\n')
print('SPACING',json.dumps(report))
