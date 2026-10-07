"""Audit the cooked clips and mesh, including fractional matrix contacts.
Usage: Python with numpy validate_cooked.py <generated light model directory>.
"""
from pathlib import Path
import ast,json,struct,sys
import numpy as np
O=Path(__file__).resolve().parent;P=O.parents[2];stage=Path(sys.argv[1])
t=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text())
exec(compile(ast.Module(body=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),'helper','exec'))
source=model(P/'assets/models/light_body_v1/light.psxmdl');cooked=model(stage/'mesh.psxmdl');report={};meta=json.loads((O/'authoring.json').read_text())
for name,prefix in [('run','clip_03'),('turn','clip_04'),('alert','clip_05')]:
    a=next(stage.glob(prefix+'*.psxanim'));poses=animation(a);d=np.load(O/f'{name}.npz')
    assert len(poses)==len(d['skin']);assert struct.unpack_from('<H',a.read_bytes(),16)[0]==30
    if meta[name]['loop']:assert all(np.array_equal(x[0],y[0]) and np.array_equal(x[1],y[1]) for x,y in zip(poses[0],poses[-1]))
    errors=[];spreads=[];drifts=[]
    for leg,joints in enumerate([[16,17],[20,21]]):
        ids=[i for i in source['owner'] if source['owner'][i] in joints];low=min(source['v'][i][1] for i in ids);sole=[i for i in ids if source['v'][i][1]==low]
        for fi in range(len(poses)-1):
            if not d['contact'][fi,leg]:continue
            fractions=[0,.25,.5,.75,1] if d['contact'][fi+1,leg] else [0]
            start=deform(cooked,poses[fi])[sole]
            for alpha in fractions:
                pose=[(x[0]*(1-alpha)+y[0]*alpha,x[1]*(1-alpha)+y[1]*alpha) for x,y in zip(poses[fi],poses[fi+1])];v=deform(cooked,pose)
                errors.append(float(np.abs(v[sole,1]+1250).max()*184/4096));spreads.append(float(np.ptp(v[sole,1])*184/4096))
                delta=(v[sole]-start)*184/4096+np.array([0,0,meta[name]['speed']/30*alpha]);drifts.extend(np.linalg.norm(delta,axis=1))
    assert max(errors)<.06 and max(spreads)<.06 and max(drifts)<.1,(name,max(errors),max(spreads),max(drifts))
    report[name]={'stored_frames':len(poses),'sample_hz':30,'max_planted_sole_spread_engine_units':max(spreads),'max_planted_sole_floor_error_engine_units':max(errors),'max_contact_drift_engine_units':float(max(drifts)),'contact_samples':len(errors)}
(P/'validation/light-motion-v1/cooked-validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
