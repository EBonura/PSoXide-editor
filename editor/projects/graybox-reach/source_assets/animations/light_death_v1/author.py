"""Authored, grounded one-shot collapse; no runtime physics or added joints."""
from pathlib import Path
import runpy,math,json
import bpy
import numpy as np
from mathutils import Matrix,Vector
O=Path(__file__).resolve().parent;P=O.parents[2]
s=runpy.run_path(str(O.parent/'light_motion_v1/author.py'))
for name in ['bind','ib','parents','base','legs','turn','ik','keys','pack','mesh','deform']:
 globals()[name]=s[name]
N=66;HZ=30;FLOOR=-20000.

def elbow(g,a,b,c,degrees):
 u=g[b,:3,3]-g[a,:3,3];v=g[c,:3,3]-g[b,:3,3];axis=np.cross(u,v);axis/=np.linalg.norm(axis)
 r=np.array(Matrix.Rotation(math.radians(degrees),3,Vector(axis)));p=g[b,:3,3].copy()
 for j in [b,c]:g[j,:3,:3]=r@g[j,:3,:3];g[j,:3,3]=p+r@(g[j,:3,3]-p)

def pose(k):
 g=base.copy()
 sink=keys(k,[(0,0),(5,450),(10,-900),(16,-2300),(24,-4800),(33,-11000),(40,-17000),(43,-16500),(49,-17500),(58,-17500),(66,-17500)])
 shift=keys(k,[(0,0),(7,450),(16,1400),(24,2400),(33,4800),(40,7200),(49,7500),(66,7500)])
 front=keys(k,[(0,0),(6,-1200),(14,-900),(24,800),(40,5000),(50,5400),(66,5400)])
 g[:,:3,3]+=np.array([shift,sink,front])
 # Recoil yields to a hanging chest; final yaw makes a diagonal corpse silhouette.
 yaw=keys(k,[(0,0),(5,-15),(10,-18),(22,-7),(36,-18),(45,-20),(66,-20)])
 chest=keys(k,[(0,0),(4,-19),(8,-17),(16,9),(24,25),(38,8),(42,9),(50,5),(58,6),(66,6)])
 turn(g,1,yaw,'Y',range(1,14));turn(g,2,chest,'X',range(2,14))
 nod=keys(k,[(0,0),(3,2),(7,-15),(12,-9),(23,13),(37,-8),(41,-18),(46,-26),(55,-30),(66,-30)])
 turn(g,4,nod,'X',[4,5]);turn(g,4,keys(k,[(0,0),(9,-9),(23,7),(41,14),(56,11),(66,11)]),'Y',[4,5])
 collapse=keys(k,[(0,0),(10,0),(24,.38),(40,1),(66,1)])
 for a,b,c,side,amount in [(7,8,9,1,1),(11,12,13,-1,.7)]:
  recoil=keys(k,[(0,0),(3,.10),(8,1),(14,.5),(24,0),(66,0)])
  arm_fall=keys(k,[(0,0),(10,0),(24,-45),(30,35),(35,38),(40,4),(44,1),(52,5),(58,5),(66,5)])
  turn(g,a,12*recoil+arm_fall*amount,'X',[a,b,c])
  turn(g,a,side*(8*recoil+24*collapse),'Z',[a,b,c])
  turn(g,a,(-16 if side==1 else 14)*collapse,'Y',[a,b,c])
  elbow(g,a,b,c,-8*recoil+keys(k,[(0,0),(10,0),(24,18),(34,6),(40,-10),(46,-6),(56,-8),(66,-8)])*amount)
 # Uneven leg collapse. Solve contact first; after support fails, rotate the
 # folded leg chain with the torso. The rear leg trails instead of mirroring.
 for i,(side,h,n,a,t,relative,center,lengths) in enumerate(legs):
  foot=relative.copy();target=center.copy()
  target[2]+=keys(k,[(0,0),(16,0),(24,-1800 if i==0 else -200),(38,-3300 if i==0 else -1200),(66,-3300 if i==0 else -1200)])
  target[0]+=keys(k,[(0,0),(18,0),(30,650 if i==0 else -800),(66,650 if i==0 else -800)])
  foot[:,:3,3]+=target
  # Cache a crouched lower-body shape at frame 24 before the whole-body fall.
  if k<=24:
   ik(g,h,n,a,foot[0,:3,3],np.array([side*.35,-.12,1.]));g[a],g[t]=foot
  else:
   crouch=pose(24)
   neutral=np.load(O.parent/'light_reaction_v1/stun.npz')['skin'][0]@bind
   for sg,hh,nn,aa,tt,_,_,ll in legs:
    target=neutral[hh,:3,3]+np.array([sg*400,-sum(ll)*.96,-600])
    delta=target-neutral[aa,:3,3];foot=neutral[[aa,tt]].copy();foot[:,:3,3]+=delta
    ik(neutral,hh,nn,aa,target,np.array([sg*.15,0,1.]));neutral[aa],neutral[tt]=foot
    turn(neutral,aa,70,'X',[aa,tt])
   blend=keys(k,[(24,0),(33,0),(40,.25),(50,.85),(58,1),(66,1)])
   for j in [h,n,a,t]:
    parent=int(parents[j]);old=np.linalg.inv(crouch[parent])@crouch[j];new=np.linalg.inv(neutral[parent])@neutral[j]
    local=np.eye(4);local[:3,:3]=np.array(Matrix(old[:3,:3]).to_quaternion().slerp(Matrix(new[:3,:3]).to_quaternion(),blend).to_matrix());local[:3,3]=old[:3,3]*(1-blend)+new[:3,3]*blend
    g[j]=g[parent]@local
   turn(g,h,(-3 if i==0 else -12)*blend,'X',[h,n,a,t])
   turn(g,h,side*3*blend,'Z',[h,n,a,t])
 # Tilt accelerates into impact, then a restrained ground settle.
 lean=keys(k,[(0,0),(10,-2),(18,-6),(24,-10),(30,-14),(35,-17),(40,-22),(43,-18),(49,-22),(58,-22),(66,-22)])
 turn(g,0,lean,'Z',range(22))
 pitch=keys(k,[(0,0),(16,0),(24,12),(30,27),(35,48),(40,82),(43,77),(49,84),(58,84),(66,84)])
 turn(g,0,pitch,'X',range(22))
 # Until support fails, restore planted soles and solve the bent knees again.
 if k<=18:
  for side,h,n,a,t,relative,center,lengths in legs:
   foot=relative.copy();foot[:,:3,3]+=center
   ik(g,h,n,a,foot[0,:3,3],np.array([side*.35,-.12,1.]));g[a],g[t]=foot
 # Ground the actual mesh, including the long claw tips, not just joint pivots.
 sk=g@ib;v=deform(mesh,[(x[:3,:3],x[:3,3]) for x in sk]);lift=max(0,FLOOR-v[:,1].min())
 g[:,1,3]+=lift
 return g
frames=np.array([pose(k)@ib for k in range(N+1)])
frames=np.concatenate([frames,frames[-1:]])
pack(O/'death.psxanim',frames)
np.savez(O/'death.npz',skin=frames,bind=bind,parents=parents,contact=np.array([[k<=18,k<=18] for k in range(N+2)]))
meta={'death':{'frames':N,'stored_frames':len(frames),'sample_hz':HZ,'playback_speed_q8':256,'duration_seconds':N/HZ,'loop':False,'speed':0.,'still_from_frame':58}}
(O/'authoring.json').write_text(json.dumps(meta,indent=2));print('RESULT',json.dumps(meta),'Blender',bpy.app.version_string)
