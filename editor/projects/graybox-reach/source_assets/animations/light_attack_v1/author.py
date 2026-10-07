"""Dedicated single claw sweep. Blender 5.2, original 22-joint light-enemy rig."""
from pathlib import Path
import runpy,math,json,struct
import numpy as np
from mathutils import Matrix,Vector
O=Path(__file__).resolve().parent;P=O.parents[2]
# Reuse the approved rig/sole definitions without executing its authoring main.
shared=runpy.run_path(str(O.parent/'light_motion_v1/author.py'))
for name in ['bind','ib','parents','base','legs','turn','ik','keys','smooth','pack']:
    globals()[name]=shared[name]
N=48;HZ=30;FLOOR=-20000.

def feet_at(k,index):
    side,hip,knee,ankle,toe,relative,center,_=legs[index]
    target=center.copy();angle=0.;plant=True
    if index==0:
        amount=keys(k,[(0,0),(13,0),(20,1),(34,1),(46,0),(48,0)])
        target+=np.array([1700,0,6200])*amount;angle=-16*amount
        for a,b,lift in [(13,20,4100),(34,46,2300)]:
            if a<k<b:target[1]+=lift*math.sin(math.pi*(k-a)/(b-a))**2;plant=False
    r=np.array(Matrix.Rotation(math.radians(angle),3,'Y'));foot=relative.copy()
    foot[:,:3,3]=np.einsum('ij,kj->ki',r,foot[:,:3,3])+target
    foot[:,:3,:3]=np.einsum('ij,kjl->kil',r,foot[:,:3,:3]);return foot,plant

def body_at(k):
    g=base.copy()
    # Compress, show a large coiled silhouette, then drive the pelvis first.
    # Chest, elbow and claw arrive successively later; recovery is deliberately slow.
    hip=keys(k,[(0,0),(12,22),(16,24),(21,-23),(25,-27),(31,-20),(48,0)])
    waist=keys(k,[(0,0),(13,28),(18,32),(23,-32),(27,-36),(32,-24),(48,0)])
    chest=keys(k,[(0,0),(14,17),(19,19),(24,-20),(28,-24),(34,-14),(48,0)])
    weight=keys(k,[(0,0),(10,-2300),(16,-1800),(22,2700),(26,3300),(32,2400),(46,0),(48,0)])
    depth=keys(k,[(0,0),(12,-2000),(17,-900),(22,3600),(26,4200),(32,3300),(48,0)])
    height=keys(k,[(0,0),(8,-1700),(16,900),(19,500),(23,-1800),(26,-2300),(31,-1000),(40,-150),(48,0)])
    g[:,:3,3]+=np.array([weight,height,depth])
    turn(g,0,hip,'Y',range(22));turn(g,1,waist,'Y',range(1,14));turn(g,2,chest,'Y',range(2,14))
    fold=keys(k,[(0,0),(14,-12),(18,-10),(25,24),(29,27),(34,15),(48,0)])
    turn(g,2,fold,'X',range(2,14));turn(g,2,keys(k,[(0,0),(15,12),(19,10),(25,-17),(29,-20),(35,-9),(48,0)]),'Z',range(2,14))
    turn(g,4,-(hip+waist+chest)*.7,'Y',[4,5]);turn(g,4,-fold*.65,'X',[4,5])
    # Free arm counterbalances the coil and lags the follow-through.
    counter=keys(k,[(0,0),(15,-.7),(19,-.6),(27,1.6),(32,1.3),(48,0)])
    a,b,c=11,12,13
    target=g[c,:3,3]+np.array([-850*counter,1000*abs(counter),-1900*counter])
    ik(g,a,b,c,target,g[b,:3,3]-g[a,:3,3])
    a,b,c=7,8,9
    shoulder=g[a,:3,3].copy();old_upper=g[b,:3,3]-shoulder
    old_lower=g[c,:3,3]-g[b,:3,3];old_r=g[c,:3,:3].copy()
    # FK sweep: upper arm drives first, bent forearm trails it, wrist releases
    # last. The wrist is never aimed at a forward target or moved on a reach ray.
    # Hold the threat, burst through the contact in five frames, then overshoot.
    phase=keys(k,[(0,0),(19,0),(20,.08),(21,.25),(22,.55),(23,.82),(24,1),(28,1.07),(33,1.02),(48,1.02)])
    upper_angle=math.radians(125-235*phase)
    bend=math.radians(90-52*math.sin(math.pi*min(phase,1.)))
    lower_angle=upper_angle+bend
    upper_elevation=.58-.87*smooth(min(phase/.8,1.))
    lower_elevation=.68-1.16*smooth(min(phase/.85,1.))
    def direction(yaw,elevation):
        return np.array([math.sin(yaw)*math.cos(elevation),math.sin(elevation),math.cos(yaw)*math.cos(elevation)])
    influence=keys(k,[(0,0),(4,.12),(12,.9),(16,1),(33,1),(40,.48),(48,0)])
    desired_upper=direction(upper_angle,upper_elevation)
    desired_lower=direction(lower_angle,lower_elevation)
    # Blend rotations, rather than positions, to keep both limb lengths fixed.
    from mathutils import Quaternion
    qu=Quaternion().slerp(Vector(old_upper).rotation_difference(Vector(desired_upper)),influence)
    ql=Quaternion().slerp(Vector(old_lower).rotation_difference(Vector(desired_lower)),influence)
    upper=np.array(qu.to_matrix())@old_upper;lower=np.array(ql.to_matrix())@old_lower
    g[a,:3,:3]=np.array(qu.to_matrix())@g[a,:3,:3]
    g[b,:3,:3]=np.array(ql.to_matrix())@g[b,:3,:3]
    g[b,:3,3]=shoulder+upper;g[c,:3,3]=shoulder+upper+lower
    # The claw follows the forearm around the arc, then rolls down and across
    # the far hip; this is a raking sweep, not the alert's pointing attitude.
    claw_angle=lower_angle+math.radians(35-55*phase)
    claw_elevation=.45-1.03*smooth(min(phase/.85,1.))
    tip=np.array([-443.90856805,10211.15579709,844.17601394])
    aim=np.array(Vector(old_r@tip).rotation_difference(Vector(direction(claw_angle,claw_elevation))).to_matrix())@old_r
    g[c,:3,:3]=np.array(Matrix(old_r).to_quaternion().slerp(Matrix(aim).to_quaternion(),influence).to_matrix())
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
pack(O/'strike.psxanim',frames);np.savez(O/'strike.npz',skin=frames,bind=bind,parents=parents,contact=contacts)
meta={'strike':{'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'duration_seconds':N/HZ,'loop':False,'speed':0.,'active_start_frame':20,'active_end_frame':25,'pelvis_drop_model_units':drop,'max_leg_extension':max(extensions)}}
(O/'authoring.json').write_text(json.dumps(meta,indent=2));print('RESULT',json.dumps(meta))
