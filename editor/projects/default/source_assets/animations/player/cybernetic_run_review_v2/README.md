# Forward run v2: original-based refinement

Rejected candidate, never installed. Original run retained by user. User requested a new run resembling the original, retaining its whole-body forward lean and urgency. V1 was rejected for looking like an upright jog.

V2 reconstructs the original cooked Run clip 77 onto Aletha's existing skeleton. The frozen reference mesh and clip are in `reference/`. Original body pitch, pelvis translation, leg motion, shoulder rotation and 0.60-second cadence are retained. Edits add 3 degrees of elbow clearance and 8 percent neighboring-key wrist smoothing. An 18-frame cycle at 30 fps plus an exact duplicate endpoint replaces linear matrix interpolation with skeletal interpolation in this editable source.

The script checks that the body and leg matrices remain unchanged by the arm edits. The source loop has zero endpoint vertex displacement. Three angles are inspected at matching playback speeds; both presentations are floor-aligned. The reconstructed source's vertical calibration, runtime rebake and run start/stop transitions still require validation before installation.

Reproduce using Blender 5.2 in background with `--factory-startup --python-exit-code 1`: run `author.py`, `render.py`, then `render_original.py`. Run `encode.py` with Python/Pillow and ffmpeg available to produce `review/run-v2/run-v2-comparison.mp4`. Editable masters are `run_fwd.blend` and `run_fwd.glb`; checks and source hashes are in `validation.json`.
