"""Light enemy HitReact: planted asymmetric recoil and measured guard recovery."""
from pathlib import Path
import runpy,math,json,struct
import numpy as np
from mathutils import Matrix,Vector

O=Path(__file__).resolve().parent;P=O.parents[2]
shared=runpy.run_path(str(O.parent/'light_motion_v1/author.py'))
for name in ['bind','ib','parents','base','legs','turn','ik','keys','pack']:
    globals()[name]=shared[name]
N=27;HZ=30;FLOOR=-20000.

def feet_at(k,index):
    side,hip,knee,ankle,toe,relative,center,_=legs[index]
    foot=relative.copy();foot[:,:3,3]+=center
    return foot,True

def body_at(k):
    g=base.copy()
    # The chest takes the impact first; pelvis catches the force two frames
    # later. Both soles remain fixed while soft knees absorb the weight.
    shift=keys(k,[(0,0),(3,-300),(6,-900),(9,-850),(16,-200),(22,70),(27,0)])
    depth=keys(k,[(0,0),(4,-1100),(7,-1450),(10,-1300),(18,-150),(23,80),(27,0)])
    height=keys(k,[(0,0),(3,300),(7,-650),(10,-800),(18,-140),(24,0),(27,0)])
    g[:,:3,3]+=np.array([shift,height,depth])
    hip=keys(k,[(0,0),(2,0),(6,7),(10,7),(19,-1),(27,0)])
    waist=keys(k,[(0,0),(4,11),(8,13),(12,10),(20,-2),(27,0)])
    chest=keys(k,[(0,0),(3,16),(5,19),(8,19),(14,7),(21,-2),(27,0)])
    turn(g,0,hip,'Y',range(22));turn(g,1,waist,'Y',range(1,14));turn(g,2,chest,'Y',range(2,14))
    recoil=keys(k,[(0,0),(4,-18),(7,-17),(10,-11),(17,3),(23,1),(27,0)])
    roll=keys(k,[(0,0),(4,10),(8,12),(15,4),(22,-1),(27,0)])
    turn(g,2,recoil,'X',range(2,14));turn(g,2,roll,'Z',range(2,14))
    # Delayed head recoil; gaze returns before the shoulders finish settling.
    nod=keys(k,[(0,0),(2,1),(6,-13),(9,-11),(14,4),(20,0),(27,0)])
    gaze=keys(k,[(0,0),(3,0),(7,10),(10,7),(16,-4),(22,0),(27,0)])
    turn(g,4,nod,'X',[4,5]);turn(g,4,gaze,'Y',[4,5])
    for a,b,c,side,amount in [(7,8,9,1,1.),(11,12,13,-1,.65)]:
        drag=keys(k,[(0,0),(3,.10),(7,1),(10,.96),(17,.25),(23,-.06),(27,0)])
        turn(g,a,18*drag*amount,'X',[a,b,c])
        turn(g,a,side*9*drag*amount,'Z',[a,b,c])
        # Open the elbow slightly with the impact, then curl into guard.
        bend=keys(k,[(0,0),(3,0),(7,-14),(10,-12),(17,8),(23,2),(27,0)])*amount
        u=g[b,:3,3]-g[a,:3,3];l=g[c,:3,3]-g[b,:3,3]
        axis=np.cross(u,l);axis/=np.linalg.norm(axis)
        r=np.array(Matrix.Rotation(math.radians(bend),3,Vector(axis)));pivot=g[b,:3,3].copy()
        for j in [b,c]:
            g[j,:3,:3]=r@g[j,:3,:3];g[j,:3,3]=pivot+r@(g[j,:3,3]-pivot)
    return g

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
        foot,plant=feet_at(k,i)
        extensions.append(ik(g,h,n,a,foot[0,:3,3],np.array([side*.1,-.12,1.])))
        g[a],g[t]=foot;contact.append(plant)
    frames.append(g@ib);contacts.append(contact)
frames.append(frames[-1].copy());contacts.append(contacts[-1]);frames=np.array(frames)
pack(O/'hit.psxanim',frames)
# Preserve the authored sample rate; HitReact binding plays this clip at 1x.
data=bytearray((O/'hit.psxanim').read_bytes());struct.pack_into('<H',data,16,HZ);(O/'hit.psxanim').write_bytes(data)
np.savez(O/'hit.npz',skin=frames,bind=bind,parents=parents,contact=contacts)
meta={'hit':{'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'playback_speed_q8':256,'duration_seconds':N/HZ,'source_duration_seconds':N/HZ,'loop':False,'speed':0.,'pelvis_drop_model_units':drop,'max_leg_extension':max(extensions)}}
(O/'authoring.json').write_text(json.dumps(meta,indent=2));print('RESULT',json.dumps(meta))
