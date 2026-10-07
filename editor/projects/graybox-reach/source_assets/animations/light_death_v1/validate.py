"""Audit decoded export at source frames and fractional PSX matrix interpolation."""
from pathlib import Path
import ast,json,struct
import numpy as np
from mathutils.bvhtree import BVHTree
O=Path(__file__).resolve().parent;P=O.parents[2]
t=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text());exec(compile(ast.Module(body=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),'helper','exec'))
m=model(P/'assets/models/light_body_v1/light.psxmdl');a=animation(O/'death.psxanim');d=np.load(O/'death.npz');meta=json.loads((O/'authoring.json').read_text())['death'];scale=184/65536
assert len(a)==meta['stored_frames'];assert struct.unpack_from('<H',(O/'death.psxanim').read_bytes(),16)[0]==30
points=np.array([deform(m,x) for x in a]);assert np.isfinite(points).all()
floor=[];overlap=[];faces=[[[v[0] for v in f] for f in m['f'] if all(m['owner'][v[0]] in js for v in f)] for js in [[14,15,16,17],[18,19,20,21]]]
for f in np.arange(0,len(a)-1,.25):
 i=int(f);u=f-i;p=[(x[0]*(1-u)+y[0]*u,x[1]*(1-u)+y[1]*u) for x,y in zip(a[i],a[i+1])];v=deform(m,p);floor.append(float((v[:,1].min()+20000)*scale));tr=[BVHTree.FromPolygons(v.tolist(),fs,all_triangles=True) for fs in faces];hits=tr[0].overlap(tr[1])
 if hits:overlap.append((float(f),len(hits)))
foot=[i for i in m['owner'] if m['owner'][i] in [16,17,20,21]];slip=float(np.abs(points[:19,foot]-points[0,foot]).max()*scale)
still=float(np.abs(points[58:]-points[58]).max()*scale)
g=[]
for p in a:
 sk=np.tile(np.eye(4),(22,1,1))
 for j,(r,tr) in enumerate(p):sk[j,:3,:3]=r;sk[j,:3,3]=tr
 g.append(sk@d['bind'])
g=np.array(g);w=np.swapaxes(g[:,8,:3,:3],-1,-2)@g[:,9,:3,:3];dw=np.swapaxes(w[:1],-1,-2)@w;wrist=float(np.degrees(np.arccos(np.clip((np.trace(dw,axis1=-2,axis2=-1)-1)/2,-1,1))).max())
steps=np.sqrt(np.mean(np.sum(np.diff(points,axis=0)**2,axis=2),axis=1))*scale
report={'stored_frames':len(a),'duration_seconds':meta['duration_seconds'],'minimum_floor_clearance_engine_units':min(floor),'max_planted_foot_drift_engine_units':slip,'corpse_motion_after_frame58_engine_units':still,'max_wrist_relative_rotation_degrees':wrist,'leg_overlap_quarter_frames':overlap,'max_frame_vertex_rms_step_engine_units':float(steps.max()),'largest_step_frame':int(np.argmax(steps)),'corpse_height_engine_units':float(np.ptp(points[-1,:,1])*scale),'final_lowest_vertices_joints':[int(m['owner'][i]) for i in np.argsort(points[-1,:,1])[:12]]}
(O/'validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps(report))
assert min(floor)>-.15,report
assert slip<.08 and still==0 and wrist<15,report
assert not overlap,report
