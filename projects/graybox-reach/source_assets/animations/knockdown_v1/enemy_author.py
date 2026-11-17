"""Light enemy (training enemy) knockdown candidates. Shipped: C_knee_buckle (assets/animations/knockdown_v1/light_knockdown.psxanim). Writes next to this script.
Same construction as light_reaction_v1/stun: keyed body channels over the
walk-v5 guard pose (the stun's start and end pose), planted-foot leg IK, 30 Hz.
Args after --: [candidate prefixes]."""
import runpy,sys,math,json,struct
sys.dont_write_bytecode=True
from pathlib import Path
import numpy as np
from mathutils import Matrix,Vector
OUT=Path(__file__).resolve().parent
G=OUT.parent
sh=runpy.run_path(str(G/'light_motion_v1/author.py'))
bind,ib,parents,base,legs,turn,ik,keys,pack=[sh[k] for k in ['bind','ib','parents','base','legs','turn','ik','keys','pack']]
mesh_m=sh['mesh'];deform=sh['deform']
HZ=30;FLOOR=-20000.
owner=np.array([mesh_m['owner'][i] for i in range(len(mesh_m['v']))])
TIP=np.array([-443.90856805,10211.15579709,844.17601394])  # claw tip in hand frame (light_motion_v1)

FOOTV={j:[i for i in range(len(mesh_m['v'])) if mesh_m['owner'][i]==j] for j in [16,17,20,21]}
def vkeys(t,pts):
    a,x=pts[0];b,y=pts[1]
    if a<t<=b:u=(t-a)/(b-a);u=1-(1-u)**2;return x+(y-x)*u
    return keys(t,pts)
def breathe(k,a,b,amp):
    if a<k<b:return amp*math.sin(math.pi*(k-a)/(b-a))*math.sin(2*math.pi*(k-a)/9.)
    return 0.

def foot_pose(index,dx,dz,lift,yaw,roll):
    """Foot transforms: planted sole centre offset (dx,dz), lift, yaw about Y,
    and a heel roll about the toe (positive raises the heel)."""
    side,hip,knee,ankle,toe,relative,center,_=legs[index]
    foot=relative.copy();target=center.copy()+np.array([dx,lift,dz])
    ry=np.array(Matrix.Rotation(math.radians(yaw),3,'Y'))
    foot[:,:3,3]=np.einsum('ij,kj->ki',ry,foot[:,:3,3]);foot[:,:3,:3]=np.einsum('ij,kjl->kil',ry,foot[:,:3,:3])
    if roll:
        pivot=foot[1,:3,3].copy();rx=np.array(Matrix.Rotation(math.radians(roll),3,ry@np.array([1.,0,0])))
        foot[0,:3,3]=pivot+rx@(foot[0,:3,3]-pivot);foot[0,:3,:3]=rx@foot[0,:3,:3]
    foot[:,:3,3]+=target
    if roll:
        # Keep the rolled sole on the floor: lift by however far its mesh sinks.
        low=1e9
        for jj,fm in zip([ankle,toe],foot):
            sk=fm@ib[jj];vv=np.array([mesh_m['v'][i][:3] for i in FOOTV[jj]],float)
            if len(vv):low=min(low,(vv@sk[:3,:3].T+sk[:3,3])[:,1].min())
        if low<FLOOR+lift:foot[:,1,3]+=FLOOR+lift-low
    return foot

def arm_ik(g,a,b,c,target,pole):
    rel=np.linalg.inv(g[b,:3,:3])@g[c,:3,:3]
    e=ik(g,a,b,c,target,pole);g[c,:3,:3]=g[b,:3,:3]@rel;return e

def plant_claw(g,a,b,c,tip_target,pole,w):
    """Move the hand so the claw tip lands on tip_target (blended by w)."""
    if w<=0:return 0.
    for _ in range(3):
        tip=g[c,:3,3]+g[c,:3,:3]@TIP
        goal=g[c,:3,3]+(tip_target-tip)*w
        h=g[a,:3,3];L=np.linalg.norm(g[b,:3,3]-h)+np.linalg.norm(g[c,:3,3]-g[b,:3,3]);v=goal-h;d=np.linalg.norm(v)
        if d>.97*L:goal=h+v/d*.97*L
        e=arm_ik(g,a,b,c,goal,pole)
    return e

# --------------------------------------------------------------------------
# Model space: +Y up, +Z forward (toward the player), floor y=-20000.
# Leg 0 (joints 14-17, +X) is the leading leg, leg 1 (18-21) the rear.
def cand_A(k):
    """Claw to the floor: heavier recoil, one failed catch step back, the body
    drops low and pitches forward and the free claw stabs into the floor as a
    prop; head hangs; pushes off the claw and recovers its guard."""
    N=64;c={}
    c['shift']=vkeys(k,[(0,0),(3,-1500),(8,-2100),(16,-900),(40,-800),(50,-300),(60,0),(64,0)])
    c['depth']=vkeys(k,[(0,0),(3,-2400),(7,-4200),(12,-3600),(18,-1800),(40,-1700),(50,-1200),(58,-200),(64,0)])
    c['height']=keys(k,[(0,0),(3,700),(8,-1800),(13,-5200),(17,-6600),(20,-6400),(40,-6300),(46,-5600),(52,-3000),(58,-500),(61,150),(64,0)])+breathe(k,20,42,250)
    c['hip']=vkeys(k,[(0,0),(4,-10),(9,-14),(18,-6),(40,-6),(52,2),(64,0)])
    c['waist']=vkeys(k,[(0,0),(3,-20),(7,-26),(14,-8),(40,-6),(52,3),(64,0)])
    c['chest']=vkeys(k,[(0,0),(2,-14),(5,-20),(14,-6),(40,-5),(52,2),(64,0)])
    c['fold']=vkeys(k,[(0,0),(3,-28),(6,-22),(11,14),(16,34),(20,38),(40,36),(46,30),(52,16),(58,2),(64,0)])+breathe(k,20,42,2)
    c['roll']=vkeys(k,[(0,0),(3,-16),(8,-18),(16,10),(22,14),(40,13),(52,3),(64,0)])
    c['head']=keys(k,[(0,0),(2,4),(5,-18),(9,-12),(16,16),(24,28),(38,26),(44,8),(50,-6),(58,-2),(64,0)])
    c['legs']=[
        dict(dx=keys(k,[(0,0),(2,0),(8,550),(52,550),(60,0),(64,0)]),dz=keys(k,[(0,0),(2,0),(8,-3800),(52,-3800),(60,0),(64,0)]),
             lift=sum(L*math.sin(math.pi*(k-a)/(b-a))**2 for a,b,L in [(2,8,2400),(52,60,1800)] if a<k<b),yaw=keys(k,[(0,0),(8,10),(52,10),(60,0)]),roll=0),
        dict(dx=0,dz=0,lift=0,yaw=0,roll=keys(k,[(0,0),(10,0),(16,22),(46,22),(54,0),(64,0)]))]
    c['reflex']=keys(k,[(0,0),(2,.1),(6,1),(11,.6),(16,.2),(46,.2),(56,0),(64,0)])
    return N,c

def cand_B(k):
    """Doubled over: the hit lands in the gut. It folds forward at once, both
    claws curl in, two stumbling steps back, deep sag on bent knees, one failed
    attempt to straighten, then the guard comes back."""
    N=60;c={}
    c['shift']=keys(k,[(0,0),(6,500),(12,-500),(20,0),(60,0)])
    c['depth']=vkeys(k,[(0,0),(2,-1500),(6,-3200),(11,-4800),(16,-5600),(36,-5500),(44,-5000),(50,-2800),(56,-400),(60,0)])
    c['height']=keys(k,[(0,0),(2,-500),(6,-1600),(11,-2800),(16,-4200),(20,-4600),(34,-4500),(38,-3300),(41,-3900),(46,-3000),(52,-1400),(57,100),(60,0)])+breathe(k,18,36,220)
    c['hip']=vkeys(k,[(0,0),(3,6),(10,-4),(16,3),(40,3),(52,-1),(60,0)])
    c['waist']=vkeys(k,[(0,0),(3,8),(10,-6),(16,4),(40,4),(60,0)])
    c['chest']=vkeys(k,[(0,0),(3,6),(10,-4),(16,3),(40,3),(60,0)])
    c['fold']=vkeys(k,[(0,0),(2,20),(5,40),(9,38),(14,46),(20,50),(34,48),(38,34),(41,40),(47,28),(53,10),(58,-1),(60,0)])+breathe(k,18,36,2.5)
    c['roll']=keys(k,[(0,0),(4,6),(9,-6),(14,4),(36,4),(50,-1),(60,0)])
    c['head']=keys(k,[(0,0),(1,-14),(3,-20),(7,6),(12,18),(20,24),(34,24),(38,6),(41,12),(48,-4),(56,1),(60,0)])
    c['legs']=[
        dict(dx=0,dz=keys(k,[(0,0),(2,0),(7,-3000),(46,-3000),(53,0),(60,0)]),
             lift=sum(L*math.sin(math.pi*(k-a)/(b-a))**2 for a,b,L in [(2,7,2000),(46,53,1800)] if a<k<b),yaw=0,roll=0),
        dict(dx=0,dz=keys(k,[(0,0),(7,0),(12,-2400),(48,-2400),(55,0),(60,0)]),
             lift=sum(L*math.sin(math.pi*(k-a)/(b-a))**2 for a,b,L in [(7,12,1600),(48,55,1400)] if a<k<b),yaw=0,roll=0)]
    c['curl']=keys(k,[(0,0),(2,.7),(5,1),(36,1),(42,.8),(52,.2),(58,0),(60,0)])
    return N,c

def cand_C(k):
    """Knee buckle: blown sideways and back, the rear knee gives and drops to
    the floor while the body lists over it, claws flung wide then hanging; a
    long helpless beat; it plants the lead foot and drives back up."""
    N=66;c={}
    c['shift']=vkeys(k,[(0,0),(3,-2200),(8,-3400),(14,-3000),(20,-2600),(44,-2500),(52,-1500),(60,-200),(66,0)])
    c['depth']=vkeys(k,[(0,0),(3,-2000),(8,-3400),(14,-3200),(20,-2800),(44,-2700),(52,-1800),(60,-300),(66,0)])
    c['height']=keys(k,[(0,0),(3,600),(8,-1500),(13,-7000),(17,-10400),(20,-10200),(44,-10100),(50,-8400),(56,-3200),(61,-400),(64,120),(66,0)])+breathe(k,20,46,250)
    c['hip']=vkeys(k,[(0,0),(4,14),(9,18),(18,10),(44,9),(56,2),(66,0)])
    c['waist']=vkeys(k,[(0,0),(3,22),(7,26),(14,12),(44,10),(56,2),(66,0)])
    c['chest']=vkeys(k,[(0,0),(2,16),(5,20),(14,8),(44,7),(56,1),(66,0)])
    c['fold']=vkeys(k,[(0,0),(3,-26),(6,-20),(12,6),(18,22),(26,26),(44,24),(50,18),(57,6),(63,0),(66,0)])+breathe(k,20,46,2)
    c['roll']=vkeys(k,[(0,0),(3,20),(7,26),(14,22),(20,24),(44,22),(52,12),(60,2),(66,0)])
    c['head']=keys(k,[(0,0),(2,4),(5,-16),(9,-10),(16,12),(24,22),(40,20),(48,4),(56,-6),(62,-1),(66,0)])
    c['legs']=[
        dict(dx=keys(k,[(0,0),(46,0),(54,-400),(66,0)]),dz=keys(k,[(0,0),(46,0),(52,-1200),(60,0),(66,0)]),
             lift=sum(L*math.sin(math.pi*(k-a)/(b-a))**2 for a,b,L in [(46,60,1600)] if a<k<b),yaw=0,roll=0),
        dict(dx=keys(k,[(0,0),(3,0),(9,-900),(54,-900),(62,0),(66,0)]),dz=keys(k,[(0,0),(3,0),(9,-1600),(54,-1600),(62,0),(66,0)]),
             lift=sum(L*math.sin(math.pi*(k-a)/(b-a))**2 for a,b,L in [(3,9,1800),(54,62,1500)] if a<k<b),
             yaw=keys(k,[(0,0),(9,-12),(54,-12),(62,0)]),roll=keys(k,[(0,0),(10,0),(18,62),(46,62),(56,10),(62,0),(66,0)]))]
    c['reflex']=keys(k,[(0,0),(2,.1),(6,1),(12,.7),(20,.1),(66,0)])
    c['hang']=keys(k,[(0,0),(10,0),(18,1),(46,1),(58,0),(66,0)])
    return N,c

CANDS={'A_claw_down':cand_A,'B_doubled_over':cand_B,'C_knee_buckle':cand_C}

TIPS={}
def floor_guard(g,margin=120.):
    """Claws that would go through the floor swing forward about the elbow
    (forearm and claw as one piece, wrist untouched) until they rest on it."""
    for a,b,cc,side in [(7,8,9,1),(11,12,13,-1)]:
        sel=np.array([mesh_m['v'][i][:3] for i in range(len(mesh_m['v'])) if owner[i] in (b,cc)],float);own=[owner[i] for i in range(len(owner)) if owner[i] in (b,cc)]
        def low():
            sk=g@ib;return min((sel[i]@sk[o][:3,:3].T+sk[o][:3,3])[1] for i,o in enumerate(own))
        lo=low();turned=0.
        while lo<FLOOR+margin and turned<80:
            turn(g,b,-4.,'X',[b,cc]);turned+=4;lo=low()
        if turned:
            # bisect back so the claw just touches (continuous across frames)
            step=2.
            for _ in range(7):
                turn(g,b,step,'X',[b,cc]);lo=low()
                if lo<FLOOR+margin:turn(g,b,-step,'X',[b,cc])
                step/=2
def body_at(fn,k,plants=True):
    N,c=fn(k);g=base.copy()
    g[:,:3,3]+=np.array([c['shift'],c['height'],c['depth']])
    turn(g,0,c['hip'],'Y',range(22));turn(g,1,c['waist'],'Y',range(1,14));turn(g,2,c['chest'],'Y',range(2,14))
    turn(g,2,c['fold'],'X',range(2,14));turn(g,2,c['roll'],'Z',range(2,14))
    turn(g,4,c['head'],'X',[4,5]);turn(g,4,-(c['hip']+c['waist']+c['chest'])*.35,'Y',[4,5])
    r=c.get('reflex',0.)
    for a,b,cc,side,amount in [(7,8,9,1,1.),(11,12,13,-1,.65)]:
        if r:
            turn(g,a,-34*r*amount,'X',[a,b,cc]);turn(g,a,side*20*r*amount,'Z',[a,b,cc])
    if c.get('curl'):
        u=c['curl']
        for a,b,cc,side in [(7,8,9,1),(11,12,13,-1)]:
            turn(g,a,-c['fold']*.95*u,'X',[a,b,cc]);turn(g,a,-side*10*u,'Z',[a,b,cc])
            up=g[b,:3,3]-g[a,:3,3];lo=g[cc,:3,3]-g[b,:3,3];axis=np.cross(up,lo);axis/=np.linalg.norm(axis)
            rr=np.array(Matrix.Rotation(math.radians(18*u),3,Vector(axis)));p=g[b,:3,3].copy()
            for jj in [b,cc]:g[jj,:3,:3]=rr@g[jj,:3,:3];g[jj,:3,3]=p+rr@(g[jj,:3,3]-p)
    if c.get('hang'):
        u=c['hang']
        for a,b,cc,side in [(7,8,9,1),(11,12,13,-1)]:
            turn(g,a,-(c['fold']*.7+8)*u,'X',[a,b,cc]);turn(g,a,(-c['roll']*.9+side*6)*u,'Z',[a,b,cc])
    for (a,b,cc,side),w,ref in (c.get('plants',[]) if plants else []):
        if w<=0:continue
        key=(fn.__name__,a)
        if key not in TIPS:
            _,_,gr=body_at(fn,ref,False);tip=gr[cc,:3,3]+gr[cc,:3,:3]@TIP;tip[1]=FLOOR+150;TIPS[key]=tip
        plant_claw(g,a,b,cc,TIPS[key],np.array([side*.9,.3,-.2]),w)
    if plants:floor_guard(g)
    return N,c,g

want=sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else []
report={}
for label,fn in CANDS.items():
    if want and not any(label.startswith(w) for w in want):continue
    N=fn(0)[0];drop=0.
    for k in np.arange(0,N+.01,.25):
        _,c,g=body_at(fn,k)
        for i,(_,h,_,_,_,_,_,lengths) in enumerate(legs):
            L=c['legs'][i];foot=foot_pose(i,L['dx'],L['dz'],L['lift'],L['yaw'],L['roll']);hp,fp=g[h,:3,3],foot[0,:3,3]
            horizontal=np.sum((hp[[0,2]]-fp[[0,2]])**2);limit=.975*sum(lengths)
            assert horizontal<limit**2,(label,k,i)
            drop=max(drop,hp[1]-fp[1]-math.sqrt(limit**2-horizontal))
    frames=[];contacts=[];ext=[];lows=[]
    for k in range(N+1):
        _,c,g=body_at(fn,k);g[:,1,3]-=drop
        for i,(side,h,n_,a,t,_,_,_) in enumerate(legs):
            L=c['legs'][i];foot=foot_pose(i,L['dx'],L['dz'],L['lift'],L['yaw'],L['roll'])
            ext.append(ik(g,h,n_,a,foot[0,:3,3],np.array([side*.1,-.12,1.])));g[a],g[t]=foot
        skin=g@ib;frames.append(skin)
        v=deform(mesh_m,[(m[:3,:3],m[:3,3]) for m in skin])
        lows.append({'k':k,'all':float(v[:,1].min()-FLOOR),'claw_free':float(v[owner==13,1].min()-FLOOR),'claw_main':float(v[owner==9,1].min()-FLOOR),
                     'knee_rear':float(v[owner==19,1].min()-FLOOR),'knee_lead':float(v[owner==15,1].min()-FLOOR),'low_owner':int(owner[v[:,1].argmin()]),'knee_joint_rear':float(g[19,1,3]-FLOOR)})
    frames.append(frames[-1].copy());frames=np.array(frames)
    pack(OUT/f'{label}.psxanim',frames)
    np.savez(OUT/f'{label}.npz',skin=frames,bind=bind,parents=parents)
    (OUT/f'{label}_contacts.json').write_text(json.dumps(lows))
    report[label]={'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'seconds':N/HZ,'pelvis_drop':drop,'max_leg_extension':max(ext),
        'min_vertex_above_floor':min(x['all'] for x in lows),'min_free_claw':min(x['claw_free'] for x in lows),'min_rear_knee':min(x['knee_rear'] for x in lows)}
    print('RESULT',label,json.dumps(report[label]),flush=True)
(OUT/'author_report.json').write_text(json.dumps(report,indent=2))
