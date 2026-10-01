# Walk transitions v2: more weight and follow-through

Review candidate, not installed. The user liked v1, then asked for more. This pass retains all three durations and exact endpoint poses while increasing the body motion within each transition.

Added load is 6 cm at startup and 7.5 cm in the stops, reaching its peak early in the action. The pelvis shifts forward by 3.5-4.5 cm and transfers weight laterally; hips and spine pitch into the move, then chest, head and arms settle with staggered timing. The feet keep separate trajectories and a two-bone solve. The leading startup foot clears 8.5 cm above its endpoint interpolation. These are authored offsets; they are not runtime movement pushes.

Durations remain 0.467 seconds for start and 0.8 seconds for either stop. The approved walk and idle endpoints are preserved. `audit.py` compares evaluated endpoint mesh vertices with v1 and samples intermediate poses at half frames. `validation.json` records target accuracy, reach, framing, endpoint differences and rotation steps. Runtime contact matching is still pending.

Run `author.py`, `render.py` and `audit.py` with Blender 5.2 in background, with `--factory-startup --python-exit-code 1`. Run `encode.py` using Python/Pillow and ffmpeg. The video compares v1 on the left and v2 on the right using v1's saved render frames: `review/walk-transitions-v2/walk-transitions-v2-comparison.mp4`.

This is a studio source review with approximate crossfades. No clips are installed, and runtime cooking, root calibration and arbitrary-phase release still require validation.
