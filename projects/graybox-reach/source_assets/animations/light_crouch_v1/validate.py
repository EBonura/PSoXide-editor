from pathlib import Path
import json,struct,numpy as np
P=Path(__file__).resolve().parents[3];O=Path(__file__).resolve().parent;src=(P/'tools/enemy_reduction/validate_render.py').read_text();exec(src[src.index('def model'):src.index('def mesh')]);m=model(P/'assets/models/light_body_v1/light.psxmdl');scale=184/4096;report={};rig=np.load(P/'source_assets/characters/light_body_v1/bind.npz');bind=rig['bind']
for name in ['idle','walk']:
 poses=animation(O/f'{name}.psxanim');assert all(np.array_equal(a[0],b[0]) and np.array_equal(a[1],b[1]) for a,b in zip(poses[0],poses[-1]));poses=poses[:-1];pp=np.array([deform(m,p) for p in poses]);assert np.isfinite(pp).all();count=len(pp);steps=np.sqrt(np.mean(np.sum(np.diff(pp,axis=0)**2,axis=2),axis=1));seam=np.sqrt(np.mean(np.sum((pp[-1]-pp[0])**2,axis=1)));contact=np.ones((count,2),bool) if name=='idle' else np.load(P/'source_assets/animations/light_walk_v1/walk.npz')['contact'];drift=[]
 for leg,joints in enumerate([[16,17],[20,21]]):
  ids=[i for i in range(len(m['v'])) if m['owner'][i] in joints]
  for i in range(count):
   j=(i+1)%count
   if contact[i,leg] and contact[j,leg]:drift.extend(np.linalg.norm((pp[j,ids]-pp[i,ids])*scale+np.array([0,0,0 if name=='idle' else 1680/30]),axis=1))
 extensions=[];bone_deltas=[]
 for pose in poses:
  skin=np.tile(np.eye(4),(22,1,1))
  for j,(r,t) in enumerate(pose):skin[j,:3,:3]=r;skin[j,:3,3]=t
  g=skin@bind
  for a,b,c in [(14,15,16),(18,19,20)]:
   h,k,f=g[[a,b,c],:3,3];extensions.append(np.linalg.norm(f-h)/(np.linalg.norm(k-h)+np.linalg.norm(f-k)))
 report[name]={'frames':count,'loop_seam_over_largest_step':float(seam/steps.max()),'max_planted_foot_drift_per_sample_world_units':float(max(drift)),'max_leg_extension':float(max(extensions)),'floor_range_world_units':float(np.ptp(pp[:,:,1].min(1))*scale)}
 assert max(drift)<1,report[name];assert max(extensions)<.96;assert seam/steps.max()<1.2
(O/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
