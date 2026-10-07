"""Committed overhead claw cleave. Blender 5.2, 22-joint light-enemy rig."""
from pathlib import Path
import runpy,math,json,struct
import numpy as np
from mathutils import Matrix,Vector
O=Path(__file__).resolve().parent;P=O.parents[2]
# Reuse the approved rig/sole definitions without executing its authoring main.
shared=runpy.run_path(str(O.parent/'light_motion_v1/author.py'))
for name in ['bind','ib','parents','base','legs','turn','ik','keys','smooth','pack']:
    globals()[name]=shared[name]
N=66;HZ=30;FLOOR=-20000.

def feet_at(k,index):
    side,hip,knee,ankle,toe,relative,center,_=legs[index]
    target=center.copy();angle=0.;plant=True
    if index==0:
        amount=keys(k,[(0,0),(24,0),(32,1),(48,1),(64,0),(66,0)])
        target+=np.array([2400,0,9000])*amount;angle=-14*amount
        for a,b,lift in [(24,32,4800),(48,64,2300)]:
            if a<k<b:target[1]+=lift*math.sin(math.pi*(k-a)/(b-a))**2;plant=False
    r=np.array(Matrix.Rotation(math.radians(angle),3,'Y'));foot=relative.copy()
    foot[:,:3,3]=np.einsum('ij,kj->ki',r,foot[:,:3,3])+target
    foot[:,:3,:3]=np.einsum('ij,kjl->kil',r,foot[:,:3,:3]);return foot,plant

def body_at(k):
    g=base.copy()
    # Compress, show a large coiled silhouette, then drive the pelvis first.
    # Chest, elbow and claw arrive successively later; recovery is deliberately slow.
    hip=keys(k,[(0,0),(18,16),(27,20),(33,-13),(39,-19),(48,-15),(60,2),(66,0)])
    waist=keys(k,[(0,0),(20,22),(29,25),(35,-21),(41,-27),(49,-21),(63,2),(66,0)])
    chest=keys(k,[(0,0),(22,14),(30,17),(36,-15),(42,-19),(51,-13),(66,0)])
    weight=keys(k,[(0,0),(12,-2300),(27,-1400),(34,3100),(39,3700),(48,3000),(64,0),(66,0)])
    depth=keys(k,[(0,0),(19,-2500),(28,-1100),(34,5000),(39,5800),(48,4500),(66,0)])
    height=keys(k,[(0,0),(10,-2300),(22,1000),(28,1700),(31,900),(36,-2900),(40,-3200),(48,-2500),(58,150),(66,0)])
    g[:,:3,3]+=np.array([weight,height,depth])
    turn(g,0,hip,'Y',range(22));turn(g,1,waist,'Y',range(1,14));turn(g,2,chest,'Y',range(2,14))
    fold=keys(k,[(0,0),(20,-18),(29,-21),(36,35),(41,40),(48,34),(58,7),(64,-2),(66,0)])
    turn(g,2,fold,'X',range(2,14));turn(g,2,keys(k,[(0,0),(22,9),(30,11),(37,-12),(43,-15),(52,-8),(66,0)]),'Z',range(2,14))
    turn(g,4,-(hip+waist+chest)*.7,'Y',[4,5]);turn(g,4,-fold*.65,'X',[4,5])
    # Free arm counterbalances the coil and lags the follow-through.
    counter=keys(k,[(0,0),(22,-1.2),(30,-1.1),(39,2.1),(46,1.8),(57,-.15),(66,0)])
    a,b,c=11,12,13
    target=g[c,:3,3]+np.array([-850*counter,1000*abs(counter),-1900*counter])
    reach=target-g[a,:3,3]
    reserve=.965*(np.linalg.norm(g[b,:3,3]-g[a,:3,3])+np.linalg.norm(g[c,:3,3]-g[b,:3,3]))
    target=g[a,:3,3]+reach*min(1.,reserve/np.linalg.norm(reach))
    ik(g,a,b,c,target,g[b,:3,3]-g[a,:3,3])
    a,b,c=7,8,9
    shoulder=g[a,:3,3].copy()
    upper=g[b,:3,3]-shoulder;lower=g[c,:3,3]-g[b,:3,3]
    upper_length=np.linalg.norm(upper);lower_length=np.linalg.norm(lower)
    u=upper/upper_length;l=lower/lower_length
    normal=np.cross(u,l);normal/=np.linalg.norm(normal)
    tangent=np.cross(normal,u)
    rest_bend=math.acos(np.clip(np.dot(u,l),-1,1))
    # One shoulder frame transports the entire chain. The elbow only hinges
    # within that frame; the wrist inherits the forearm plus a modest cock.
    # Author the shoulder relative to the moving chest, not independently
    # in world space. The elbow remains outside the head in the wind-up.
    phase=keys(k,[(0,0),(30,0),(31,.03),(32,.12),(33,.30),(34,.57),(35,.82),(36,1),(40,1.04),(48,1.01),(66,1.01)])
    yaw=math.radians(130-110*phase)
    elevation=.38-.90*phase
    du=np.array([math.sin(yaw)*math.cos(elevation),math.sin(elevation),math.cos(yaw)*math.cos(elevation)])
    dt=np.array([-math.sin(yaw)*math.sin(elevation),math.cos(elevation),-math.cos(yaw)*math.sin(elevation)])
    dn=np.cross(du,dt)
    chest_delta=g[2,:3,:3]@base[2,:3,:3].T
    target_frame=chest_delta@np.column_stack((du,dt,dn))
    old_frame=np.column_stack((u,tangent,normal))
    shoulder_target=Matrix(target_frame@old_frame.T).to_quaternion()
    from mathutils import Quaternion
    shoulder_weight=keys(k,[(0,0),(6,.06),(17,.75),(24,1),(45,1),(55,.48),(64,0),(66,0)])
    rotation=np.array(Quaternion().slerp(shoulder_target,shoulder_weight).to_matrix())
    # Bent load -> forearm unfolding through impact -> relaxed bent recovery.
    bend=math.radians(keys(k,[(0,66.25),(18,108),(27,118),(31,116),(34,82),(36,42),(41,52),(48,68),(58,80),(66,66.25)]))
    elbow_weight=keys(k,[(0,0),(8,.12),(22,1),(50,1),(60,.4),(66,0)])
    axis=rotation@normal
    elbow_rotation=np.array(Matrix.Rotation((bend-rest_bend)*elbow_weight,3,Vector(axis)))
    chain=elbow_rotation@rotation
    g[a,:3,:3]=rotation@g[a,:3,:3]
    g[b,:3,:3]=chain@g[b,:3,:3]
    g[c,:3,:3]=chain@g[c,:3,:3]
    g[b,:3,3]=shoulder+rotation@upper
    g[c,:3,3]=g[b,:3,3]+chain@lower
    wrist=math.radians(keys(k,[(0,0),(23,9),(31,12),(35,4),(38,-7),(45,-5),(57,2),(66,0)]))
    g[c,:3,:3]=np.array(Matrix.Rotation(wrist,3,Vector(axis)))@g[c,:3,:3]
    return g

# A single height correction keeps the pelvis smooth if any pose needs reach reserve.
drop=0.
for k in np.arange(0,N+.01,.25):
    g=body_at(k)
    for i,(_,h,_,_,_,_,_,lengths) in enumerate(legs):
        foot,_=feet_at(k,i);hp,fp=g[h,:3,3],foot[0,:3,3]
        horizontal=np.sum((hp[[0,2]]-fp[[0,2]])**2);limit=.975*sum(lengths)
        assert horizontal<limit**2
        drop=max(drop,hp[1]-fp[1]-math.sqrt(limit**2-horizontal))
frames=[];contacts=[];extensions=[]
for k in range(N+1):
    g=body_at(k);g[:,1,3]-=drop;contact=[]
    for i,(side,h,n,a,t,_,_,_) in enumerate(legs):
        feet,plant=feet_at(k,i);extensions.append(ik(g,h,n,a,feet[0,:3,3],np.array([side*.1,-.12,1.])))
        g[a],g[t]=feet;contact.append(plant)
    frames.append(g@ib);contacts.append(contact)
# Keep a duplicate settled pose after the addressable final frame. The cooker's
# one-shot timing contract treats frame_count-2 as the last gameplay frame.
frames.append(frames[-1].copy());contacts.append(contacts[-1]);frames=np.array(frames)
pack(O/'heavy.psxanim',frames);np.savez(O/'heavy.npz',skin=frames,bind=bind,parents=parents,contact=contacts)
meta={'heavy':{'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'duration_seconds':N/HZ,'loop':False,'speed':0.,'active_start_frame':32,'active_end_frame':37,'pelvis_drop_model_units':drop,'max_leg_extension':max(extensions)}}
(O/'authoring.json').write_text(json.dumps(meta,indent=2));print('RESULT',json.dumps(meta))
