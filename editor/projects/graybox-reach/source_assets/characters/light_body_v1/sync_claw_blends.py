"""Refresh active editable meshes from runtime geometry; keep actions intact."""
from pathlib import Path
import bpy,struct,json

O=Path(__file__).resolve().parent;P=O.parents[2]
b=(P/'assets/models/light_body_v1/light.psxmdl').read_bytes()
jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);vo=28+jc*4+mc*8+pc*16
points=[struct.unpack_from('<3h',b,vo+i*8) for i in range(vc)]
paths=[O/'light-body.blend']
for folder in ['light_walk_v5','light_stalk_v2','light_directional_v1','light_motion_v1','light_attack_v2','light_heavy_v1']:
    paths.extend((P/'source_assets/animations'/folder).glob('*.blend'))
bpy.context.preferences.filepaths.save_version=0
for path in paths:
    bpy.ops.wm.open_mainfile(filepath=str(path))
    ob=bpy.data.objects.get('Mantis body 457')
    assert ob and len(ob.data.vertices)==vc,str(path)
    for v,(x,y,z) in zip(ob.data.vertices,points):v.co=(x*.0001,-z*.0001,y*.0001)
    ob.data.update()
    bpy.ops.wm.save_as_mainfile(filepath=str(path))
print('RESULT',json.dumps({'mesh_snapshots_refreshed':len(paths),'animation_keys':'unchanged'}))
