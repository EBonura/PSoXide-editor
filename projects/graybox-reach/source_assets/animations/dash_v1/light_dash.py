"""Light enemy (training enemy) knockdown candidates. Review only; writes to scratch.
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

DEBUG='--dbg' in sys.argv
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
        if DEBUG:print('FOOTDBG',index,roll,low,[len(FOOTV[a]) for a in FOOTV])
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

# Dash Left: the enemy's left is +X. The motor carries the body sideways; the clip is in place.
# 13 frames at 30 Hz = 26 ticks = roll_active_frames 14 + roll_recovery_frames 12 (its Character data).
def arc(k,a,b,h):return h*math.sin(math.pi*(k-a)/(b-a))**2 if a<k<b else 0.
def cand_A(k):
    """Low skid: a quick dip, the body throws itself low into the direction with the
    claws tucked, a short skip with both feet off the floor, the lead foot lands and
    skids, the trailing foot slides in under it, settle into guard."""
    N=13;c={}
    c['shift']=keys(k,[(0,0),(1,-300),(3,1800),(6,2200),(9,800),(12,-100),(13,0)])
    c['depth']=keys(k,[(0,0),(3,-300),(7,-200),(13,0)])
    c['height']=keys(k,[(0,0),(1,-900),(3,-1800),(5,-1200),(7,-2600),(9,-1600),(11,150),(13,0)])
    c['hip']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['waist']=keys(k,[(0,0),(3,-8),(8,-4),(13,0)])
    c['chest']=keys(k,[(0,0),(3,-4),(8,-2),(13,0)])
    c['fold']=keys(k,[(0,0),(1,4),(3,14),(7,16),(10,6),(13,0)])
    c['roll']=keys(k,[(0,0),(1,4),(3,-20),(6,-22),(9,-8),(11,2),(13,0)])
    c['head']=keys(k,[(0,0),(3,-8),(7,-6),(11,2),(13,0)])
    c['reflex']=keys(k,[(0,0),(2,.6),(4,.9),(8,.7),(12,0),(13,0)])
    c['legs']=[
        dict(dx=keys(k,[(0,0),(2,0),(6,1800),(9,1800),(12,0),(13,0)]),dz=0,lift=arc(k,2,6,1900)+arc(k,9,12,700),yaw=keys(k,[(0,0),(6,10),(10,4),(13,0)]),roll=0),
        dict(dx=keys(k,[(0,0),(3,0),(6,900),(9,1400),(12,0),(13,0)]),dz=0,lift=arc(k,3,7,2300)+arc(k,9,12,600),yaw=0,roll=0)]
    return N,c
def cand_B(k):
    """Spring hop: a crouch, a springing sideways hop that clears both feet with the
    body upright and the claws flared for balance, a compressed landing, guard."""
    N=13;c={}
    c['shift']=keys(k,[(0,0),(2,-500),(5,1600),(8,1400),(11,200),(13,0)])
    c['depth']=keys(k,[(0,0),(2,300),(6,-400),(13,0)])
    c['height']=keys(k,[(0,0),(2,-2000),(4,600),(5,900),(7,-400),(8,-2400),(10,-900),(12,120),(13,0)])
    c['hip']=keys(k,[(0,0),(4,4),(8,2),(13,0)])
    c['waist']=keys(k,[(0,0),(4,6),(8,2),(13,0)])
    c['chest']=keys(k,[(0,0),(4,3),(13,0)])
    c['fold']=keys(k,[(0,0),(2,12),(4,-6),(6,-4),(8,14),(10,6),(13,0)])
    c['roll']=keys(k,[(0,0),(2,6),(4,-12),(6,-8),(8,6),(10,-2),(13,0)])
    c['head']=keys(k,[(0,0),(2,-6),(5,4),(8,-6),(13,0)])
    c['reflex']=keys(k,[(0,0),(2,.3),(4,1),(7,.8),(10,.2),(13,0)])
    c['legs']=[
        dict(dx=keys(k,[(0,0),(2,0),(5,900),(8,900),(11,0),(13,0)]),dz=0,lift=arc(k,2,8,3200)+arc(k,8,11,500),yaw=0,roll=0),
        dict(dx=keys(k,[(0,0),(2,0),(5,600),(8,600),(11,0),(13,0)]),dz=0,lift=arc(k,2,8.5,3000)+arc(k,9,12,500),yaw=0,roll=0)]
    return N,c
CANDS={'light_A_low_skid':cand_A,'light_B_spring_hop':cand_B}

sys.path.insert(0,str(Path(__file__).resolve().parent));from dashtravel import travel,LIGHT_TRAVEL
def TR(k):return travel(k,LIGHT_TRAVEL)
def cand_A2(k):
    """Low skid v2: the motor launches at tick 0, so both feet leave the floor at once; the
    body flies low with the claws tucked, the lead foot lands at frame 6 and is pinned in the
    world for the rest of the travel, the trailing foot lands under the body at frame 8."""
    N=13;c={};T7=TR(7)
    c['shift']=keys(k,[(0,0),(1,1200),(3,2200),(6,1600),(8,600),(11,-100),(13,0)])
    c['depth']=keys(k,[(0,0),(3,-300),(7,-200),(13,0)])
    c['height']=keys(k,[(0,0),(1,-1200),(3,-1600),(5,-1800),(6,-2600),(8,-2400),(10,-900),(12,120),(13,0)])
    c['hip']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['waist']=keys(k,[(0,0),(3,-8),(8,-4),(13,0)])
    c['chest']=keys(k,[(0,0),(3,-4),(8,-2),(13,0)])
    c['fold']=keys(k,[(0,0),(1,8),(4,14),(7,12),(10,4),(13,0)])
    c['roll']=keys(k,[(0,0),(1,-14),(3,-22),(6,-18),(8,-8),(11,2),(13,0)])
    c['head']=keys(k,[(0,0),(3,-8),(7,-6),(11,2),(13,0)])
    c['reflex']=keys(k,[(0,0),(1,.7),(4,.9),(8,.6),(12,0),(13,0)])
    if k<=0:ldx=0.
    elif k<6:ldx=-2500*math.sin(math.pi*k/6)+(T7-TR(6))*sh["smooth"](k/6)
    else:ldx=T7-TR(k)
    if 0<k<8:rdx=-3000*math.sin(math.pi*k/8)
    else:rdx=0.
    c['legs']=[dict(dx=ldx,dz=0,lift=arc(k,0,6,2400),yaw=0,roll=0),dict(dx=rdx,dz=0,lift=arc(k,0,8,2000),yaw=0,roll=0)]
    return N,c
CANDS={'light_A2_low_skid_v2':cand_A2}

def cand_A3(k):
    """Low bound v3: frame 0 is the guard; on frame 1 the motor is already moving, so the
    trailing foot pushes off from where it stood (toe leaving the ground, leg stretched back)
    while the lead foot is already up; both feet travel in the air, finish their sideways move
    before they come down, the lead foot lands at frame 6 and is pinned in the world for the
    rest of the travel, the trailing foot comes straight down under the body at frame 8."""
    N=13;c={};T7=TR(7)
    c['shift']=keys(k,[(0,0),(1,-1100),(3,2200),(6,1600),(8,600),(11,-100),(13,0)])
    c['depth']=keys(k,[(0,0),(3,-300),(7,-200),(13,0)])
    c['height']=keys(k,[(0,0),(1,-1600),(3,-1700),(5,-1800),(6,-2500),(8,-2300),(10,-900),(12,120),(13,0)])
    c['hip']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['waist']=keys(k,[(0,0),(3,-8),(8,-4),(13,0)])
    c['chest']=keys(k,[(0,0),(3,-4),(8,-2),(13,0)])
    c['fold']=keys(k,[(0,0),(1,8),(4,14),(7,12),(10,4),(13,0)])
    c['roll']=keys(k,[(0,0),(1,-14),(3,-22),(6,-18),(8,-8),(11,2),(13,0)])
    c['head']=keys(k,[(0,0),(3,-8),(7,-6),(11,2),(13,0)])
    c['reflex']=keys(k,[(0,0),(1,.7),(4,.9),(8,.6),(12,0),(13,0)])
    lwx=keys(k,[(0,0),(5,T7),(13,T7)]);rwx=keys(k,[(0,0),(1,0),(6,T7),(13,T7)])
    llift=keys(k,[(0,0),(1,1800),(3,2400),(5,700),(6,0),(13,0)])
    rlift=keys(k,[(0,0),(1,250),(2,1600),(4,2200),(6,900),(7,250),(8,0),(13,0)])
    c['legs']=[dict(dx=lwx-TR(k),dz=0,lift=llift,yaw=0,roll=0),dict(dx=rwx-TR(k),dz=0,lift=rlift,yaw=0,roll=0)]
    return N,c

from dashtravel import travel as _trv
SLOW_ACTIVE=26
def TRS(k):return _trv(k,27563,SLOW_ACTIVE)
def WARP(k):
    """New frame -> v2 key time: the v2 active part (frames 0-7) stretched over 13 frames,
    its recovery (7-13) over the remaining 8."""
    return k*7/13 if k<=13 else 7+(k-13)*6/8
def cand_A4(k):
    """Low bound v3: frame 0 is the guard; on frame 1 the motor is already moving, so the
    trailing foot pushes off from where it stood (toe leaving the ground, leg stretched back)
    while the lead foot is already up; both feet travel in the air, finish their sideways move
    before they come down, the lead foot lands at frame 6 and is pinned in the world for the
    rest of the travel, the trailing foot comes straight down under the body at frame 8."""
    N=21;c={};T7=TRS(21);kk=k;k=WARP(kk)
    c['shift']=keys(k,[(0,0),(1,-1100),(3,2200),(6,1600),(8,600),(11,-100),(13,0)])
    c['depth']=keys(k,[(0,0),(3,-300),(7,-200),(13,0)])
    c['height']=keys(k,[(0,0),(1,-1600),(3,-1700),(5,-1800),(6,-2500),(8,-2300),(10,-900),(12,120),(13,0)])
    c['hip']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['waist']=keys(k,[(0,0),(3,-8),(8,-4),(13,0)])
    c['chest']=keys(k,[(0,0),(3,-4),(8,-2),(13,0)])
    c['fold']=keys(k,[(0,0),(1,8),(4,14),(7,12),(10,4),(13,0)])
    c['roll']=keys(k,[(0,0),(1,-14),(3,-22),(6,-18),(8,-8),(11,2),(13,0)])
    c['head']=keys(k,[(0,0),(3,-8),(7,-6),(11,2),(13,0)])
    c['reflex']=keys(k,[(0,0),(1,.7),(4,.9),(8,.6),(12,0),(13,0)])
    lwx=keys(k,[(0,0),(5,T7),(13,T7)]);rwx=keys(k,[(0,0),(.6,0),(5.4,T7),(13,T7)])
    llift=keys(k,[(0,0),(1,1800),(3,2400),(5,700),(6,0),(13,0)])
    rlift=keys(k,[(0,0),(.6,0),(1.1,900),(2,2400),(3.5,3600),(5.4,1800),(6.1,0),(13,0)])
    c['legs']=[dict(dx=lwx-TRS(kk),dz=0,lift=llift,yaw=0,roll=0),dict(dx=rwx-TRS(kk),dz=0,lift=rlift,yaw=0,roll=0)]
    return N,c
CANDS={'light_A4_low_bound_slow':cand_A4}

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
TIPS={}
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


def cand_A4R(k):
    """Dash Right: cand_A4 mirrored per limb: the -X leg leads, the +X leg trails and pushes off,
    sideways body channels and travel flip sign."""
    N,c=cand_A4(k)
    for ch in ['shift','roll','hip','waist','chest']:c[ch]=-c[ch]
    lead,trail=c['legs']
    c['legs']=[dict(trail,dx=-trail['dx']),dict(lead,dx=-lead['dx'])]
    return N,c
CANDS={'light_A4_low_bound_slow':cand_A4,'light_A4R_low_bound_slow_right':cand_A4R}

want=sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else []
FACES=np.array([[c[0] for c in f] for f in mesh_m['f']],np.int32)
for label,fn in CANDS.items():
    if want and not any(label.startswith(w) for w in want):continue
    N=fn(0)[0];frames=[];V=[];lows=[];ext=[]
    for k in range(N+1):
        _,c,g=body_at(fn,k)
        for i,(side,h,n_,a,t,_,_,_) in enumerate(legs):
            L=c['legs'][i];foot=foot_pose(i,L['dx'],L['dz'],L['lift'],L['yaw'],L['roll'])
            ext.append(ik(g,h,n_,a,foot[0,:3,3],np.array([side*.1,-.12,1.])));g[a],g[t]=foot
        skin=g@ib;frames.append(skin);v=deform(mesh_m,[(m[:3,:3],m[:3,3]) for m in skin]);V.append(v);lows.append(float(v[:,1].min()-FLOOR))
    frames.append(frames[-1].copy());V.append(V[-1]);frames=np.array(frames)
    pack(OUT/f'{label}.psxanim',frames)
    np.savez(OUT/f'{label}.npz',verts=np.array(V,np.float32),faces=FACES,hz=HZ,skin=frames)
    print('RESULT',label,json.dumps({'frames':N,'stored':len(frames),'max_leg_ext':max(ext),'min_above_floor':min(lows),'end_vs_start':float(np.abs(frames[-1]-frames[0]).max())}),flush=True)
