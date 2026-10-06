# Rust Mantis walk v3

Rebuilt around the approved light_stalk_v2 idle pose. The body mesh, idle clip and gameplay movement speed are unchanged.

24 unique frames at 30 Hz (0.8 seconds), plus a duplicate endpoint for runtime looping. Each foot plants for 56% of its cycle, with 12% total double support. Feet use narrow forward tracks, 1100 model-unit clearance (half of v2), constant backwards velocity while planted, and Hermite swing paths with matching endpoint velocity. Knee poles stay forward. The pelvis has small periodic weight shifts and counter-rotation; the chest stays closer to the approved stance. Claw swing is larger than cannon swing, with a small lag. A constant 643.26 model-unit pelvis adjustment keeps maximum leg extension at 95.5% without per-frame IK height pops.

Runtime resource 33 selects `assets/animations/light_walk_v3/walk.psxanim`. In-place root subtraction remains disabled. Contact is calibrated for world speed 1680 units/second (28 per 60 Hz tick), visual scale 417. Other speeds, terrain and turns need separate runtime assessment.

`walk.blend` contains the editable 22-bone rig and packed textures. Frames 1–24 are the unique loop; frame 25 duplicates frame 1 so cyclic curves span the full period. Regenerate with author.py in Blender, validate.py with numpy, then save_walk.py in Blender. The author uses v2 idle for the base pose and the retained light_walk_v1 rig snapshot for planted foot orientations. Preserve manual edits before rerunning scripts, which regenerate snapshots. Copy the regenerated PSXA to the runtime asset folder to install it.

Validation checks the decoded integer animation, the endpoint-exclusive runtime loop and fractional matrix interpolation during contact. Previous/current previews use the same mesh, camera, frame rate and ground travel; they are Blender previews, not gameplay captures.

To export manual bone-animation edits, save walk.blend and run export_blend.py in Blender, then copy walk.psxanim into the runtime asset folder. A saved-file export round trip was checked against all 25 poses. Keep the duplicate endpoint and the 30 Hz sample rate. The comparison GIF uses 30/30/40 ms frame durations so the full cycle is exactly 800 ms.
