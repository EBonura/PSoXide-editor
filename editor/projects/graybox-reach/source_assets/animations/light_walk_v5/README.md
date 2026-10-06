# Rust Mantis asymmetric walk v5

Adds a persistent 22-degree torso yaw, split into eight degrees at the waist and fourteen at the chest, toward the claw side in the approved weighted v4 walk. A 1.5-degree shoulder-roll bias, unequal shoulder motion across the two steps and a 20% alternating chest response break the mirrored upper-body rhythm. The neck counters fourteen degrees of the yaw to keep the gaze forward. Cannon/claw inertia from v4 remains.

Root and leg matrices match v4 exactly. Same footwork, contact timing, speed, 24 unique frames at 30 Hz and duplicate endpoint. Resource 33 selects assets/animations/light_walk_v5/walk.psxanim; idle and the 457-triangle body are unchanged.

walk.blend is the editable 22-bone source snapshot with packed textures. Frames 1–24 form the loop; frame25 is the duplicate closing key. Run author.py in Blender, validate.py with numpy, and save_walk.py in Blender to regenerate. Preserve manual edits before regeneration. export_blend.py exports manual bone-animation edits from the saved Blender file; copy the resulting PSXA into the runtime asset folder.

Validation checks source loop continuity, fractional matrix interpolation during contact, rig deformation and exact root/leg preservation against v4. Contact assumes steady forward movement at world speed1680 units/second, visual scale417 and in_place=false. The GIF comparison is a Blender preview at 800 milliseconds per loop.
