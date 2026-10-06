import bpy,bmesh,json,struct,collections,math
import numpy as np
from pathlib import Path
from mathutils import Vector
from mathutils.bvhtree import BVHTree
P=Path(__file__).resolve().parents[2];O=P/'source_assets/characters/enemy_reduced';O.mkdir(parents=True,exist_ok=True);D=P.parent/'default/assets/models';C=json.loads((Path(__file__).parent/'components.json').read_text());print('VERSION',bpy.app.version_string)
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
ratios={'light': {1: 0.7, 3: 0.75, 6: 0.6, 10: 0.85, 11: 0.65, 31: 0.8, 32: 0.95, 33: 0.65, 35: 0.75, 39: 0.85, 43: 0.95, 45: 0.65}, 'heavy': {1: 0.85, 2: 0.65, 7: 0.7, 8: 0.95, 10: 0.65, 14: 0.6, 15: 0.6, 16: 0.6, 17: 0.8, 18: 0.95, 21: 0.95, 28: 0.8, 30: 0.85, 32: 0.65, 35: 0.75, 36: 0.7, 41: 0.95, 42: 0.75, 43: 0.65, 48: 0.95, 24: 0.85}}
results={}
for name,rel in [('light','rust_mantis/rust_mantis.psxmdl'),('heavy','tank_boss_animated_model/tank_boss_animated_model.psxmdl')]:
 b=(D/rel).read_bytes();jc,pc,vc,fc,mc,tw,th,scale=struct.unpack_from('<8H',b,12);po=28+jc*4+mc*8;vo=po+pc*16;fo=vo+vc*8;parts=[struct.unpack_from('<6H',b,po+i*16) for i in range(pc)];verts=[struct.unpack_from('<3hBB',b,vo+i*8) for i in range(vc)];faces=[[struct.unpack_from('<HBB',b,fo+i*12+k*4) for k in range(3)] for i in range(fc)];owners={i:p[0] for p in parts for i in range(p[1],p[1]+p[2])};fowners={i:p[0] for p in parts for i in range(p[3],p[3]+p[4])};banks=b[fo+fc*12:];bank=lambda i:(banks[i//4]>>((i%4)*2))&3
 newverts=[];newfaces=[];changes=[];rejected=[]
 height=max(v[1] for v in verts)-min(v[1] for v in verts)
 for comp in C[name]:
  ids=comp['verts'];fs=comp['faces'];remap={i:k for k,i in enumerate(ids)};co=[verts[i][:3] for i in ids];ff=[[(remap[i],u,v) for i,u,v in faces[f]] for f in fs];pal=[bank(f) for f in fs];primary=[owners[i] for i in ids];secondary=[verts[i][3] for i in ids];blend=[verts[i][4] for i in ids];fprimary=[fowners[i] for i in fs]
  original=(co,ff,pal,primary,secondary,blend,fprimary)
  if name=='light' and comp['id']==7:
   # Remove only the inner recess wall: front inner-ring corners collapse to
   # matching rear-ring vertices. Outer lens housing and red disk are intact.
   mapping={}
   for k,(x,y,z) in enumerate(co):
    if z==5521:
     match=next((j for j,p in enumerate(co) if p==(x,y,5362)),None)
     if match is not None:mapping[k]=match
   assert len(mapping)==8
   kept=[];kp=[];ko=[]
   for f,p,owner in zip(ff,pal,fprimary):
    f=[(mapping.get(i,i),u,v) for i,u,v in f]
    if len(set(i for i,u,v in f))==3:kept.append(f);kp.append(p);ko.append(owner)
   ff,pal,fprimary=kept,kp,ko
  elif comp['id'] in ratios[name]:
   assert len(comp['weights'])==1 and len(set(fprimary))==1
   mesh=bpy.data.meshes.new('component');mesh.from_pydata([tuple(a/10000 for a in p) for p in co],[],[[c[0] for c in f] for f in ff]);uv=mesh.uv_layers.new(name='UVMap')
   for i in range(4):mesh.materials.append(bpy.data.materials.get(str(i)) or bpy.data.materials.new(str(i)))
   for p,f,palette in zip(mesh.polygons,ff,pal):
    p.material_index=palette
    for li,(_,u,v) in zip(p.loop_indices,f):uv.data[li].uv=(u/256,v/256)
   obj=bpy.data.objects.new('component',mesh);bpy.context.collection.objects.link(obj);bpy.context.view_layer.objects.active=obj;obj.select_set(True);mod=obj.modifiers.new('Local reduction','DECIMATE');mod.ratio=ratios[name][comp['id']];mod.use_collapse_triangulate=True;bpy.ops.object.modifier_apply(modifier=mod.name)
   co=[tuple(round(a*10000) for a in v.co) for v in obj.data.vertices];ff=[];pal=[];uv=obj.data.uv_layers.active
   for p in obj.data.polygons:
    assert len(p.vertices)==3
    ff.append([(obj.data.loops[li].vertex_index,round(uv.data[li].uv.x*256),round(uv.data[li].uv.y*256)) for li in p.loop_indices]);pal.append(p.material_index)
   j,sec,w=comp['weights'][0];primary=[j]*len(co);secondary=[sec]*len(co);blend=[w]*len(co);fprimary=[fprimary[0]]*len(ff);bpy.data.objects.remove(obj,do_unlink=True)
  if len(ff)!=len(fs):
   bvh0=BVHTree.FromPolygons([Vector(p) for p in original[0]],[[c[0] for c in f] for f in original[1]],all_triangles=True)
   bvh1=BVHTree.FromPolygons([Vector(p) for p in co],[[c[0] for c in f] for f in ff],all_triangles=True)
   def samples(points, faces):
    points=[Vector(p) for p in points];tri=[[c[0] for c in f] for f in faces]
    return points+[sum((points[i] for i in f),Vector())/3 for f in tri]+[(points[f[j]]+points[f[(j+1)%3]])/2 for f in tri for j in range(3)]
   err=max([bvh1.find_nearest(p)[3] for p in samples(original[0],original[1])]+[bvh0.find_nearest(p)[3] for p in samples(co,ff)])/height*100
   if err>1.2:
    rejected.append({'component':comp['id'],'max_surface_error_percent_height':err,'would_save':len(fs)-len(ff)})
    co,ff,pal,primary,secondary,blend,fprimary=original
  used=sorted({i for f in ff for i,u,v in f});local={i:len(newverts)+k for k,i in enumerate(used)}
  for i in used:newverts.append({'p':co[i],'owner':primary[i],'sec':secondary[i],'w':blend[i]})
  for f,p,owner in zip(ff,pal,fprimary):newfaces.append({'corners':[(local[i],u,v) for i,u,v in f],'bank':p,'owner':owner})
  if len(ff)!=len(fs):changes.append({'component':comp['id'],'joint':comp['weights'],'before':len(fs),'after':len(ff),'saved':len(fs)-len(ff)})
 # Keep the original skeleton, material table, scale and part order exactly.
 vout=[];fout=[];pout=[];mapping={}
 for pi,p in enumerate(parts):
  j=p[0];start=len(vout)
  for i,v in enumerate(newverts):
   if v['owner']==j:mapping[i]=len(vout);vout.append(v)
  pout.append([j,start,len(vout)-start,0,0,p[5]])
 for p in pout:
  p[3]=len(fout);fout += [f for f in newfaces if f['owner']==p[0]];p[4]=len(fout)-p[3]
 out=bytearray(b[:po]);struct.pack_into('<H',out,16,len(vout));struct.pack_into('<H',out,18,len(fout))
 for i,p in enumerate(pout):out+=struct.pack('<6H',*p)+b[po+i*16+12:po+i*16+16]
 for v in vout:out+=struct.pack('<3hBB',*v['p'],v['sec'],v['w'])
 for f in fout:
  for i,u,v in f['corners']:out+=struct.pack('<HBB',mapping[i],min(255,max(0,u)),min(255,max(0,v)))
 packed=bytearray((len(fout)+3)//4)
 for i,f in enumerate(fout):packed[i//4]|=f['bank']<<((i%4)*2)
 out+=packed;struct.pack_into('<I',out,8,len(out)-12);(O/f'{name}-reduced.psxmdl').write_bytes(out)
 # Save an editable mesh with runtime weights, IDs and UVs, in PSMD coordinates.
 mesh=bpy.data.meshes.new(name+' reduced');mesh.from_pydata([tuple(a/10000 for a in v['p']) for v in vout],[],[[mapping[i] for i,u,v in f['corners']] for f in fout]);obj=bpy.data.objects.new(name+' reduced',mesh);bpy.context.collection.objects.link(obj)
 for j in range(jc):obj.vertex_groups.new(name='joint_'+str(j))
 for i,v in enumerate(vout):
  obj.vertex_groups[v['owner']].add([i],(255-v['w'])/255,'REPLACE')
  if v['w']:obj.vertex_groups[v['sec']].add([i],v['w']/255,'ADD')
 uv=mesh.uv_layers.new(name='RuntimeUV')
 for p,f in zip(mesh.polygons,fout):
  for li,(_,u,v) in zip(p.loop_indices,f['corners']):uv.data[li].uv=(u/256,1-v/256)
 obj['psmd_units_per_blender_unit']=10000;obj['skeleton_joint_count']=jc
 results[name]={'before_triangles':fc,'triangles':len(fout),'before_vertices':vc,'vertices':len(vout),'joints':jc,'changes':changes,'rejected':rejected}
(O/'reduction.json').write_text(json.dumps(results,indent=2));bpy.ops.wm.save_as_mainfile(filepath=str(O/'enemy-reduction-editable.blend'));print('RESULT',json.dumps(results))
