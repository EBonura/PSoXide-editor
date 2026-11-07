"""Run, turn-adjustment and acquisition alert for Graybox Reach's light enemy.
Run with Blender --background --factory-startup --python-exit-code 1.
Coordinates stay in the original 22-joint model space; the game owns actor yaw.
"""
from pathlib import Path
import ast, json, math, struct, sys
import bpy
import numpy as np
from mathutils import Matrix, Vector

OUT=Path(__file__).resolve().parent
PROJECT=OUT.parents[2]
HZ=30
SCALE=184/4096/16
FLOOR=-20000.
SPECS={'run':dict(frames=20,loop=True,speed=360.),
       'turn':dict(frames=24,loop=True,speed=0.),
       'alert':dict(frames=60,loop=False,speed=0.)}

def helpers(path,names):
    tree=ast.parse(path.read_text())
    exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names],type_ignores=[]),str(path),'exec'),globals())
helpers(PROJECT/'tools/enemy_reduction/validate_render.py',{'model','animation','deform'})
helpers(PROJECT/'source_assets/animations/light_stalk_v2/author.py',{'turn','ik'})
data=np.load(PROJECT/'source_assets/characters/light_body_v1/bind.npz')
bind,parents=data['bind'],data['parents'];ib=np.linalg.inv(bind)
mesh=model(PROJECT/'assets/models/light_body_v1/light.psxmdl')
walk=np.load(PROJECT/'source_assets/animations/light_walk_v5/walk.npz')['skin']@bind
base=walk[0].copy()
legs=[]
for side,hip,knee,ankle,toe in [(1,14,15,16,17),(-1,18,19,20,21)]:
    ids=[i for i in range(len(mesh['v'])) if mesh['owner'][i] in [ankle,toe]]
    points=np.array([mesh['v'][i][:3] for i in ids])
    sole=np.array([points[:,0].mean(),points[:,1].min(),points[:,2].mean()])
    relative=bind[[ankle,toe]].copy();relative[:,:3,3]-=sole
    center=np.array([base[hip,0,3]+side*850,FLOOR,base[hip,2,3]-relative[0,2,3]])
    lengths=[np.linalg.norm(bind[knee,:3,3]-bind[hip,:3,3]),np.linalg.norm(bind[ankle,:3,3]-bind[knee,:3,3])]
    legs.append((side,hip,knee,ankle,toe,relative,center,lengths))

def smooth(t):
    t=np.clip(t,0.,1.);return t*t*(3-2*t)

def keys(t,points):
    if t<=points[0][0]:return points[0][1]
    for (a,x),(b,y) in zip(points,points[1:]):
        if t<=b:return x+(y-x)*smooth((t-a)/(b-a))
    return points[-1][1]

def sample_walk(frame):
    a=int(math.floor(frame))%24;b=(a+1)%24;t=frame%1;g=walk[a].copy()
    for j in range(22):
        g[j,:3,:3]=np.array(Matrix(walk[a,j,:3,:3]).to_quaternion().slerp(Matrix(walk[b,j,:3,:3]).to_quaternion(),t).to_matrix())
        g[j,:3,3]=walk[a,j,:3,3]*(1-t)+walk[b,j,:3,3]*t
    return g

def feet_at(name,k,index):
    side,hip,knee,ankle,toe,relative,center,_=legs[index]
    foot=relative.copy();target=center.copy();angle=0.
    if name=='run':
        # Linear ground travel, with a shorter support phase and lifted return.
        duty=.20;u=(k/20-index*.5)%1;travel=360/SCALE*20/HZ
        front,back=travel*duty/2,-travel*duty/2
        if u<duty:
            along=front-travel*u;lift=0.;plant=True
        else:
            t=(u-duty)/(1-duty)
            along=back+(front-back)*smooth(t)-travel*(1-duty)*t*(1-t)*(1-2*t)**9
            lift=4200*math.sin(math.pi*t)**2;plant=False
        target[2]+=along;target[1]+=lift
    elif name=='turn':
        # Two quick replant steps. Local yaw stays neutral: runtime turns actor.
        a,b=(2.,10.) if index==0 else (14.,22.)
        t=(k-a)/(b-a);active=0<t<1;arc=math.sin(math.pi*t)**2 if active else 0.
        target+=np.array([side*650*arc,1700*arc,side*900*math.sin(2*math.pi*t)*arc if active else 0.])
        angle=side*9*math.sin(2*math.pi*t)*arc if active else 0.;plant=not active
    else:
        # Only the lead foot steps and recovers. The rear foot anchors the
        # weight shift, with its full transform fixed for the whole gesture.
        if index==0:
            amount=keys(k,[(0,0),(8,0),(18,1),(47,1),(59,0),(60,0)])
            windows=[(8.,18.,3400.),(47.,59.,2400.)]
            offset=np.array([850.,0.,4800.]);angle=-9*amount
        else:
            amount=0.;windows=[]
            offset=np.zeros(3);angle=0.
        target+=offset*amount;plant=True
        for start,end,lift in windows:
            if start<k<end:
                target[1]+=lift*math.sin(math.pi*(k-start)/(end-start))**2;plant=False
    rot=np.array(Matrix.Rotation(math.radians(angle),3,'Y'))
    foot[:,:3,3]=np.einsum('ij,kj->ki',rot,foot[:,:3,3])+target
    foot[:,:3,:3]=np.einsum('ij,kjl->kil',rot,foot[:,:3,:3])
    return foot,plant

def body_at(name,k):
    if name=='run':
        g=sample_walk(k*24/20);phase=k/20
        # Less lateral sway than the stalking walk; stronger forward intent.
        g[:,0,3]-=(g[0,0,3]-base[0,0,3])*.55
        turn(g,1,6,'X',range(1,14));turn(g,4,-4,'X',[4,5])
        # Compression is grounded; rise occurs during the flight between feet.
        t=(k%10)/10
        rise=keys(t,[(0,0),(.16,-450),(.30,100),(.40,900),(.5,1100),(.7,600),(1,0)])
        g[:,1,3]+=rise
        for side,a,b,c in [(1,7,8,9),(-1,11,12,13)]:
            lag=.07 if side==1 else .12
            target=g[c,:3,3]+np.array([0,600,-side*3100*math.cos(2*math.pi*(phase-lag))])
            ik(g,a,b,c,target,np.array([side*.65,-.1,-.9]))
    else:
        g=base.copy()
        if name=='turn':
            weight=keys(k,[(0,0),(2,-900),(6,-1100),(11,0),(14,900),(18,1100),(23,0),(24,0)])
            g[:,0,3]+=weight
            g[:,1,3]+=keys(k,[(0,0),(3,-220),(9,100),(12,0),(15,-220),(21,100),(24,0)])
            turn(g,1,keys(k,[(0,0),(4,4),(10,-2),(12,0),(16,-4),(22,2),(24,0)]),'Y',range(1,14))
            turn(g,4,keys(k,[(0,0),(2,-5),(8,0),(14,5),(20,0),(24,0)]),'Y',[4,5])
        else:
            # Two-second step-and-point. Anticipation is back over the support
            # leg; the lead heel lands before the shoulder/arm commit.
            lead=keys(k,[(0,0),(10,0),(21,1),(38,1),(44,.75),(56,0),(60,0)])
            coil=keys(k,[(0,0),(9,1),(18,0),(60,0)])
            hip_yaw=keys(k,[(0,0),(9,10),(18,-9),(27,-7),(38,-7),(52,1),(60,0)])
            waist_yaw=keys(k,[(0,0),(10,12),(20,-17),(28,-13),(39,-13),(54,1),(60,0)])
            chest_yaw=keys(k,[(0,0),(11,6),(22,-8),(30,-5),(40,-5),(55,0),(60,0)])
            weight=keys(k,[(0,0),(8,-1800),(16,600),(20,1800),(29,1500),(38,1500),(46,900),(55,0),(60,0)])
            depth=keys(k,[(0,0),(9,-1400),(18,2000),(23,2600),(30,2300),(38,2300),(46,1800),(58,0),(60,0)])
            height=keys(k,[(0,0),(9,-950),(15,450),(19,-850),(25,150),(31,0),(38,0),(45,-300),(55,150),(60,0)])
            g[:,:3,3]+=np.array([weight,height,depth])
            turn(g,0,hip_yaw,'Y',range(22))
            turn(g,1,waist_yaw,'Y',range(1,14))
            turn(g,2,chest_yaw,'Y',range(2,14))
            turn(g,2,7*lead-5*coil,'X',range(2,14))
            turn(g,2,-5*lead+3*coil,'Z',range(2,14))
            # Head finds the player first and resists the shoulder wind-up.
            turn(g,4,-hip_yaw-waist_yaw-chest_yaw,'Y',[4,5])
            turn(g,4,keys(k,[(0,0),(4,-8),(14,-8),(24,-4),(39,-4),(56,0),(60,0)]),'X',[4,5])
            # The free arm sweeps behind him, then catches up to the ribcage.
            counter=keys(k,[(0,0),(11,-.35),(24,1),(32,.75),(40,.75),(56,0),(60,0)])
            a,b,c=11,12,13
            target=g[c,:3,3]+np.array([-1300*counter,1100*counter,-2800*counter])
            pole=g[b,:3,3]-g[a,:3,3]
            ik(g,a,b,c,target,pole)
            # Right claw arm points forward at the player, just below shoulder
            # height. The other arm stays down, so it reads as an indication.
            a,b,c=7,8,9
            shoulder=g[a,:3,3].copy()
            old_hand=g[c,:3,3].copy()
            old_rotation=g[c,:3,:3].copy()
            length=np.linalg.norm(g[b,:3,3]-shoulder)+np.linalg.norm(old_hand-g[b,:3,3])
            direction=np.array([-.15,-.05,1.]);direction/=np.linalg.norm(direction)
            target=old_hand*(1-lead)+(shoulder+direction*length*.95)*lead
            old_pole=g[b,:3,3]-shoulder;old_pole/=np.linalg.norm(old_pole)
            pole=old_pole*(1-lead)+np.array([.85,-.45,-.15])*lead
            ik(g,a,b,c,target,pole)
            # Orient the long claw tips along the pointing ray, rather than
            # leaving them hanging vertically from the raised wrist.
            tip_local=np.array([-443.90856805,10211.15579709,844.17601394])
            tip_direction=old_rotation@tip_local
            aim=np.array(Vector(tip_direction).rotation_difference(Vector(direction)).to_matrix())@old_rotation
            g[c,:3,:3]=np.array(Matrix(old_rotation).to_quaternion().slerp(Matrix(aim).to_quaternion(),lead).to_matrix())
    return g

def pack(path,frames,hz=HZ):
    shift=max(0,math.ceil(math.log2(max(1,np.abs(frames[:,:,:3,3]).max())/32760)))
    payload=bytearray(struct.pack('<4H',22,len(frames),hz,shift))
    for frame in frames:
        for mat in frame:
            vals=[*np.rint(mat[:3,:3].T*4096).astype(int).ravel(),*np.rint(mat[:3,3]/2**shift).astype(int)]
            assert all(-32768<=x<=32767 for x in vals)
            payload+=struct.pack('<12h',*vals)
    head=bytearray((PROJECT/'assets/animations/light_walk_v5/walk.psxanim').read_bytes()[:12])
    struct.pack_into('<H',head,4,2);struct.pack_into('<I',head,8,len(payload));path.write_bytes(head+payload)


def main():
    report=json.loads((OUT/'authoring.json').read_text()) if (OUT/'authoring.json').exists() else {}
    for name,spec in SPECS.items():
        if '--alert-only' in sys.argv and name!='alert':continue
        drop=0.
        for k in np.arange(0,spec['frames']+.01,.25):
            g=body_at(name,k)
            for index,(_,h,_,_,_,_,_,lengths) in enumerate(legs):
                foot,_=feet_at(name,k,index);hp,fp=g[h,:3,3],foot[0,:3,3]
                horizontal=np.sum((hp[[0,2]]-fp[[0,2]])**2);limit=.975*sum(lengths)
                assert horizontal<limit**2,(name,k,index,horizontal,limit**2)
                drop=max(drop,hp[1]-fp[1]-math.sqrt(limit**2-horizontal))
        frames=[];contacts=[];extensions=[]
        for k in range(spec['frames']+1):
            g=body_at(name,k);g[:,1,3]-=drop;contact=[]
            for index,(side,h,n,a,t,_,_,_) in enumerate(legs):
                feet,planted=feet_at(name,k,index)
                extensions.append(ik(g,h,n,a,feet[0,:3,3],np.array([side*.10,-.12,1.])))
                g[a],g[t]=feet;contact.append(planted)
            frames.append(g@ib);contacts.append(contact)
        frames=np.array(frames)
        if spec['loop']:frames[-1]=frames[0]
        pack(OUT/f'{name}.psxanim',frames)
        np.savez(OUT/f'{name}.npz',skin=frames,bind=bind,parents=parents,contact=contacts)
        report[name]={**spec,'sample_hz':HZ,'duration_seconds':spec['frames']/HZ,'pelvis_drop_model_units':drop,'max_leg_extension':max(extensions),'stored_frames':len(frames)}
    (OUT/'authoring.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps(report))
if __name__=='__main__':main()
