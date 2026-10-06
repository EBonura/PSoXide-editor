# Rust Mantis body study v1

457 triangles, 410 vertices, 22 original joints. Graybox Reach only.

The 438-triangle mesh is retained in `../enemy_reduced`. This pass tapers and raises the lower chest, folds the front into two breast plates with a recessed centre and chevron hem, narrows the waist and pelvis, adjusts the head shell, sweeps the upper-arm armour and tapers the asymmetric thigh plates. Original rounder elbow housings are restored (+15 triangles); chest folds cost four triangles. Cannon, claw, feet, skeleton and animations are preserved. The shared 4-bit atlas is unchanged.

`light-body.blend` has the rest mesh, packed atlas palettes, UVs, runtime weights, a 22-bone armature and the original idle action for inspection. Select the mesh and enter Edit Mode to work on the body. Unhide the armature to pose it. No subdivision is required.

Regenerate from the retained 438-triangle baseline with `shape.py`, then `detail.py` using Python with numpy. Run `save_blend.py` in Blender to rebuild the editable snapshot. Rebuilding the snapshot overwrites manual Blender edits; archive edited work before regeneration. The Blender snapshot is not an automatic reimport source.

The separate walk draft remains in `source_assets/animations/light_walk_v1` and has not been installed.

Installed runtime assets are `assets/models/light_body_v1/{light,light_clawless}.psxmdl`, selected by resources 26 and 111. The clawless variant disables part 9 and draws 411 triangles. The retained reduction assets are unchanged. Validation: 330 animation frames, no duplicate/zero-area triangles, original cannon/claw/feet face data preserved, project disc build passed, zero MIPS load-delay hazards, scratchpad stack checks passed. Blender previews are in `validation/light-body-v1`; they are not in-game screenshots.
