"""Transfer the original wake-up take onto Aletha's delivered rig.

Blender --background --factory-startup --python tools/aletha_intro.py -- PROJECT [SOURCE NAME]
The original take stays alongside the result under source_assets/animations/player.
"""
import sys
from pathlib import Path
import bpy
sys.path.insert(0, str(Path(__file__).resolve().parent))
import aletha_bvh_retarget as bridge
import json
import struct

def export(rig, action, frames, path):
    rig.animation_data.action = action
    for fc in action.fcurves:
        for k in fc.keyframe_points:
            k.interpolation = "LINEAR"
    scene = bpy.context.scene
    scene.frame_start, scene.frame_end = 0, frames - 1
    bpy.ops.object.select_all(action="DESELECT")
    rig.select_set(True)
    for child in rig.children_recursive:
        child.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.export_scene.gltf(
        filepath=str(path), export_format="GLB", use_selection=True,
        export_animations=True, export_animation_mode="ACTIVE_ACTIONS",
        export_frame_range=True, export_force_sampling=True,
        export_anim_single_armature=True, export_reset_pose_bones=True,
        export_optimize_animation_size=False,
    )
    # Blender's merged-action export otherwise calls every take "Animation",
    # causing the batch cooker to overwrite clips with the same output name.
    data = path.read_bytes()
    json_size = struct.unpack_from("<I", data, 12)[0]
    document = json.loads(data[20:20 + json_size])
    assert len(document["animations"]) == 1
    document["animations"][0]["name"] = action.name
    encoded = json.dumps(document, separators=(",", ":")).encode()
    encoded += b" " * (-len(encoded) % 4)
    tail = data[20 + json_size:]
    path.write_bytes(struct.pack("<4sII", b"glTF", 2, 20 + len(encoded) + len(tail))
                     + struct.pack("<I4s", len(encoded), b"JSON") + encoded + tail)


args = sys.argv[sys.argv.index('--') + 1:]
root = Path(args[0]).resolve()
source_path = Path(args[1]).resolve() if len(args) > 1 else root / 'source_assets/animations/player/wake_up_original.glb'
clip_name = args[2] if len(args) > 2 else 'aletha_wake_up'
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.fps = 30
bpy.ops.import_scene.gltf(filepath=str(root / 'source_assets/characters/aletha/Aletha.glb'))
target = next(o for o in scene.objects if o.type == 'ARMATURE')
target.animation_data_clear()
source = bridge.import_animation_source(source_path)
start, end = map(round, source.animation_data.action.frame_range)
action, _ = bridge.retarget(source, target, clip_name, start, end, smooth=False)
# Align the final standing pose once. Per-frame mesh grounding cancels the
# source's root motion and makes the body jump when a different limb is lowest.
meshes = [o for o in target.children_recursive if o.type == 'MESH']
scene.frame_set(end)
bpy.context.view_layer.update()
graph = bpy.context.evaluated_depsgraph_get()
floor_offset = min((o.matrix_world @ v.co).z
                   for o in [m.evaluated_get(graph) for m in meshes]
                   for v in o.data.vertices)
for frame in range(start, end + 1):
    scene.frame_set(frame)
    bpy.context.view_layer.update()
    hips = target.pose.bones['Hips']
    world = target.matrix_world @ hips.matrix
    world.translation.z -= floor_offset
    hips.matrix = target.matrix_world.inverted() @ world
    hips.keyframe_insert('location', frame=frame, group='Hips')
out = root / f'source_assets/animations/player/{clip_name}.glb'
out.parent.mkdir(parents=True, exist_ok=True)
export(target, action, end + 1, out)
print('INTRO', start, end, out)
