# Crouched Rust Mantis stance v1

Uses the 457-triangle body and the original 22-bone skeleton. No geometry or material changes.

Idle lowers the pelvis 4300 model units, shifts it back 1300, distributes 30 degrees of forward bend through the spine, counters the neck by 18 degrees, raises the shoulder chains and uses two-bone IK for bent arms and legs. Feet are widened and staggered, with their transforms fixed throughout idle. Original upper-body idle motion is retained.

Walk builds on the saved `../light_walk_v1` draft. It adds a smaller 10-degree lean because the source walk is already pitched forward, lowers the pelvis by a further 1800 units, and preserves the draft's planted-foot trajectories. 24 unique samples at 30 Hz, 0.8 seconds, calibrated for world speed 1680 units/second (28 per 60 Hz tick) at visual scale 417. Runtime PSXA stores 25 samples including the duplicate endpoint required by looped playback. Idle has 96 unique samples at 12 Hz plus its duplicate endpoint.

`idle.blend` and `walk.blend` are editable rigged previews with packed palette textures and linear bone keys. Their separate scenes use the correct sample rates. Runtime clips are installed for resources 32 and 33 with in-place root subtraction disabled; neither clip translates the actor through the world. Other actions retain their existing authored poses.

Run `author.py` in Blender, `validate.py` with numpy, and `save_idle.py` / `save_walk.py` in Blender to regenerate. The authoring scripts regenerate snapshots and overwrite manual edits; preserve edited Blender files before rerunning. Runtime installation copies the generated PSXA files to `assets/animations/light_crouch_v1`.

Validation measures the decoded integer PSXA, including its actual endpoint-exclusive runtime loop. Contact checks assume straight, steady walking at the stated speed; collisions, turns and action transitions are outside that measurement.
