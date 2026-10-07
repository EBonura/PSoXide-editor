"""Create editable packed rigs, audit the bake, render two-angle review frames."""
from pathlib import Path
import ast,json,struct,sys
import bpy
import numpy as np
from mathutils import Matrix,Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
OUT=Path(__file__).resolve().parent;P=OUT.parents[2]
REVIEW=P/'validation/light-reaction-v1';ROOT=P.parents[2]/'build/graybox-reach/light-reaction-v1/frames'
tree=ast.parse((P/'tools/enemy_reduction/validate_render.py').read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'model','deform'}],type_ignores=[]),'mesh helpers','exec'))
m=model(P/'assets/models/light_body_v1/light.psxmdl')
C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted()
meta=json.loads((OUT/'authoring.json').read_text());report=json.loads((REVIEW/'rig-validation.json').read_text()) if (REVIEW/'rig-validation.json').exists() else {}
for name in ['stun']:
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
    act=rig.animation_data.action;act.name=f'Light enemy / {name} / poise break and recovery';act.use_fake_user=True
    for fc in action_get_channelbag_for_slot(act,rig.animation_data.action_slot).fcurves:
        for key in fc.keyframe_points:key.interpolation='LINEAR'
        if meta[name]['loop']:fc.modifiers.new('CYCLES')
    s.frame_start=1;s.frame_end=meta[name]['frames']+1;s.render.fps=meta[name]['sample_hz']
    s.timeline_markers.clear()
    beats={'stun':[('Impact',1),('Recoil',4),('Catch step',9),('Fold',15),('Recover',25),('Guard',43)]}
    for label,f in beats[name]:s.timeline_markers.new(label,frame=f)
    rig['runtime_frames']=len(frames);rig['export_note']='30 Hz source at 1x runtime; end42 plus sentinel43; full recovery before AI resumes.'
    err=0.;all_vertices=[]
    for fi,frame in enumerate(frames):
        s.frame_set(fi+1);bpy.context.view_layer.update();ev=ob.evaluated_get(bpy.context.evaluated_depsgraph_get());me=ev.to_mesh()
        target=deform(m,[(a[:3,:3],a[:3,3]) for a in frame]);target=np.array([(C@Vector(tuple(v)+(1,)))[:3] for v in target])
        all_vertices.extend(target.tolist())
        err=max(err,float(np.linalg.norm(np.array([v.co[:] for v in me.vertices])-target,axis=1).max()));ev.to_mesh_clear()
    assert err<.001,(name,err)
    report[name]={'evaluated_mesh_max_error_blender_units':err,'samples_checked':len(frames)}
    rig.hide_set(True);s.frame_set(1);s.render.engine='BLENDER_EEVEE';s.eevee.taa_render_samples=8
    s.render.resolution_x=640;s.render.resolution_y=864;s.render.resolution_percentage=100
    s.render.image_settings.file_format='PNG';s.view_settings.view_transform='Standard';s.world.color=(.08,.095,.115)
    cam=s.camera;aim=Vector((-.23,-.45,-.2));cam.location=(4,-7,1.7);cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=6.4
    bpy.context.preferences.filepaths.save_version=0;bpy.ops.wm.save_as_mainfile(filepath=str(OUT/f'{name}.blend'))
    if '--render' in sys.argv or '--stills' in sys.argv:
        for view,pos in [('threequarter',(4,-7,1.7)),('side',(7,-.3,.8))]:
            cam.location=pos;cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler()
            # Fit the entire sweep, not just the resting character silhouette.
            rotation=cam.rotation_euler.to_matrix();inv=rotation.transposed()
            projected=np.array([tuple(inv@(Vector(v)-cam.location)) for v in all_vertices])
            low,high=projected.min(0),projected.max(0);center=(low+high)*.5
            cam.location+=rotation@Vector((center[0],center[1],0))
            cam.data.ortho_scale=max((high[1]-low[1])*1.10,(high[0]-low[0])*864/640*1.10,4.9)

            directory=ROOT/name/view;directory.mkdir(parents=True,exist_ok=True)
            for fi in (range(1,s.frame_end+1) if '--render' in sys.argv else sorted({1,4,len(frames)//2,len(frames)-3})):
                s.frame_set(fi);s.render.filepath=str(directory/f'{fi:03}.png');bpy.ops.render.render(write_still=True)
    print('RESULT',name,json.dumps(report[name]),flush=True)
(REVIEW/'rig-validation.json').write_text(json.dumps(report,indent=2))
