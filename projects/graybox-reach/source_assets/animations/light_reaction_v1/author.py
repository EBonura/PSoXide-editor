"""Light enemy poise-break reaction: recoil, catch weight, recover guard."""
from pathlib import Path
import runpy,math,json,struct
import numpy as np
from mathutils import Matrix,Vector

O=Path(__file__).resolve().parent;P=O.parents[2]
shared=runpy.run_path(str(O.parent/'light_motion_v1/author.py'))
for name in ['bind','ib','parents','base','legs','turn','ik','keys','pack']:
    globals()[name]=shared[name]
N=42;HZ=30;FLOOR=-20000.

def feet_at(k,index):
    side,hip,knee,ankle,toe,relative,center,_=legs[index]
    target=center.copy();angle=0.;plant=True
    if index==0:
        amount=keys(k,[(0,0),(2,0),(8,1),(24,1),(37,0),(42,0)])
        target+=np.array([550,0,-3600])*amount;angle=9*amount
        for a,b,lift in [(2,8,2200),(24,37,1800)]:
            if a<k<b:
                target[1]+=lift*math.sin(math.pi*(k-a)/(b-a))**2;plant=False
    r=np.array(Matrix.Rotation(math.radians(angle),3,'Y'));foot=relative.copy()
    foot[:,:3,3]=np.einsum('ij,kj->ki',r,foot[:,:3,3])+target
    foot[:,:3,:3]=np.einsum('ij,kjl->kil',r,foot[:,:3,:3])
    return foot,plant

def body_at(k):
    g=base.copy()
    # Fast impact, short overshoot and a compressed vulnerable hold. The hips
    # catch the weight after the chest reacts, then lead the return to guard.
    shift=keys(k,[(0,0),(4,-1400),(9,-1800),(18,-1500),(28,-400),(39,0),(42,0)])
    depth=keys(k,[(0,0),(3,-1600),(7,-3100),(17,-2700),(27,-1200),(39,0),(42,0)])
    height=keys(k,[(0,0),(3,600),(9,-1800),(14,-2400),(20,-2200),(30,-600),(38,100),(42,0)])
    g[:,:3,3]+=np.array([shift,height,depth])
    hip=keys(k,[(0,0),(5,-8),(10,-12),(20,-10),(32,2),(42,0)])
    waist=keys(k,[(0,0),(3,-16),(7,-21),(18,-15),(30,3),(42,0)])
    chest=keys(k,[(0,0),(2,-12),(5,-17),(16,-10),(29,2),(42,0)])
    turn(g,0,hip,'Y',range(22));turn(g,1,waist,'Y',range(1,14));turn(g,2,chest,'Y',range(2,14))
    fold=keys(k,[(0,0),(3,-22),(6,-16),(12,22),(20,20),(30,4),(38,-2),(42,0)])
    roll=keys(k,[(0,0),(3,-13),(7,-17),(18,-11),(31,2),(42,0)])
    turn(g,2,fold,'X',range(2,14));turn(g,2,roll,'Z',range(2,14))
    # Head trails the chest's impulse by two frames, then refocuses first.
    head=keys(k,[(0,0),(2,3),(5,-14),(9,-9),(15,10),(22,7),(30,-2),(38,0),(42,0)])
    turn(g,4,head,'X',[4,5]);turn(g,4,-(hip+waist+chest)*.35,'Y',[4,5])
    # Reflexive bent-arm guard. Each rotation carries the whole child chain,
    # preserving the accepted claw-to-forearm attachment without wrist aiming.
    reflex=keys(k,[(0,0),(2,.08),(6,1),(12,.84),(21,.72),(29,.36),(39,0),(42,0)])
    for a,b,c,side,amount in [(7,8,9,1,1.),(11,12,13,-1,.65)]:
        turn(g,a,-34*reflex*amount,'X',[a,b,c])
        turn(g,a,side*14*reflex*amount,'Z',[a,b,c])
        u=g[b,:3,3]-g[a,:3,3];l=g[c,:3,3]-g[b,:3,3]
        axis=np.cross(u,l);axis/=np.linalg.norm(axis)
        r=np.array(Matrix.Rotation(math.radians(23*reflex*amount),3,Vector(axis)))
        pivot=g[b,:3,3].copy()
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
pack(O/'stun.psxanim',frames)
# Preserve the authored sample rate; Stun binding plays this clip at 1x.
data=bytearray((O/'stun.psxanim').read_bytes());struct.pack_into('<H',data,16,HZ);(O/'stun.psxanim').write_bytes(data)
np.savez(O/'stun.npz',skin=frames,bind=bind,parents=parents,contact=contacts)
meta={'stun':{'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'playback_speed_q8':256,'duration_seconds':N/HZ,'source_duration_seconds':N/HZ,'loop':False,'speed':0.,'pelvis_drop_model_units':drop,'max_leg_extension':max(extensions)}}
(O/'authoring.json').write_text(json.dumps(meta,indent=2));print('RESULT',json.dumps(meta))
