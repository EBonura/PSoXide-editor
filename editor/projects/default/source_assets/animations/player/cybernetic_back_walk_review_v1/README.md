# Backward walk - review v1

Candidate only, not installed. Built on the approved v7 Aletha rig, relaxed hands and 8-degree arm clearance. Explicit backward foot trajectories and analytical leg IK; this is not a reversed forward clip or a UniMate sample.

30 fps, frames 0–42, exact duplicate endpoint. Continuous in-place cycle, 1.4 seconds. Each foot has 62% support and 38% return, giving overlapping support. Nominal stride speed 0.438 m/s is a studio design parameter, not yet matched to the game motor.

Beat plan (the opposite leg repeats 21 frames later):

| Frame | Left foot / body |
|---|---|
| 0 | Backward toe contact; weight begins transferring |
| 5 | Sole settles, knee absorbs support |
| 13 | Body passes over planted foot |
| 22 | Frontward end of support, foot begins releasing |
| 26 | Lift-off; short backward return begins |
| 34 | Foot passes, 6.5 cm swing clearance |
| 42 | Reach back into the next toe contact |

Pelvis sway 2.4 cm peak-to-peak, bob 1.6 cm. Restrained counter-swing in the arms, quiet upright chest, soft knees. No root travel. The engine should preserve lateral weight transfer on eventual installation. Timing and moving contact still require in-game validation after approval.

`author.py` recreates the editable `walk_bwd.blend` and `walk_bwd.glb`. `render.py` generates the studio evidence in the ignored project review folder. `validation.json` records measurements.

Superseded by approved and installed backward walk v3.
