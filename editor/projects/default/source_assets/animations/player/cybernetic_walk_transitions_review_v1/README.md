# Walk start and stops, review v1

Candidate set for WalkWindup (74), WalkWinddown (75) and WalkWinddownAlt (76). Not installed. The original run is retained after user review of both rejected run candidates.

The transitions are built around the approved walk's actual cooked poses and the current idle pose. Startup lasts 14 intervals at 30 fps (0.467 seconds), matching the current start duration. The body compresses slightly and shifts weight as the right foot steps into the walking pose. The two 24-interval stops (0.8 seconds) begin at the walk's start and half-cycle poses. The leading foot settles first, then the trailing foot closes to idle. Arms retain the approved walk clearance and settle with the body.

Separate foot trajectories and a two-bone solve avoid relying solely on an FK pose blend. Maximum leg reach is under 98.2 percent and foot-target error is under 0.1 mm at authored samples. These are rig checks, not proof of planted contacts at runtime.

`author.py` creates the three editable masters and GLB exports, plus joined source review sequences. `render.py` renders both stop variants in quarter view and variant A in side view; `encode.py` makes the comparison video. The reference folder freezes the current cooked mesh, idle, approved walk and original transition clips. Bone-name mapping comes from the project, and the rig comes from the approved walk master.

Review video: `review/walk-transitions-v1/walk-transitions-comparison.mp4`. Both panels use the same approved walk and approximate the engine's short crossfades. This is a studio preview. Runtime cooking, root calibration, arbitrary-phase stops and gameplay contact matching remain to be checked after approval. The installed project, animation assets and engine are unchanged.
