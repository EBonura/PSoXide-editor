# Hook flight v2

Replaces the held raised-knee takeoff with an extended flight silhouette: nearly straight trailing legs, 24-degree torso lean, right arm reaching into the ascent, left arm swept back with slight follow-through. The existing launch crouch is retained briefly and extends by frame 12 of the 36-frame clip at 50 Hz. Runtime movement remains responsible for travel.

`author.py` reads the preserved v1 Blender take and writes the native animation plus editable `hook-flight.blend`. Run Blender 5.2 headless with factory startup and python-exit-code 1. `animation.json` records the leg-extension check; the in-game review is in `build/graybox-reach/hooks/flight-v2/` at repository root.
