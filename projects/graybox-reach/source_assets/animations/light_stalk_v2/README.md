# Rust Mantis stalking stance v2

Replaces the deep, wide crouch in v1. The 457-triangle mesh and cannon are unchanged.

Idle: hips are raised 2100 model units from v1 and shifted forward; the feet are staggered, with a bent lead leg and a longer rear leg. Knee poles follow the legs more closely. The spine leans forward 25.5 degrees relative to the source, with 6 degrees of chest twist. Shoulder elevation and hand targets are asymmetric. Feet stay fixed while the source upper-body idle motion plays.

Walk: pelvis lowered 1000 units from the saved light_walk_v1 draft, narrower stance, 5 degrees of additional lean, asymmetric arms. Existing planted foot paths and 0.8-second cycle are retained. 24 unique frames at 30 Hz plus a duplicate loop endpoint. Idle has 96 unique frames at 12 Hz plus its endpoint. Resources 32/33 select the v2 clips; in-place subtraction remains disabled.

Editable Blender files: idle.blend and walk.blend, with the 22-bone runtime skeleton, original weights and packed atlas. Regenerate using author.py in Blender, validate.py with numpy, then save_idle.py / save_walk.py in Blender. Regeneration overwrites manual snapshot edits. Copy PSXA results to assets/animations/light_stalk_v2 to install them. Other animation actions retain their existing poses.

Contact checks assume straight walking at 28 world units per 60 Hz tick, visual scale 417. The retained v1 files provide the previous comparison pose.
