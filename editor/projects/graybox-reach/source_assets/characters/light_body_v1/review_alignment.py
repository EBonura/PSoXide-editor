"""Same-pose, same-camera close-ups of the claw attachment before and after."""
from pathlib import Path
import bpy,struct,json
import numpy as np
from mathutils import Vector

O=Path(__file__).resolve().parent;P=O.parents[2];V=P/'validation/claw-alignment'
def read(path):
    b=path.read_bytes();jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12)
    po=28+4*jc+8*mc;vo=po+16*pc
    parts=[struct.unpack_from('<6H',b,po+16*i) for i in range(pc)]
    ids=[i for part in parts if part[0] in [8,9] for i in range(part[1],part[1]+part[2])]
    return [(x*.0001,-z*.0001,y*.0001) for x,y,z in [struct.unpack_from('<3h',b,vo+8*i) for i in range(vc)]],ids
before,ids=read(O/'light-before-claw-alignment.psxmdl')
after,_=read(P/'assets/models/light_body_v1/light.psxmdl')
bpy.ops.wm.open_mainfile(filepath=str(P/'source_assets/animations/light_heavy_v1/heavy.blend'))
s=bpy.context.scene;ob=bpy.data.objects['Mantis body 457'];cam=s.camera
s.render.resolution_x=960;s.render.resolution_y=960;s.render.resolution_percentage=100;s.eevee.taa_render_samples=16
def mesh(points):
    for v,p in zip(ob.data.vertices,points):v.co=p
    ob.data.update();ob.update_tag(refresh={'DATA'});bpy.context.view_layer.update()
for label,frame in [('neutral',1),('windup',30),('impact',37)]:
    s.frame_set(frame);points=[]
    for vertices in [before,after]:
        mesh(vertices);ev=ob.evaluated_get(bpy.context.evaluated_depsgraph_get());me=ev.to_mesh()
        points.extend([me.vertices[i].co.copy() for i in ids]);ev.to_mesh_clear()
    center=Vector(np.array([tuple(p) for p in points]).mean(0))
    cam.location=center+Vector((.7,-1,.20)).normalized()*7
    cam.rotation_euler=(center-cam.location).to_track_quat('-Z','Y').to_euler()
    rotation=cam.rotation_euler.to_matrix();inv=rotation.transposed()
    pp=np.array([tuple(inv@(p-center)) for p in points]);low,high=pp.min(0),pp.max(0)
    cam.location+=rotation@Vector(((low[0]+high[0])*.5,(low[1]+high[1])*.5,0))
    cam.data.ortho_scale=max(high[0]-low[0],high[1]-low[1])*1.60
    for name,vertices in [('before',before),('after',after)]:
        mesh(vertices);s.render.filepath=str(V/f'{label}-{name}.png');bpy.ops.render.render(write_still=True)
print('RESULT attachment comparison rendered')
