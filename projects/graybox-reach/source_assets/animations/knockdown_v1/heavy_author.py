"""Heavy enemy knockdown candidates. Shipped: B_claw_brace (default/assets/animations/knockdown_v1/heavy_knockdown.psxanim). Keyed body channels over the
pose the Heavy's Stun + Recovery starts and ends on, in the cooked clip's own
skin space: chains turn about joint pivots solved from the shipped clips,
two-bone leg IK onto planted (or stepped, heel-rolled) feet, and a claw floor
guard. 30 Hz. Plain python3 + numpy. Args: [candidate prefixes]."""
import sys,math,json,struct
sys.dont_write_bytecode=True;sys.path.insert(0,sys.path[0])
from psxdecode import model,animation,deform
import numpy as np
from pathlib import Path
OUT=Path(__file__).resolve().parent
P=OUT.parents[3]
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
BASE=stun[0].copy()
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
ARMV={s:np.where(np.isin(OWN,DESC[ARMS[s][1]]))[0] for s in 'LR'}
def claw_guard(S,margin=150.):
    """A claw that would go through the floor swings about the elbow by the
    smallest angle (either way) that rests it on the floor."""
    for s,(up,fo) in ARMS.items():
        def low(T):return verts(T)[ARMV[s],1].min()
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

# Model space: +Y up, +Z forward, +X the enemy's left. Units: the body is ~31,400 tall.
def cand_A(k):
    """Knee slam: the hit rocks the whole mass back, one heavy step back with the
    right foot, the legs give and the right knee slams down; torso slumps over,
    claws drag on the floor; a long heaving hold; a slow two-stage rise."""
    N=84;c={}
    c['shift']=vkeys(k,[(0,0),(3,-500),(8,-900),(20,-600),(52,-500),(66,-200),(84,0)])
    c['depth']=vkeys(k,[(0,0),(3,-1200),(8,-2600),(14,-2400),(20,-1300),(52,-1200),(62,-900),(72,-200),(84,0)])
    c['height']=keys(k,[(0,0),(3,500),(8,-600),(13,-4200),(18,-7200),(21,-6900),(52,-6800),(58,-5600),(62,-5900),(70,-2500),(77,-300),(81,120),(84,0)])+breathe(k,22,52,260)
    c['fold']=vkeys(k,[(0,0),(3,-14),(7,-10),(12,10),(18,30),(22,34),(52,32),(58,24),(62,27),(70,12),(78,1),(84,0)])+breathe(k,22,52,2)
    c['twist']=vkeys(k,[(0,0),(3,10),(8,14),(18,6),(52,5),(66,0),(84,0)])
    c['list']=vkeys(k,[(0,0),(3,-6),(9,-8),(18,-6),(52,-5),(66,0),(84,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(2,-6),(6,-20),(12,4),(20,0),(84,0)]),out=keys(k,[(0,0),(3,10),(8,16),(14,4),(84,0)])),
               'R':dict(swing=keys(max(0,k-1.5),[(0,0),(2,-6),(6,-22),(12,4),(20,0),(84,0)]),out=keys(max(0,k-1.5),[(0,0),(3,12),(8,18),(14,4),(84,0)]))}
    c['feet']={'L':dict(dx=0,dz=0,lift=0,roll=0),
               'R':dict(dx=keys(k,[(0,0),(4,0),(12,-600),(60,-600),(72,0),(84,0)]),dz=keys(k,[(0,0),(4,0),(12,-4200),(60,-4200),(72,0),(84,0)]),
                        lift=arc(k,4,12,2200)+arc(k,60,72,2600),roll=keys(k,[(0,0),(12,0),(20,58),(56,58),(62,40),(68,10),(72,0),(84,0)]))}
    return N,c
def cand_B(k):
    """Claw brace: two heavy stumbling steps back, the body sags forward and the
    left claw is thrown down onto the floor as a prop; hold leaning on it; it
    shoves itself up off the claw and walks its feet back under it."""
    N=86;c={}
    c['shift']=vkeys(k,[(0,0),(3,600),(9,800),(16,400),(54,400),(68,100),(86,0)])
    c['depth']=vkeys(k,[(0,0),(3,-1800),(9,-3400),(16,-5200),(22,-5400),(54,-5300),(66,-3200),(78,-400),(86,0)])
    c['height']=keys(k,[(0,0),(3,300),(9,-800),(16,-2600),(22,-5200),(26,-5000),(54,-4900),(60,-3800),(68,-1800),(78,-200),(82,100),(86,0)])+breathe(k,24,54,240)
    c['fold']=vkeys(k,[(0,0),(3,-12),(8,-6),(16,14),(22,38),(26,42),(54,40),(60,30),(68,14),(78,2),(86,0)])+breathe(k,24,54,2)
    c['twist']=vkeys(k,[(0,0),(3,-10),(9,-12),(18,-14),(54,-13),(66,-5),(86,0)])
    c['list']=vkeys(k,[(0,0),(3,8),(9,10),(20,12),(54,11),(66,3),(86,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(3,-10),(8,-18),(14,12),(20,22),(54,21),(62,6),(72,0),(86,0)]),out=keys(k,[(0,0),(3,14),(9,10),(20,6),(54,6),(70,0),(86,0)])),
               'R':dict(swing=keys(max(0,k-1.5),[(0,0),(3,-10),(8,-20),(14,-4),(22,4),(86,0)]),out=keys(max(0,k-1.5),[(0,0),(3,14),(9,16),(16,6),(86,0)]),elbow=keys(k,[(0,0),(12,0),(20,-34),(56,-34),(68,0),(86,0)]))}
    c['feet']={'R':dict(dx=0,dz=keys(k,[(0,0),(3,0),(9,-3400),(66,-3400),(76,0),(86,0)]),lift=arc(k,3,9,2400)+arc(k,66,76,2200),roll=0),
               'L':dict(dx=0,dz=keys(k,[(0,0),(9,0),(16,-3600),(58,-3600),(68,0),(86,0)]),lift=arc(k,9,16,2400)+arc(k,58,68,2200),roll=0)}
    return N,c
def cand_C(k):
    """Collapse: both legs buckle straight down into a deep squat under the
    weight, the torso slumps forward and lists, claws drag; a long hold; a
    failed first try to stand, then a slow full rise. No stepping."""
    N=90;c={}
    c['shift']=vkeys(k,[(0,0),(3,-700),(8,-1100),(18,-800),(58,-700),(72,-200),(90,0)])
    c['depth']=vkeys(k,[(0,0),(3,-1600),(8,-2400),(14,-1900),(20,-1500),(58,-1400),(70,-600),(90,0)])
    c['height']=keys(k,[(0,0),(3,400),(8,-1200),(13,-5600),(18,-8600),(21,-8300),(46,-8200),(51,-6400),(55,-7600),(62,-5600),(72,-2400),(80,-300),(85,120),(90,0)])+breathe(k,22,46,280)
    c['fold']=vkeys(k,[(0,0),(3,-12),(7,-8),(13,14),(18,32),(22,36),(46,34),(51,26),(55,33),(64,20),(74,6),(82,0),(90,0)])+breathe(k,22,46,2)
    c['twist']=vkeys(k,[(0,0),(3,8),(9,10),(20,4),(46,4),(64,0),(90,0)])
    c['list']=vkeys(k,[(0,0),(3,-10),(9,-14),(20,-12),(46,-11),(56,-6),(70,-1),(90,0)])
    c['arms']={'L':dict(swing=keys(k,[(0,0),(3,-10),(7,-16),(14,4),(22,0),(90,0)]),out=keys(k,[(0,0),(3,10),(8,14),(16,2),(90,0)])),
               'R':dict(swing=keys(max(0,k-1.5),[(0,0),(3,-10),(7,-18),(14,4),(22,0),(90,0)]),out=keys(max(0,k-1.5),[(0,0),(3,12),(8,16),(16,2),(90,0)]))}
    c['feet']={'L':dict(dx=0,dz=0,lift=0,roll=0),'R':dict(dx=0,dz=0,lift=0,roll=0)}
    return N,c
CANDS={'A_knee_slam':cand_A,'B_claw_brace':cand_B,'C_collapse':cand_C}

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
report={}
FACES=np.array([[c[0] for c in f] for f in M['f']],np.int32)
for label,fn in CANDS.items():
    if want and not any(label.startswith(w) for w in want):continue
    N=fn(0)[0];frames=[];V=[];reach=0;lows=[]
    for k in range(N+1):
        S,r=pose(fn,k);reach=max(reach,r);frames.append(S);v=verts(S);V.append(v)
        lows.append({'k':k,'all':float(v[:,1].min()-FLOOR),'low_owner':int(OWN[v[:,1].argmin()]),
                     'knee_R':float(v[OWN==23,1].min()-FLOOR),'knee_L':float(v[OWN==18,1].min()-FLOOR)})
    frames.append(frames[-1].copy());V.append(V[-1]);frames=np.array(frames)
    pack(OUT/f'{label}.psxanim',frames,HZ)
    np.savez(OUT/f'{label}.npz',verts=np.array(V,np.float32),faces=FACES,hz=HZ,skin=frames)
    (OUT/f'{label}_contacts.json').write_text(json.dumps(lows))
    report[label]={'frames':N,'stored':len(frames),'hz':HZ,'seconds':N/HZ,'max_leg_reach':reach,'min_above_floor':min(x['all'] for x in lows),
                   'min_knee_R':min(x['knee_R'] for x in lows),'start_vs_base':float(np.abs(frames[0]-BASE).max()),'end_vs_base':float(np.abs(frames[-1]-BASE).max())}
    print('RESULT',label,json.dumps(report[label]),flush=True)
(OUT/'author_report.json').write_text(json.dumps(report,indent=2))
print('FLOOR',FLOOR)
