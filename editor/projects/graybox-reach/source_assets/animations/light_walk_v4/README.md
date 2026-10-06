# Rust Mantis weighted walk v4

Adds body weight and overlapping motion to v3. Same 24 unique frames / 30 Hz / 0.8-second cycle, plus a duplicate endpoint. Same gameplay speed, foot tracks, clearance and stance timing. Resource 33 now selects assets/animations/light_walk_v4/walk.psxanim. Idle and the 457-triangle mesh are unchanged.

Timing uses zero-based samples: foot contact at 0/12, pelvis compression at 2/14, recovery around 8/20. Periodic Hermite keys make the drop faster than the recovery. Hip travel is 1750 units to either side, pelvis yaw 4 degrees, delayed opposing chest yaw 6 degrees, shoulder roll 4 degrees and a 3.5-degree impact fold. Neck counter-motion follows another frame later. Claw position follows the shoulder by one sample; the cannon follows by two, producing visible inertia. IK retains the planted foot transforms and caps leg extension at 95.5%.

Editable source: walk.blend, unique frames 1–24 with duplicate closing key at frame 25. The original 22-bone skeleton, weights and packed atlas are retained. author.py regenerates the PSXA/NPZ; validate.py checks the decoded source and fractional foot contact; save_walk.py rebuilds the Blender snapshot. export_blend.py exports saved manual bone edits. Preserve manual edits before regeneration. After export, copy walk.psxanim to the runtime asset folder.

Contact calibration is world speed 1680 units/second (28 per 60 Hz tick), visual scale 417, in_place=false. Straight steady movement is covered; turning, terrain and action transitions require gameplay review. Previous/current GIFs are Blender previews at exactly 800 ms per loop.
