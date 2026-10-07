"""Validate the exported PSXA, including sub-frame matrix interpolation."""
from pathlib import Path
import ast, json, math, struct
import numpy as np
OUT=Path(__file__).resolve().parent
PROJECT=OUT.parents[2]
source=PROJECT/'tools/enemy_reduction/validate_render.py'
tree=ast.parse(source.read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),str(source),'exec'))
m=model(PROJECT/'assets/models/light_body_v1/light.psxmdl')
SCALE=184/4096/16
report={}
for name in ['walk_backward','strafe_left','strafe_right']:
    data=np.load(OUT/f'{name}.npz')
    assert struct.unpack_from('<H',(OUT/f'{name}.psxanim').read_bytes(),16)[0]==15
    poses=animation(OUT/f'{name}.psxanim')
    assert all(np.array_equal(a[0],b[0]) and np.array_equal(a[1],b[1]) for a,b in zip(poses[0],poses[-1]))
    poses=poses[:-1]
    count=len(poses)
    contact=data['contact']
    assert all(contact.any(axis=1))
    points=np.array([deform(m,p) for p in poses])
    assert np.isfinite(points).all()
    globals=[]
    for pose in poses:
        mats=np.tile(np.eye(4),(22,1,1))
        for j,(r,t) in enumerate(pose):mats[j,:3,:3]=r;mats[j,:3,3]=t
        globals.append(mats@data['bind'])
    globals=np.array(globals)
    extensions=[]
    for hip,knee,foot in [(14,15,16),(18,19,20)]:
        h,k,f=globals[:,hip,:3,3],globals[:,knee,:3,3],globals[:,foot,:3,3]
        extensions.extend(np.linalg.norm(f-h,axis=1)/(np.linalg.norm(k-h,axis=1)+np.linalg.norm(f-k,axis=1)))
    drift=[]
    sole_spreads=[]
    sole_floor_errors=[]
    for leg,joints in enumerate([[16,17],[20,21]]):
        indices=[i for i in range(len(m['v'])) if m['owner'][i] in joints]
        bind_floor=min(m['v'][i][1] for i in indices)
        # Contact corners of the actual authored sole (heel and toe), not the
        # lowest point of a pitched foot. Both sole planes are flat in bind.
        sole=[i for i in indices if m['v'][i][1]==bind_floor]
        assert len(sole)>=4
        for fi in range(count):
            if contact[fi,leg]:
                sole_spreads.append(float(np.ptp(points[fi,sole,1])*SCALE))
                sole_floor_errors.append(float(np.abs(points[fi,sole,1]+20000).max()*SCALE))
        for fi in range(count):
            next_frame=(fi+1)%count
            if contact[fi,leg] and contact[next_frame,leg]:
                for alpha in [.25,.5,.75,1.]:
                    pose=[(a[0]*(1-alpha)+b[0]*alpha,a[1]*(1-alpha)+b[1]*alpha) for a,b in zip(poses[fi],poses[next_frame])]
                    p=deform(m,pose)[indices]
                    slip=(p-points[fi,indices])*SCALE+data['direction']*(30/15*alpha)
                    drift.extend(np.linalg.norm(slip,axis=1))
    steps=np.sqrt(np.mean(np.sum(np.diff(points,axis=0)**2,axis=2),axis=1))
    seam=np.sqrt(np.mean(np.sum((points[-1]-points[0])**2,axis=1)))
    floor_range=float(np.ptp(points[:,:,1].min(1))*SCALE)
    frame_delta=float(max(np.linalg.norm(points[(i+1)%count]-points[i],axis=1).max() for i in range(count))*SCALE)
    result={'frames':count,'sample_hz':15,'bytes':(OUT/f'{name}.psxanim').stat().st_size,
            'max_contact_drift_engine_units':float(max(drift)),
            'max_planted_sole_height_spread':max(sole_spreads),
            'max_planted_sole_floor_error':max(sole_floor_errors),
            'floor_range_engine_units':floor_range,'max_leg_extension':float(max(extensions)),
            'loop_seam_over_largest_step':float(seam/steps.max()),'largest_vertex_step_engine_units':frame_delta}
    assert max(sole_spreads)<.03,(name,'tilted sole',result)
    assert max(sole_floor_errors)<.03,(name,'sole contact',result)
    assert max(drift)<.03,(name,'slip',result)
    assert floor_range<.08,(name,'floor',result)
    assert max(extensions)<.955,(name,'knee',result)
    assert seam/steps.max()<1.15,(name,'seam',result)
    report[name]=result
report['limits']='Straight steady movement; no runtime foot IK. Engine integer skinning, collision-clipped motion, orbit turning and radial correction add error beyond these source checks.'
(OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
