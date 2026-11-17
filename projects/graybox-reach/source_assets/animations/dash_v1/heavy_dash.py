"""Heavy enemy knockdown candidates (review only). Keyed body channels over the
pose the Heavy's Stun + Recovery starts and ends on, in the cooked clip's own
skin space: chains turn about joint pivots solved from the shipped clips,
two-bone leg IK onto planted (or stepped, heel-rolled) feet, and a claw floor
guard. 30 Hz. Plain python3 + numpy. Args: [candidate prefixes]."""
import sys,math,json,struct
sys.dont_write_bytecode=True;sys.path.insert(0,sys.path[0]);sys.path.insert(0,sys.path[0]+'/../knockdown_v1')
from psxdecode import model,animation,deform
import numpy as np
from pathlib import Path
OUT=Path(__file__).resolve().parent
P=OUT.parents[3]
sys.path.insert(0,str(OUT.parent/'knockdown_v1'))
A=P/'default/assets/animations/tank_boss_ai'
M=model(P/'grid-dungeon/assets/models/enemy_reduced/heavy.psxmdl')
HZ=30
PARENTS=[None,0,1,2,3,4,5,6,5,8,5,10,2,12,13,14,15,0,17,18,19,20,0,22,23,24,25]
NJ=27
def desc(j):
    out=[j];ch=True
    while ch:
        ch=False
        for k,p in enumerate(PARENTS):
            if p in out and k not in out:out.append(k);ch=True
    return out
DESC={j:desc(j) for j in range(NJ)}
def to4(R,t):
    m=np.eye(4);m[:3,:3]=R;m[:3,3]=t;return m
def clip(name):return np.array([[to4(R,t) for R,t in f] for f in animation(A/f'{name}.psxanim')])
idle,stun,hit=clip('idle'),clip('stun_recovery'),clip('hit_react')
# Joint pivots in bind space: the point a joint and its parent move identically.
PIV={}
allf=np.concatenate([idle,stun,hit])
for j,p in enumerate(PARENTS):
    if p is None:continue
    Am=np.vstack([f[j,:3,:3]-f[p,:3,:3] for f in allf]);bm=np.hstack([f[p,:3,3]-f[j,:3,3] for f in allf])
    x,_,rk,_=np.linalg.lstsq(Am,bm,rcond=None)
    if rk==3:PIV[j]=x
BASE=(clip('walk_rgt')[0] if 'strafeR' in sys.argv else clip('walk_lft')[0] if 'strafe' in ' '.join(sys.argv) else stun[0]).copy()
OWN=np.array([M['owner'][i] for i in range(len(M['v']))])
V0=np.array([v[:3] for v in M['v']],float)
def verts(S):return deform(M,[(m[:3,:3],m[:3,3]) for m in S])
FLOOR=float(verts(BASE)[:,1].min())
def piv(S,j):return S[j,:3,:3]@PIV[j]+S[j,:3,3]
def rot(axis,deg):
    a=np.asarray(axis,float);a/=np.linalg.norm(a);x,y,z=a;c=math.cos(math.radians(deg));s=math.sin(math.radians(deg));C=1-c
    return np.array([[c+x*x*C,x*y*C-z*s,x*z*C+y*s],[y*x*C+z*s,c+y*y*C,y*z*C-x*s],[z*x*C-y*s,z*y*C+x*s,c+z*z*C]])
AX={'X':(1,0,0),'Y':(0,1,0),'Z':(0,0,1)}
def turn(S,ids,R,p):
    T=np.eye(4);T[:3,:3]=R;T[:3,3]=p-R@p
    for k in ids:S[k]=T@S[k]
def turnj(S,j,deg,axis,ids=None):
    if deg:turn(S,DESC[j] if ids is None else ids,rot(AX[axis] if isinstance(axis,str) else axis,deg),piv(S,j))
def align(u,v):
    u=u/np.linalg.norm(u);v=v/np.linalg.norm(v);c=np.cross(u,v);s=np.linalg.norm(c);d=float(np.dot(u,v))
    if s<1e-9:return np.eye(3)
    return rot(c/s,math.degrees(math.atan2(s,d)))
LEGS={'L':(17,18,19,20),'R':(22,23,24,25)}
def leg_ik(S,side,target_feet):
    h,k,a,t=LEGS[side];pole=POLE[side];H=piv(S,h);K=piv(S,k);Af=target_feet[a][:3,:3]@PIV[a]+target_feet[a][:3,3]
    l1=np.linalg.norm(piv(BASE,k)-piv(BASE,h));l2=np.linalg.norm(piv(BASE,a)-piv(BASE,k))
    v=Af-H;d=np.linalg.norm(v);reach=d/(l1+l2);assert reach<.999,(side,reach)
    ax=v/d;pl=pole-ax*np.dot(pole,ax);pl/=np.linalg.norm(pl);along=(l1*l1-l2*l2+d*d)/(2*d);Kn=H+ax*along+pl*math.sqrt(max(0,l1*l1-along*along))
    turn(S,DESC[h],align(K-H,Kn-H),H)
    K2=piv(S,k);A2=piv(S,a);turn(S,DESC[k],align(A2-K2,Af-K2),K2)
    for j in DESC[a]:S[j]=target_feet[j]
    return reach
def _pole(s):
    h,k,a,_=LEGS[s];H,K,Aa=piv(BASE,h),piv(BASE,k),piv(BASE,a);ax=(Aa-H)/np.linalg.norm(Aa-H);v=K-H;v=v-ax*np.dot(v,ax);return v/np.linalg.norm(v)
POLE={s:_pole(s) for s in 'LR'}
LEN={s:np.linalg.norm(piv(BASE,LEGS[s][1])-piv(BASE,LEGS[s][0]))+np.linalg.norm(piv(BASE,LEGS[s][2])-piv(BASE,LEGS[s][1])) for s in 'LR'}
LIM={s:max(.975,np.linalg.norm(piv(BASE,LEGS[s][2])-piv(BASE,LEGS[s][0]))/LEN[s]+.002) for s in 'LR'}
FOOTV={s:np.where(np.isin(OWN,DESC[LEGS[s][2]]))[0] for s in 'LR'}
def foot_targets(side,dx,dz,lift,roll,yaw=0.):
    a=LEGS[side][2];ids=DESC[a];F={j:BASE[j].copy() for j in ids}
    vb=verts(BASE)[FOOTV[side]]
    toe=vb[vb[:,2]>np.percentile(vb[:,2],80)];pivot=np.array([toe[:,0].mean(),toe[:,1].min(),toe[:,2].mean()])
    if roll:
        T=np.eye(4);R=rot((1,0,0),roll);T[:3,:3]=R;T[:3,3]=pivot-R@pivot
        for j in ids:F[j]=T@F[j]
    if yaw:
        c=vb.mean(0);T=np.eye(4);R=rot((0,1,0),yaw);T[:3,:3]=R;T[:3,3]=c-R@c
        for j in ids:F[j]=T@F[j]
    for j in ids:F[j][:3,3]+=np.array([dx,lift,dz])
    # keep the sole on the floor
    S=BASE.copy()
    for j in ids:S[j]=F[j]
    low=verts(S)[FOOTV[side],1].min()
    if low<FLOOR+lift:
        for j in ids:F[j][1,3]+=FLOOR+lift-low
    return F
ARMS={'L':(4,5),'R':(13,14)}
CLAW_MARGIN=1500.
ARMV={s:np.where(np.isin(OWN,DESC[ARMS[s][1]]))[0] for s in 'LR'}
def claw_guard(S,margin=150.):
    margin=CLAW_MARGIN
    """A claw that would go through the floor swings about the elbow by the
    smallest angle (either way) that rests it on the floor."""
    for s,(up,fo) in ARMS.items():
        def low(T):return verts(T)[ARMV[s],1].min()
        margin=min(CLAW_MARGIN,BASE_CLAW[s]-FLOOR-1)
        if low(S)>=FLOOR+margin:continue
        best=None
        for sgn in (-1,):
            for mag in range(4,164,4):
                T=S.copy();turnj(T,fo,sgn*mag,'X')
                if low(T)>=FLOOR+margin:best=(sgn,mag);break
            if best:break
        if not best:continue
        sgn,mag=best;lo_a,hi_a=mag-4,mag
        for _ in range(7):
            mid=(lo_a+hi_a)/2;T=S.copy();turnj(T,fo,sgn*mid,'X')
            if low(T)>=FLOOR+margin:hi_a=mid
            else:lo_a=mid
        turnj(S,fo,sgn*hi_a,'X')
BASE_CLAW={s:verts(BASE)[ARMV[s],1].min() for s in 'LR'}
def smooth(t):t=min(1,max(0,t));return t*t*(3-2*t)
def keys(k,pts):
    if k<=pts[0][0]:return pts[0][1]
    for (a,x),(b,y) in zip(pts,pts[1:]):
        if k<=b:return x+(y-x)*smooth((k-a)/(b-a))
    return pts[-1][1]
def vkeys(k,pts):
    a,x=pts[0];b,y=pts[1]
    if a<k<=b:u=(k-a)/(b-a);u=1-(1-u)**2;return x+(y-x)*u
    return keys(k,pts)
def arc(k,a,b,h):return h*math.sin(math.pi*(k-a)/(b-a))**2 if a<k<b else 0.
def breathe(k,a,b,amp):return amp*math.sin(math.pi*(k-a)/(b-a))*math.sin(2*math.pi*(k-a)/14.) if a<k<b else 0.


# Dash Left (+X is the heavy's left). 13 frames at 30 Hz = 26 ticks = its roll_active_frames 14
# + roll_recovery_frames 12. In place: the motor carries it.
def cand_A(k):
    """Shoulder lurch: no airtime. The weight drops and throws sideways, the lead foot
    shoves out, the trailing foot drags in after it, a heavy settle."""
    N=13;c={}
    c['shift']=keys(k,[(0,0),(1,-400),(4,2600),(7,2400),(10,600),(13,0)])
    c['depth']=keys(k,[(0,0),(4,-300),(13,0)])
    c['height']=keys(k,[(0,0),(1,-700),(3,-1800),(6,-2200),(8,-2600),(10,-900),(12,100),(13,0)])
    c['fold']=keys(k,[(0,0),(2,8),(5,12),(9,10),(13,0)])
    c['twist']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['list']=keys(k,[(0,0),(1,3),(4,-14),(7,-12),(10,-3),(13,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(3,-6),(7,-4),(13,0)]),out=keys(k,[(0,0),(3,10),(8,6),(13,0)])),
               'R':dict(swing=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,10),(7,6),(13,0)]),out=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,-6),(8,-2),(13,0)]))}
    c['feet']={'L':dict(dx=keys(k,[(0,0),(1,0),(5,2600),(9,2600),(12,0),(13,0)]),dz=0,lift=arc(k,1,5,1300)+arc(k,9,12,500),roll=0),
               'R':dict(dx=keys(k,[(0,0),(4,0),(9,1800),(10,1800),(12,0),(13,0)]),dz=0,lift=arc(k,4,9,700),roll=0)}
    return N,c
def cand_B(k):
    """Stomp hop: a crouch and one heavy sideways hop that only just clears the floor,
    both feet slam down together with a deep compression, then it heaves up."""
    N=13;c={}
    c['shift']=keys(k,[(0,0),(2,-600),(5,2000),(8,1800),(11,300),(13,0)])
    c['depth']=keys(k,[(0,0),(2,300),(13,0)])
    c['height']=keys(k,[(0,0),(2,-2400),(4,-400),(5,0),(6,-600),(7,-3200),(9,-2000),(11,-300),(12,80),(13,0)])
    c['fold']=keys(k,[(0,0),(2,14),(4,2),(7,18),(10,8),(13,0)])
    c['twist']=keys(k,[(0,0),(4,-4),(13,0)])
    c['list']=keys(k,[(0,0),(2,4),(4,-10),(7,4),(9,-2),(13,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(2,-8),(5,-14),(8,4),(13,0)]),out=keys(k,[(0,0),(4,16),(8,6),(13,0)])),
               'R':dict(swing=keys(min(13,max(0,k-1)*13/12),[(0,0),(2,-8),(5,-14),(8,4),(13,0)]),out=keys(min(13,max(0,k-1)*13/12),[(0,0),(4,16),(8,6),(13,0)]))}
    c['feet']={'L':dict(dx=keys(k,[(0,0),(2,0),(5,1200),(7,1200),(13,1200)]) if False else keys(k,[(0,0),(2,0),(6,1200),(10,1200),(12,0),(13,0)]),dz=0,lift=arc(k,2,7,1500),roll=0),
               'R':dict(dx=keys(k,[(0,0),(2,0),(6,1000),(10,1000),(12,0),(13,0)]),dz=0,lift=arc(k,2,7,1300),roll=0)}
    return N,c
CANDS={'heavy_A_shoulder_lurch':cand_A,'heavy_B_stomp_hop':cand_B}

from dashtravel import travel,HEAVY_TRAVEL
def TR(k):return travel(k,HEAVY_TRAVEL)
def cand_A2(k):
    """Shoulder lurch v2: the motor launches at tick 0, so the heavy bounds: the lead foot
    reaches out and lands at frame 4, pinned in the world for the rest of the travel, the
    trailing foot swings across and lands under it at frame 7, heavy settle."""
    N=13;c={};T7=TR(7)
    c['shift']=keys(k,[(0,0),(1,1500),(3,3200),(5,2400),(7,1200),(10,200),(13,0)])
    c['depth']=keys(k,[(0,0),(4,-300),(13,0)])
    c['height']=keys(k,[(0,0),(1,-900),(3,-1600),(4,-2400),(6,-2200),(7,-2600),(9,-1200),(11,-200),(12,80),(13,0)])
    c['fold']=keys(k,[(0,0),(2,8),(5,12),(9,6),(13,0)])
    c['twist']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['list']=keys(k,[(0,0),(1,-6),(3,-14),(5,-12),(8,-4),(13,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(3,-6),(7,-4),(13,0)]),out=keys(k,[(0,0),(3,10),(8,6),(13,0)])),
               'R':dict(swing=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,10),(7,6),(13,0)]),out=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,-6),(8,-2),(13,0)]))}
    if k<=0:ldx=0.
    elif k<4:ldx=(T7-TR(4))*smooth(k/4)-1500*math.sin(math.pi*k/4)
    else:ldx=T7-TR(k)
    rdx=-2500*math.sin(math.pi*k/7) if 0<k<7 else 0.
    c['feet']={'L':dict(dx=ldx,dz=0,lift=arc(k,0,4,2600),roll=0),'R':dict(dx=rdx,dz=0,lift=arc(k,0,7,1800),roll=0)}
    return N,c
CANDS={'heavy_A2_shoulder_lurch_v2':cand_A2}

def cand_A3(k):
    """Shoulder lurch v3 (on the Strafe Left start pose): frame 1 the trailing foot pushes off
    where it stood while the lead foot is already up; a short heavy bound, the lead foot finishes
    its sideways reach before it comes down at frame 4 and is pinned in the world for the rest
    of the travel; the trailing foot swings across and comes straight down at frame 7."""
    N=13;c={};T7=TR(7)
    c['shift']=keys(k,[(0,0),(1,-3200),(3,2600),(5,2200),(7,1200),(10,200),(13,0)])
    c['depth']=keys(k,[(0,0),(4,-300),(13,0)])
    c['height']=keys(k,[(0,0),(1,-2600),(2,-900),(3,-900),(4,-2300),(6,-2000),(7,-2400),(9,-1100),(11,-200),(12,80),(13,0)])
    c['fold']=keys(k,[(0,0),(1,6),(3,10),(5,12),(9,6),(13,0)])
    c['twist']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['list']=keys(k,[(0,0),(1,-4),(3,-14),(5,-12),(8,-4),(13,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(3,-10),(7,-8),(13,0)]),out=keys(k,[(0,0),(3,14),(8,8),(13,0)])),
               'R':dict(swing=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,-4),(7,-4),(13,0)]),out=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,-6),(8,-2),(13,0)]))}
    lwx=keys(k,[(0,0),(1,.12*T7),(3,T7),(13,T7)]);rwx=keys(k,[(0,0),(1,0),(2,.5*T7),(3,.9*T7),(4,T7),(13,T7)])
    llift=keys(k,[(0,0),(1,1600),(2,2600),(3,1000),(4,0),(13,0)])
    rlift=keys(k,[(0,0),(1,0),(2,1800),(3,2000),(4,1400),(5,700),(6,200),(7,0),(13,0)])
    c['feet']={'L':dict(dx=lwx-TR(k),dz=0,lift=llift,roll=0),'R':dict(dx=rwx-TR(k),dz=0,lift=rlift,roll=0)}
    return N,c

from dashtravel import travel as _trv
SLOW_ACTIVE=32
def TRS(k):return _trv(k,20937,SLOW_ACTIVE)
def WARP(k):
    """New frame -> v2 key time: the v2 active part (frames 0-7) stretched over 16 frames,
    its recovery (7-13) over the remaining 11."""
    return k*7/16 if k<=16 else 7+(k-16)*6/11
def cand_A4(k):
    """Shoulder lurch v3 (on the Strafe Left start pose): frame 1 the trailing foot pushes off
    where it stood while the lead foot is already up; a short heavy bound, the lead foot finishes
    its sideways reach before it comes down at frame 4 and is pinned in the world for the rest
    of the travel; the trailing foot swings across and comes straight down at frame 7."""
    N=27;c={};T7=TRS(27);kk=k;k=WARP(kk)
    c['shift']=keys(k,[(0,0),(1,-3200),(3,2600),(5,2200),(7,1200),(10,200),(13,0)])
    c['depth']=keys(k,[(0,0),(4,-300),(13,0)])
    c['height']=keys(k,[(0,0),(1,-2600),(2,-900),(3,-900),(4,-2300),(6,-2000),(7,-2400),(9,-1100),(11,-200),(12,80),(13,0)])
    c['fold']=keys(k,[(0,0),(1,6),(3,10),(5,12),(9,6),(13,0)])
    c['twist']=keys(k,[(0,0),(3,-6),(8,-4),(13,0)])
    c['list']=keys(k,[(0,0),(1,-4),(3,-14),(5,-12),(8,-4),(13,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(3,-10),(7,-8),(13,0)]),out=keys(k,[(0,0),(3,14),(8,8),(13,0)])),
               'R':dict(swing=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,-4),(7,-4),(13,0)]),out=keys(min(13,max(0,k-1)*13/12),[(0,0),(3,-6),(8,-2),(13,0)]))}
    lwx=keys(k,[(0,0),(1,.12*T7),(3,T7),(13,T7)]);rwx=keys(k,[(0,0),(1,0),(1.35,.02*T7),(2,.5*T7),(3,.9*T7),(4,T7),(13,T7)])
    llift=keys(k,[(0,0),(1,1600),(2,2600),(3,1000),(4,0),(13,0)])
    rlift=keys(k,[(0,0),(1,0),(1.35,1100),(2,2600),(3,3300),(4,1700),(4.6,0),(13,0)])
    c['feet']={'L':dict(dx=lwx-TRS(kk),dz=0,lift=llift,roll=0),'R':dict(dx=rwx-TRS(kk),dz=0,lift=rlift,roll=0)}
    return N,c
CANDS={'heavy_A4_shoulder_lurch_slow':cand_A4}

def cand_A4R(k):
    """Dash Right (on the Strafe Right start pose): cand_A4 mirrored per limb: the right foot
    leads, the left pushes off; sideways body channels, arm roles and travel flip."""
    N,c=cand_A4(k)
    for ch in ['shift','list','twist']:c[ch]=-c[ch]
    c['arms']={'L':c['arms']['R'],'R':c['arms']['L']}
    lead,trail=c['feet']['L'],c['feet']['R']
    c['feet']={'R':dict(lead,dx=-lead['dx']),'L':dict(trail,dx=-trail['dx'])}
    return N,c
CANDS={'heavy_A4_shoulder_lurch_slow':cand_A4,'heavy_A4R_shoulder_lurch_slow_right':cand_A4R}
def pose(fn,k):
    N,c=fn(k);S=BASE.copy()
    for j in range(NJ):S[j,:3,3]+=np.array([c['shift'],c['height'],c['depth']])
    upper=DESC[1]
    turnj(S,1,c['twist'],'Y',upper);turnj(S,1,c['fold'],'X',upper);turnj(S,1,c['list'],'Z',upper)
    for s,(up,fo) in ARMS.items():
        side=1 if s=='L' else -1;a=c['arms'][s]
        turnj(S,up,a['swing'],'X');turnj(S,up,side*a['out'],'Z');turnj(S,fo,a.get('elbow',0),'X')
    reach=[];tg={s:foot_targets(s,**c['feet'][s]) for s in 'LR'}
    def need(s):
        h,_,a,_=LEGS[s];H=piv(S,h);Af=tg[s][a][:3,:3]@PIV[a]+tg[s][a][:3,3];return np.linalg.norm(Af-H)/LEN[s]
    for _ in range(200):
        if all(need(s)<=LIM[s] for s in 'LR'):break
        for j in range(NJ):S[j,1,3]-=40.
    for s in 'LR':reach.append(leg_ik(S,s,tg[s]))
    claw_guard(S)
    return S,max(reach)
def pack(path,frames,hz):
    shift=max(0,math.ceil(math.log2(max(1,np.abs(frames[:,:,:3,3]).max())/32760)))
    payload=bytearray(struct.pack('<4H',NJ,len(frames),hz,shift))
    for frame in frames:
        for m in frame:
            vals=[*np.rint(m[:3,:3].T*4096).astype(int).ravel(),*np.rint(m[:3,3]/2**shift).astype(int)]
            assert all(-32768<=x<=32767 for x in vals);payload+=struct.pack('<12h',*vals)
    head=bytearray((A/'stun_recovery.psxanim').read_bytes()[:12]);struct.pack_into('<H',head,4,2);struct.pack_into('<I',head,8,len(payload));path.write_bytes(head+payload)

want=sys.argv[1:]
FACES=np.array([[c[0] for c in f] for f in M['f']],np.int32)
for label,fn in CANDS.items():
    if want and not any(label.startswith(w) for w in want):continue
    N=fn(0)[0];frames=[];V=[];reach=0;lows=[]
    for k in range(N+1):
        S,r=pose(fn,k);reach=max(reach,r);frames.append(S);v=verts(S);V.append(v);lows.append(float(v[:,1].min()-FLOOR))
    frames.append(frames[-1].copy());V.append(V[-1]);frames=np.array(frames)
    pack(OUT/f'{label}.psxanim',frames,HZ)
    np.savez(OUT/f'{label}.npz',verts=np.array(V,np.float32),faces=FACES,hz=HZ,skin=frames)
    print('RESULT',label,json.dumps({'frames':N,'stored':len(frames),'max_leg_reach':reach,'min_above_floor':min(lows),'start_vs_base':float(np.abs(frames[0]-BASE).max()),'end_vs_base':float(np.abs(frames[-1]-BASE).max())}),flush=True)
