from pathlib import Path
import numpy as np,json,struct
P=Path(__file__).resolve().parents[3];O=Path(__file__).resolve().parent;s=(P/'tools/enemy_reduction/validate_render.py').read_text();exec(s[s.index('def model'):s.index('def mesh')]);m=model(P/'assets/models/light_body_v1/light.psxmdl');data=np.load(O/'walk.npz');contact=data['contact'];bind=data['bind'];scale=184/4096;report={}
for name,path in [('before',P/'assets/animations/light_stalk_v2/walk.psxanim'),('after',O/'walk.psxanim')]:
 poses=animation(path);assert all(np.array_equal(a[0],b[0]) and np.array_equal(a[1],b[1]) for a,b in zip(poses[0],poses[-1]));poses=poses[:-1];points=np.array([deform(m,p) for p in poses]);g=[]
 for pose in poses:
  sk=np.tile(np.eye(4),(22,1,1))
  for j,(r,t) in enumerate(pose):sk[j,:3,:3]=r;sk[j,:3,3]=t
  g.append(sk@bind)
 g=np.array(g);steps=np.sqrt(np.mean(np.sum(np.diff(points,axis=0)**2,axis=2),axis=1));seam=np.sqrt(np.mean(np.sum((points[-1]-points[0])**2,axis=1)));ext=[]
 for a,b,c in [(14,15,16),(18,19,20)]:
  h,k,f=g[:,a,:3,3],g[:,b,:3,3],g[:,c,:3,3];ext.extend(np.linalg.norm(f-h,axis=1)/(np.linalg.norm(k-h,axis=1)+np.linalg.norm(f-k,axis=1)))
 d={'unique_frames':len(poses),'seam_over_largest_step':float(seam/steps.max()),'max_leg_extension':float(max(ext)),'head_vertical_range_world_units':float(np.ptp(g[:,5,1,3])*scale),'cannon_forward_range_world_units':float(np.ptp(g[:,13,2,3])*scale),'knee_lateral_range_world_units':[float(np.ptp(g[:,j,0,3])*scale) for j in [15,19]],'floor_range_world_units':float(np.ptp(points[:,:,1].min(1))*scale)}
 if name=='after':
  assert np.isfinite(points).all();drift=[]
  for leg,joints in enumerate([[16,17],[20,21]]):
   ids=[i for i in range(len(m['v'])) if m['owner'][i] in joints]
   for i in range(24):
    j=(i+1)%24
    if contact[i,leg] and contact[j,leg]:
     delta=(points[j,ids]-points[i,ids])*scale+np.array([0,0,1680/30]);drift.extend(np.linalg.norm(delta,axis=1))
     # Runtime linearly interpolates skin matrices: test fractional contact poses too.
     for alpha in [.25,.5,.75]:
      p=[(a[0]*(1-alpha)+b[0]*alpha,a[1]*(1-alpha)+b[1]*alpha) for a,b in zip(poses[i],poses[j])];part=deform(m,p)[ids];drift.extend(np.linalg.norm((part-points[i,ids])*scale+np.array([0,0,1680/30*alpha]),axis=1))
  d['max_contact_drift_world_units']=float(max(drift));assert max(drift)<.2;assert max(ext)<.97;assert seam/steps.max()<1.15;assert all(contact.any(axis=1))
 report[name]=d
report['contact_assumption']='Straight forward speed1680 world units/sec, scale417, 30Hz clip, in_place=false; fractional source matrix interpolation included.';(O/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
