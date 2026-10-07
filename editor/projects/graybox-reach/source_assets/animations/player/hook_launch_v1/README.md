# Hook launch

Source: Quaternius Universal Animation Library Standard, `Jump_Start` (CC0; see SOURCE-LICENSE.txt). Compared against Action Adventure Pack `jumping up.fbx`; selected the stronger crouch and raised-knee silhouette.

The first 11 source frames at 30 Hz are retimed to 50 Hz (0.2 seconds), retargeted to Aletha’s native 26-joint skeleton, and hold on the last frame during traversal. `hook-launch.blend` is the editable take. `bake.py` regenerates the native PSX animation from that file. Run with Blender 5.2 headless, factory startup and python-exit-code 1. `study.py -- --ual` reproduces the full candidate study from the local Downloads library; it writes a temporary `ual.blend`.

Runtime action: HookLaunch, appended as slot 41. The ordinary dash bindings are preserved. The runtime blends in over four ticks, anticipates for 12, bursts, flies after three mesh-capture ticks, and lands at tick 42. The FOV multiplier eases from baseline to a small anticipation squeeze, then a wider flight view, and returns to baseline after landing or interruption.
