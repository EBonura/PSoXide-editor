"""Check cooked death geometry, contacts, final hold and sample timing."""
from pathlib import Path
import ast,json,struct,sys
import numpy as np
O=Path(__file__).resolve().parent;P=O.parents[2];V=P/'validation/light-death-v1';stage=Path(sys.argv[1])
t=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text());exec(compile(ast.Module(body=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),'helper','exec'))
m=model(stage/'mesh.psxmdl');path=next(stage.glob('clip_*_light_enemy_death.psxanim'));a=animation(path);assert len(a)==68;assert struct.unpack_from('<H',path.read_bytes(),16)[0]==30
points=np.array([deform(m,p) for p in a]);scale=184/4096
floor=[]
for f in np.arange(0,len(a)-1,.25):
 i=int(f);u=f-i;p=[(x[0]*(1-u)+y[0]*u,x[1]*(1-u)+y[1]*u) for x,y in zip(a[i],a[i+1])];floor.append(float((deform(m,p)[:,1].min()+1250)*scale))
feet=[i for i in m['owner'] if m['owner'][i] in [16,17,20,21]];slip=float(np.abs(points[:19,feet]-points[0,feet]).max()*scale);still=float(np.abs(points[58:]-points[58]).max()*scale)
assert min(floor)>-.15 and slip<.1 and still==0,(min(floor),slip,still)
r={'clip':path.name,'stored_frames':68,'sample_hz':30,'duration_seconds':2.2,'minimum_floor_clearance_engine_units':min(floor),'planted_foot_drift_engine_units':slip,'corpse_motion_after_frame58_engine_units':still,'fractional_samples':len(floor)};(V/'cooked-validation.json').write_text(json.dumps(r,indent=2));print(json.dumps(r,indent=2))
