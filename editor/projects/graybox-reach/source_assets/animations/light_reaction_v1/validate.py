"""Validate decoded PSXA, matrix interpolation, foot contacts and leg clearance."""
from pathlib import Path
import ast,json,struct
import numpy as np
from mathutils.bvhtree import BVHTree
OUT=Path(__file__).resolve().parent;P=OUT.parents[2]
tree=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),'mesh helpers','exec'))
m=model(P/'assets/models/light_body_v1/light.psxmdl');scale=184/4096/16
meta=json.loads((OUT/'authoring.json').read_text());report={}
faces=[[[v[0] for v in f] for f in m['f'] if all(m['owner'][v[0]] in joints for v in f)] for joints in [[14,15,16,17],[18,19,20,21]]]
for name,spec in meta.items():
    data=np.load(OUT/f'{name}.npz');contact=data['contact'];bind=data['bind'];poses=animation(OUT/f'{name}.psxanim')
    assert len(poses)==spec['stored_frames']
    if spec['loop']:assert all(np.array_equal(a[0],b[0]) and np.array_equal(a[1],b[1]) for a,b in zip(poses[0],poses[-1]))
    points=np.array([deform(m,p) for p in poses]);assert np.isfinite(points).all()
    global_poses=[]
    for pose in poses:
        sk=np.tile(np.eye(4),(22,1,1))
        for j,(r,t) in enumerate(pose):sk[j,:3,:3]=r;sk[j,:3,3]=t
        global_poses.append(sk@bind)
    g=np.array(global_poses);extension=[];flex=[];sole_errors=[];sole_spread=[];drift=[]
    for leg,(h,k,a,t) in enumerate([(14,15,16,17),(18,19,20,21)]):
        hp,kp,fp=g[:,h,:3,3],g[:,k,:3,3],g[:,a,:3,3]
        extension.extend(np.linalg.norm(fp-hp,axis=1)/(np.linalg.norm(kp-hp,axis=1)+np.linalg.norm(fp-kp,axis=1)))
        u,v=hp-kp,fp-kp
        knee=180-np.degrees(np.arccos(np.clip(np.sum(u*v,axis=1)/np.linalg.norm(u,axis=1)/np.linalg.norm(v,axis=1),-1,1)))
        flex.extend(knee[contact[:,leg]])
        ids=[i for i in range(len(m['v'])) if m['owner'][i] in [a,t]]
        low=min(m['v'][i][1] for i in ids);sole=[i for i in ids if m['v'][i][1]==low]
        for i in range(len(poses)):
            if contact[i,leg]:
                sole_errors.append(float(np.abs(points[i,sole,1]+20000).max()*scale))
                sole_spread.append(float(np.ptp(points[i,sole,1])*scale))
        for i in range(len(poses)-1):
            if contact[i,leg] and contact[i+1,leg]:
                delta=(points[i+1,ids]-points[i,ids])*scale+np.array([0,0,spec['speed']/30])
                drift.extend(np.linalg.norm(delta,axis=1))
    overlap=[];floor=[]
    for k in np.arange(0,len(poses)-1,.25):
        i=int(k);alpha=k-i
        part=[(a[0]*(1-alpha)+b[0]*alpha,a[1]*(1-alpha)+b[1]*alpha) for a,b in zip(poses[i],poses[i+1])]
        v=deform(m,part);floor.append(float((v[:,1].min()+20000)*scale))
        trees=[BVHTree.FromPolygons(v.tolist(),fs,all_triangles=True) for fs in faces]
        hits=trees[0].overlap(trees[1])
        if hits:overlap.append((float(k),len(hits)))
    steps=np.sqrt(np.mean(np.sum(np.diff(points,axis=0)**2,axis=2),axis=1))*scale
    d={'stored_frames':len(poses),'duration_seconds':spec['duration_seconds'],'max_leg_extension':float(max(extension)),
       'mean_planted_knee_flexion_degrees':float(np.mean(flex)),'max_planted_sole_spread_engine_units':max(sole_spread),
       'max_planted_sole_floor_error_engine_units':max(sole_errors),'max_contact_drift_engine_units':float(max(drift)),
       'lowest_vertex_above_floor_engine_units':min(floor),'leg_overlap_quarter_frames':overlap,
       'max_frame_vertex_rms_step_engine_units':float(max(steps)),'loop_seam_over_largest_step':float(steps[-1]/max(steps)) if spec['loop'] else None}
    if name=='stun':
        assert contact[:,1].all(), 'Rear foot stays planted throughout the stun'
        rear_ids=[i for i in m['owner'] if m['owner'][i] in [20,21]]
        rear_motion=float(np.abs(points[:,rear_ids]-points[0,rear_ids]).max()*scale)
        d['rear_foot_motion_engine_units']=rear_motion
        assert rear_motion<.03, ('Rear foot moved',rear_motion)
    arms=[]
    for h,n,w in [(7,8,9),(11,12,13)]:
        arms.extend(np.linalg.norm(g[:,w,:3,3]-g[:,h,:3,3],axis=1)/(np.linalg.norm(g[:,n,:3,3]-g[:,h,:3,3],axis=1)+np.linalg.norm(g[:,w,:3,3]-g[:,n,:3,3],axis=1)))
    d['max_arm_extension']=float(max(arms))
    # Guard the revised connected chain: elbow folds without locking and the
    # wrist follows the forearm rather than independently aiming the claw.
    upper=g[:,8,:3,3]-g[:,7,:3,3];lower=g[:,9,:3,3]-g[:,8,:3,3]
    bend=np.degrees(np.arccos(np.clip(np.sum(upper*lower,axis=1)/np.linalg.norm(upper,axis=1)/np.linalg.norm(lower,axis=1),-1,1)))
    wrist=np.swapaxes(g[:,8,:3,:3],-1,-2)@g[:,9,:3,:3]
    wrist_delta=np.swapaxes(wrist[:1],-1,-2)@wrist
    wrist_angle=np.degrees(np.arccos(np.clip((np.trace(wrist_delta,axis1=-2,axis2=-1)-1)*.5,-1,1)))
    d['elbow_bend_min_degrees']=float(bend.min());d['elbow_bend_max_degrees']=float(bend.max())
    d['wrist_max_offset_from_rest_degrees']=float(wrist_angle.max())
    assert bend.min()>30 and bend.max()<130,(name,d)
    assert wrist_angle.max()<15,(name,d)
    assert max(arms)<.99,(name,d)
    assert max(extension)<.99,(name,d)
    assert max(sole_spread)<.03 and max(sole_errors)<.03,(name,d)
    assert max(drift)<.08,(name,d)
    # A rigid shin fin extends slightly past the ankle at full reach.
    # Keep whole-mesh penetration under 0.15 engine units; soles use 0.03 above.
    assert min(floor)>-.15,(name,d)
    assert not overlap,(name,d)
    report[name]=d
(OUT/'validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps(report))
