"""Key the authored enemy candidates onto the walk-v5 rig and save editable blends + GLBs."""
import bpy,json,sys
import numpy as np
from pathlib import Path
from mathutils import Matrix
from bpy_extras.anim_utils import action_get_channelbag_for_slot
OUT=Path(__file__).resolve().parent
G=OUT.parent
C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted()
want=sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else []
for npz in sorted(OUT.glob('*.npz')):
    name=npz.stem
    if want and not any(name.startswith(w) for w in want):continue
    bpy.ops.wm.open_mainfile(filepath=str(G/'light_walk_v5/walk.blend'))
    s=bpy.context.scene;rig=bpy.data.objects['Mantis body rig']
    rig.animation_data_clear();rig.hide_set(False);names=json.loads(rig['joint_order'])
    frames=np.load(npz)['skin'];last={}
    for fi,frame in enumerate(frames):
        s.frame_set(fi+1)
        for j,bone in enumerate(names):
            pb=rig.pose.bones[bone];pb.rotation_mode='QUATERNION'
            pb.matrix=C@Matrix(frame[j].tolist())@CI@pb.bone.matrix_local
            if bone in last and pb.rotation_quaternion.dot(last[bone])<0:pb.rotation_quaternion.negate()
            last[bone]=pb.rotation_quaternion.copy();bpy.context.view_layer.update()
            for ch in ['location','rotation_quaternion','scale']:pb.keyframe_insert(data_path=ch,frame=fi+1)
    act=rig.animation_data.action;act.name=f'Light enemy / knockdown / {name}';act.use_fake_user=True
    for fc in action_get_channelbag_for_slot(act,rig.animation_data.action_slot).fcurves:
        for key in fc.keyframe_points:key.interpolation='LINEAR'
    for a in list(bpy.data.actions):
        if a!=act:bpy.data.actions.remove(a)
    s.frame_start=1;s.frame_end=len(frames);s.render.fps=30
    rig['runtime_frames']=len(frames);rig['export_note']='30 Hz source at 1x runtime; last sample is the end sentinel.'
    bpy.context.preferences.filepaths.save_version=0
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT/f'{name}.blend'),copy=True)
    bpy.ops.object.select_all(action='DESELECT')
    for o in s.objects:
        if o.name in ('Mantis body rig','Mantis body 457'):o.hide_set(False);o.select_set(True)
    bpy.context.view_layer.objects.active=rig
    bpy.ops.export_scene.gltf(filepath=str(OUT/f'{name}.glb'),use_selection=True,export_animations=True,export_animation_mode='ACTIONS',export_force_sampling=True,export_frame_range=True)
    print('RESULT saved',name,len(frames),flush=True)
