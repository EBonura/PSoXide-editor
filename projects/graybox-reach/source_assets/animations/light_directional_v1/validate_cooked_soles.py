from pathlib import Path
import ast,struct,json,argparse
import numpy as np
parser=argparse.ArgumentParser();parser.add_argument('cooked_model_directory',type=Path);args=parser.parse_args()
s=Path(__file__).resolve().parent;p=s.parents[2];stage=args.cooked_model_directory;t=ast.parse((p/'tools/enemy_reduction/validate_render.py').read_text());exec(compile(ast.Module(body=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),'helper','exec'));source=model(p/'assets/models/light_body_v1/light.psxmdl');cooked=model(stage/'mesh.psxmdl');report={}
for name,prefix in [('walk_backward','clip_10'),('strafe_left','clip_06'),('strafe_right','clip_07')]:
 a=next(stage.glob(prefix+'*.psxanim'));poses=animation(a);d=np.load(s/f'{name}.npz');assert len(poses)==len(d['skin'])+1;assert struct.unpack_from('<H',a.read_bytes(),16)[0]==15;errors=[];spreads=[]
 for leg,joints in enumerate([[16,17],[20,21]]):
  indices=[i for i in source['owner'] if source['owner'][i] in joints];low=min(source['v'][i][1] for i in indices);sole=[i for i in indices if source['v'][i][1]==low]
  for fi in range(len(poses)-1):
   if not d['contact'][fi,leg]:continue
   fractions=[0,.25,.5,.75] if d['contact'][(fi+1)%(len(poses)-1),leg] else [0]
   for alpha in fractions:
    pose=[(x[0]*(1-alpha)+y[0]*alpha,x[1]*(1-alpha)+y[1]*alpha) for x,y in zip(poses[fi],poses[fi+1])];v=deform(cooked,pose);errors.append(float(np.abs(v[sole,1]+1250).max()*184/4096));spreads.append(float(np.ptp(v[sole,1])*184/4096))
 assert max(errors)<.06 and max(spreads)<.06,(name,max(errors),max(spreads))
 report[name]={'max_planted_sole_spread_engine_units':max(spreads),'max_planted_sole_floor_error_engine_units':max(errors),'samples':len(errors)}
(p/'validation/light-directional-v1/flat-cooked-soles.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
