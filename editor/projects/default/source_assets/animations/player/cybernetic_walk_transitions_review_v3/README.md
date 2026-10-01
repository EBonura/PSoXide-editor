# Walk transitions v3: continuous stride

Review candidate, not installed. The user identified a limp or pause at the start and end of v2. Its startup eased into a fixed walk pose and the preview then blended from a frozen frame. The stops also used two separate foot adjustments.

V3 accelerates an advancing walk phase through startup and reaches the approved walk at its moving phase-zero pose. Its final four startup frames follow the gait directly. Stops decelerate the walk phase into idle without separate foot-lift paths. Body compression remains tied to the stride, with two-bone leg solving to preserve the intended foot positions while loading the torso.

Startup lasts 0.467 seconds. Both foot-specific stops last 0.6 seconds. The approved walk and idle provide the endpoint poses. Frozen cooked references remain in `reference/`.

The studio comparison shows v2 on the left and v3 on the right, side view first, then quarter views of both stop variants. The audit samples foot travel across the startup handoff. V2 has a practically stationary frame; v3 left-foot travel remains approximately 6-7 cm per frame and right-foot travel approximately 3 cm per frame across that join. These measurements concern the studio pose sequence, not world-space contact in the game.

Run `author.py` and `render.py` with Blender 5.2 in background, using `--factory-startup --python-exit-code 1`. Run `encode.py` with Python/Pillow and ffmpeg, then `audit.py` with Blender. `validation.json` records source hashes, pose solving, render checks, and handoff measurements. The comparison is `review/walk-transitions-v3/walk-transitions-v3-comparison.mp4`.

The preview uses a direct matched start-to-walk join. The engine currently crossfades that transition over eight ticks, so installing these sources requires validating or adjusting that handoff. Runtime cooking, root calibration, arbitrary-phase release and foot contact still require validation. Clips 74, 75 and 76 remain original in the game.
