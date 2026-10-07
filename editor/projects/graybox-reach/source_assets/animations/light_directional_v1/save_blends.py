"""Save editable directional clips on the existing runtime rig, plus review frames."""
from pathlib import Path
import ast, json, math, struct, sys
import bpy
import numpy as np
from mathutils import Matrix, Vector
from bpy_extras.anim_utils import action_get_channelbag_for_slot
OUT=Path(__file__).resolve().parent
PROJECT=OUT.parents[2]
REVIEW=PROJECT/'validation/light-directional-v1'
FRAME_ROOT=PROJECT.parents[2]/'build/graybox-reach/light-directional-v1/preview-frames'
C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)))
CI=C.inverted()
source=PROJECT/'tools/enemy_reduction/validate_render.py'
tree=ast.parse(source.read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'model','animation','deform'}],type_ignores=[]),str(source),'exec'))
mesh_data=model(PROJECT/'assets/models/light_body_v1/light.psxmdl')


def main():
    results=json.loads((REVIEW/'rig-validation.json').read_text()) if (REVIEW/'rig-validation.json').exists() else {}
    for name in ['walk_backward','strafe_left','strafe_right']:
        if '--lateral-only' in sys.argv and name=='walk_backward':
            continue
        bpy.ops.wm.open_mainfile(filepath=str(PROJECT/'source_assets/animations/light_walk_v5/walk.blend'))
        scene=bpy.context.scene
        rig=bpy.data.objects['Mantis body rig']
        ob=bpy.data.objects['Mantis body 457']
        rig.animation_data_clear()
        names=json.loads(rig['joint_order'])
        data=np.load(OUT/f'{name}.npz')
        frames=np.concatenate([data['skin'],data['skin'][:1]])
        last={}
        for fi,frame in enumerate(frames):
            scene.frame_set(fi+1)
            for j,bone in enumerate(names):
                pb=rig.pose.bones[bone]
                pb.rotation_mode='QUATERNION'
                pb.matrix=C@Matrix(frame[j].tolist())@CI@pb.bone.matrix_local
                if bone in last and pb.rotation_quaternion.dot(last[bone])<0:
                    pb.rotation_quaternion.negate()
                last[bone]=pb.rotation_quaternion.copy()
                bpy.context.view_layer.update()
                for channel in ['location','rotation_quaternion','scale']:
                    pb.keyframe_insert(data_path=channel,frame=fi+1)
        action=rig.animation_data.action
        action.name=f'Mantis {name} v1'
        action.use_fake_user=True
        bag=action_get_channelbag_for_slot(action,rig.animation_data.action_slot)
        for fc in bag.fcurves:
            for key in fc.keyframe_points:key.interpolation='LINEAR'
            fc.modifiers.new('CYCLES')
        scene.frame_start=1
        scene.frame_end=len(frames)-1
        scene.render.fps=15
        scene.timeline_markers.clear()
        for label,f in [(('Following contact' if name.startswith('strafe') else 'Lead contact'),1),('Compression',3),(('Lead contact' if name.startswith('strafe') else 'Following contact'),(len(frames)-1)//2+1),('Compression',(len(frames)-1)//2+3)]:
            scene.timeline_markers.new(label,frame=f)
        rig['runtime_frames']=len(frames)
        rig['export_note']='15 Hz directional stalking cycle. Export includes duplicate endpoint. Runtime speed 0.5 units/tick; visual Q12 scale184; position cook divisor16.'
        rig['directional_clip']=name
        rig.hide_set(False)
        err=0.
        for fi in range(len(frames)):
            scene.frame_set(fi+1);bpy.context.view_layer.update()
            ev=ob.evaluated_get(bpy.context.evaluated_depsgraph_get());me=ev.to_mesh()
            target=deform(mesh_data,[(m[:3,:3],m[:3,3]) for m in frames[fi]])
            target=np.array([(C@Vector(tuple(v)+(1,)))[:3] for v in target])
            err=max(err,float(np.linalg.norm(np.array([v.co[:] for v in me.vertices])-target,axis=1).max()))
            ev.to_mesh_clear()
        assert err<.001,(name,err)
        results[name]={'rig_deformation_max_error':err,'frames':len(frames)-1,'joints':len(names)}
        rig.hide_set(True)
        scene.render.engine='BLENDER_EEVEE'
        scene.eevee.taa_render_samples=8
        scene.render.resolution_x=480
        scene.render.resolution_y=640
        scene.render.resolution_percentage=100
        scene.render.image_settings.file_format='PNG'
        scene.view_settings.view_transform='Standard'
        scene.world.color=(.08,.095,.115)
        cam=scene.camera
        cam.location=(4,-7,1.7)
        aim=Vector((-.23,0,-.15))
        cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler()
        cam.data.ortho_scale=4.65
        # Floor and lines make the contact phase readable. Only this headless
        # scene is modified; the live Blender session is never touched.
        bpy.ops.mesh.primitive_plane_add(size=200,location=(0,0,-2.01))
        floor=bpy.context.object;floor.name='Review floor'
        mat=bpy.data.materials.new('Review floor')
        bsdf=next(n for n in mat.node_tree.nodes if n.type=='BSDF_PRINCIPLED')
        bsdf.inputs['Base Color'].default_value=(.075,.095,.12,1)
        bsdf.inputs['Roughness'].default_value=.95
        floor.data.materials.append(mat)
        # Independent reusable export entry point, and a packed editable rig.
        scene.frame_set(1)
        bpy.context.preferences.filepaths.save_version=0
        bpy.ops.wm.save_as_mainfile(filepath=str(OUT/f'{name}.blend'))
        if '--skip-render' in sys.argv:
            continue
        for view,pos in [('threequarter',(4,-7,1.7)),('side',(7,-.3,.8))]:
            cam.location=pos;cam.rotation_euler=(aim-cam.location).to_track_quat('-Z','Y').to_euler()
            directory=FRAME_ROOT/name/view;directory.mkdir(parents=True,exist_ok=True)
            frames_to_render=range(1,len(frames)) if view=='threequarter' else [1,3,(len(frames)-1)//2+1,(len(frames)-1)//2+3]
            for fi in frames_to_render:
                scene.frame_set(fi)
                scene.render.filepath=str(directory/f'{fi:03}.png')
                bpy.ops.render.render(write_still=True)
    (REVIEW/'rig-validation.json').write_text(json.dumps(results,indent=2)+'\n')
    print('RESULT',json.dumps(results))

if __name__=='__main__':main()
