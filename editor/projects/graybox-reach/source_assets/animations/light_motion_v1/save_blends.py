"""Create editable packed rigs, audit the bake, render two-angle review frames."""
from pathlib import Path
import ast,json,struct,sys
import bpy
import numpy as np
from mathutils import Matrix,Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
OUT=Path(__file__).resolve().parent;P=OUT.parents[2]
REVIEW=P/'validation/light-motion-v1';ROOT=P.parents[2]/'build/graybox-reach/light-motion-v1/frames'
tree=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'model','deform'}],type_ignores=[]),'mesh helpers','exec'))
m=model(P/'assets/models/light_body_v1/light.psxmdl')
C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted()
meta=json.loads((OUT/'authoring.json').read_text());report=json.loads((REVIEW/'rig-validation.json').read_text()) if (REVIEW/'rig-validation.json').exists() else {}
for name in ['run','turn','alert']:
    if '--alert-only' in sys.argv and name!='alert':continue
    bpy.ops.wm.open_mainfile(filepath=str(P/'source_assets/animations/light_walk_v5/walk.blend'))
    s=bpy.context.scene;rig=bpy.data.objects['Mantis body rig'];ob=bpy.data.objects['Mantis body 457']
    rig.animation_data_clear();rig.hide_set(False);names=json.loads(rig['joint_order'])
    frames=np.load(OUT/f'{name}.npz')['skin'];last={}
    for fi,frame in enumerate(frames):
        s.frame_set(fi+1)
        for j,bone in enumerate(names):
            pb=rig.pose.bones[bone];pb.rotation_mode='QUATERNION'
            pb.matrix=C@Matrix(frame[j].tolist())@CI@pb.bone.matrix_local
            if bone in last and pb.rotation_quaternion.dot(last[bone])<0:pb.rotation_quaternion.negate()
            last[bone]=pb.rotation_quaternion.copy();bpy.context.view_layer.update()
            for channel in ['location','rotation_quaternion','scale']:pb.keyframe_insert(data_path=channel,frame=fi+1)
    act=rig.animation_data.action;act.name=f'Light enemy / {name} / motion v1';act.use_fake_user=True
    for fc in action_get_channelbag_for_slot(act,rig.animation_data.action_slot).fcurves:
        for key in fc.keyframe_points:key.interpolation='LINEAR'
        if meta[name]['loop']:fc.modifiers.new('CYCLES')
    s.frame_start=1;s.frame_end=len(frames)-int(meta[name]['loop']);s.render.fps=30
    s.timeline_markers.clear()
    beats={'run':[('Left contact',1),('Compression',3),('Flight',6),('Right contact',11),('Compression',13),('Flight',16)],'turn':[('Transfer',3),('Left lift',7),('Center',13),('Right lift',19),('Settle',25)],'alert':[('Notice',1),('Coil',10),('Lead foot plant',19),('Point',22),('Read',34),('Lead recovery',48),('Settle',61)]}
    for label,f in beats[name]:s.timeline_markers.new(label,frame=f)
    rig['runtime_frames']=len(frames);rig['export_note']='30 Hz; motion v1. Run/turn include loop endpoint; alert ends on settled pose.'
    err=0.
    for fi,frame in enumerate(frames):
        s.frame_set(fi+1);bpy.context.view_layer.update();ev=ob.evaluated_get(bpy.context.evaluated_depsgraph_get());me=ev.to_mesh()
        target=deform(m,[(a[:3,:3],a[:3,3]) for a in frame]);target=np.array([(C@Vector(tuple(v)+(1,)))[:3] for v in target])
        err=max(err,float(np.linalg.norm(np.array([v.co[:] for v in me.vertices])-target,axis=1).max()));ev.to_mesh_clear()
    assert err<.001,(name,err)
    report[name]={'evaluated_mesh_max_error_blender_units':err,'samples_checked':len(frames)}
    rig.hide_set(True);s.frame_set(1);s.render.engine='BLENDER_EEVEE';s.eevee.taa_render_samples=8
    s.render.resolution_x=640;s.render.resolution_y=864;s.render.resolution_percentage=100
    s.render.image_settings.file_format='PNG';s.view_settings.view_transform='Standard';s.world.color=(.08,.095,.115)
    cam=s.camera;aim=Vector((-.23,-1.1 if name=='alert' else 0,-.2));cam.location=(4,-7,1.7);cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=5.7 if name=='alert' else 4.9
    bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(OUT/f'{name}.blend'))
    if '--render' in sys.argv or '--stills' in sys.argv:
        for view,pos in [('threequarter',(4,-7,1.7)),('side',(7,-.3,.8))]:
            cam.location=pos;cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler()
            directory=ROOT/name/view;directory.mkdir(parents=True,exist_ok=True)
            for fi in (range(1,s.frame_end+1) if '--render' in sys.argv else sorted({1,4,len(frames)//2,len(frames)-3})):
                s.frame_set(fi);s.render.filepath=str(directory/f'{fi:03}.png');bpy.ops.render.render(write_still=True)
    print('RESULT',name,json.dumps(report[name]),flush=True)
(REVIEW/'rig-validation.json').write_text(json.dumps(report,indent=2))
