import bpy,sys,json,struct
import numpy as np
from pathlib import Path
from mathutils import Matrix,Vector
O=Path(__file__).resolve().parent;P=O.parents[3];sys.path.insert(0,str(O.parent/'cybernetic_walk_transitions_review_v4'));from decode_clip import decode
report={}
for label in ['poise','hit']:
 bpy.ops.wm.open_mainfile(filepath=str(O/(label+'.blend')));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');mesh=bpy.data.objects['Aletha optimized'];world=r.matrix_world.copy();names=[b.name for b in r.data.bones];rest={n:world@r.data.bones[n].matrix_local for n in names}
 blob=(P/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes();j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16;raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float);verts=np.array([mesh.matrix_world@v.co for v in mesh.data.vertices]);C=Matrix(((1,0,0),(0,0,-1),(0,1,0)));rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=np.ptp(verts[:,2])/np.ptp(rv[:,2]);center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())
 R,T,n,hz=decode(O/(label+'.psxanim'));assert hz==60 and n==s.frame_end+2 and j==26
 maxerr=0.;motion=[];floor=1e9;foot_positions={'Left':[],'Right':[]};minheight=1e9
 for f in np.arange(0,s.frame_end+.001,.25):
  s.frame_set(int(f),subframe=float(f%1));dg=bpy.context.evaluated_depsgraph_get();ev=mesh.evaluated_get(dg);me=ev.to_mesh();vs=np.array([ev.matrix_world@v.co for v in me.vertices]);ev.to_mesh_clear()
  if f==0:floor=float(vs[:,2].min());first=vs.copy()
  minheight=min(minheight,float(vs[:,2].min()-floor))
  for side in foot_positions:foot_positions[side].append(np.array((world@r.pose.bones[side+'Foot'].matrix).translation))
  if f%1==0:
   for i,name in enumerate(names):
    rr=C@Matrix(R[int(f),i].tolist())@C.transposed();skin=rr.to_4x4();skin.translation=C@Vector(T[int(f),i].tolist())*unit+center-rr@center
    actual=(world@r.pose.bones[name].matrix)@rest[name].inverted();maxerr=max(maxerr,float(np.max(np.abs(np.array(skin)-np.array(actual)))))
  motion.append(float(np.max(np.linalg.norm(vs-first,axis=1))))
 drifts={side:float(np.max(np.linalg.norm(np.array(pos)-pos[0],axis=1))) for side,pos in foot_positions.items()}
 assert maxerr<.001,(label,maxerr)
 # Blender interpolates local joint rotations; the game interpolates baked
 # skin transforms. Check exact constant foot transforms in the shipped format.
 pinned=['Right'] if label=='poise' else ['Left','Right']
 for side in pinned:
  idx=names.index(side+'Foot');assert np.max(np.abs(R[:,idx]-R[0,idx]))==0
  assert np.max(np.abs(T[:,idx]-T[0,idx]))*unit<.0002 # below 0.2 mm after export rounding
  assert drifts[side]<.001,(label,drifts) # less than 1 mm preview-only FK deviation

 assert minheight>-.001,(label,minheight)
 assert motion[-1]<.001,(label,motion[-1])
 report[label]={'stored_poses':n,'hz':hz,'duration':(n-2)/hz,'max_export_matrix_error':maxerr,'subframe_floor_penetration_m':max(0,-minheight),'foot_travel_m':drifts,'end_pose_max_vertex_error_m':motion[-1],'max_vertex_excursion_m':max(motion),'sampled_poses':len(motion)}
(P/'validation/player-reactions-v1/validation.json').write_text(json.dumps(report,indent=2));print('RESULT',json.dumps(report))
