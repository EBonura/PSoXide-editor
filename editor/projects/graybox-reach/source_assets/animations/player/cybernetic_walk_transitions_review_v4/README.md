# Walk transitions v4: readable preparation and two-foot settle

Approved and installed in the default Cortex Ignition 0.4b project. The user found v3 smoother but too immediate and missed the two-foot adjustment at the stop.

Startup now lasts 20 frames (0.667 seconds), with a longer weight transfer before the stride reaches full speed. The last five frames still follow the moving approved walk, preserving v3's continuous handoff.

Both stops last 28 frames (0.933 seconds). The first foot carries its incoming velocity into a braking step and finishes placing at frame 14. The second foot begins adjusting at frame 12 and reaches the idle stance at frame 26. Body loading and lateral transfer continue across the two placements, then settle over the last two frames. The alternate stop reverses foot order. See `BEATS.md`.

Validation: all solved leg reaches remain below 0.982; maximum foot target error is below 0.25 mm. At the startup handoff, foot displacement matches v3 without a frozen frame. Once the first foot lands, its position drifts less than 0.05 mm while the second adjusts through an 18-23 cm path. These are studio rig measurements; foot orientation, surface contact and arbitrary-phase runtime release still need integration validation. Rendered side and quarter views and a dedicated stop frame sheet were inspected.

Run `author.py`, then `render.py`, with headless Blender 5.2 using `--factory-startup --python-exit-code 1`. Run `encode.py` with Python/Pillow and ffmpeg, followed by `audit.py` with Blender. The video uses saved v3 render frames on the left and v4 on the right. Six frames of initial idle padding align the walk and stop cues without retiming either animation.

## Installation

Clips 74, 75 and 76 now use the approved v4 masters, recorded as sources 212, 213 and 214. Both clip calibration and action options retain authored translation (`in_place: false`). The source meshes and GLB exports are unchanged from approval.

Rebake with headless Blender running `bake.py`, then `target/release/frontend build-project-disc --project editor/projects/default/project.ron` from the repository root. Use this direct matrix bake: generic GLB retargeting reapplies bind offsets to these reconstructed poses and shifts joints away from the approved result. `bake.py` reverses the authoring coordinate transform against the frozen cooked model. Source assets use sixteen times its runtime coordinate scale, checked against the unchanged approved walk.

The runtime sampler reserves the final stored sample for looping. Each bake therefore appends one identical terminal sample so the approved endpoint remains reachable. `preserve_samples: true` bypasses trimming and rate reduction for these three clips only. This keeps 22/30/30 stored samples at 30 Hz, including the terminal sentinel. Actions run at Q8 speed 256.

The walk windup hands off at its endpoint tick, with no endpoint hold or crossfade. It lasts 40 simulation ticks (0.667 seconds). Each stop reaches idle at tick 56 (0.933 seconds) and its state finishes at tick 58. The ordinary short gait-to-stop blend and stop interruption remain available.

Validation passed: 459 project tests, 82 playtest tests, exact approved source hashes, preserved cooked samples and terminal poses, a start-to-walk seam with identical joint translations and at most 1/4096 rotation coefficient error, zero mapped load-delay hazards, and native repeated movement/release playback through 3101 controller polls without faults. The disc and editor frontend were rebuilt. Both stop variants were inspected from cooked poses. See `runtime-validation.json` for hashes and scope. Hardware and comprehensive world-space foot-contact validation remain unverified.
