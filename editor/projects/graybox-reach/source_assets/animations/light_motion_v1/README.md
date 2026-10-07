# Light enemy motion pass

22-joint, 457-triangle light enemy, retaining the approved walk's upright hips, narrow knees, asymmetric torso and flat foot planes. No model or texture changes.

| Clip | Beat plan at 30 Hz | Duration |
|---|---|---|
| Run | Left contact 0, compression 2, release 4; right contact 10, compression 12, release 14; loop 20. Claw leads cannon by one sample. | 0.667 s |
| Turn | Weight transfer 2; left replant 2–10; centered 12; right replant 14–22; settle 24. | 0.8 s |
| Alert | Coil 0–9; lead foot steps 8–18; shoulder drives point 18–22; rear foot stays planted; hold 32–38; lead step returns 47–59; settle 60. | 2.0 s |

Run is calibrated for the current cooked chase speed of 6 engine units/tick, 360/second, at effective model Q12 scale 184. Position cooking divides by 16. Foot contact is linear in travel space, with a lifted bounded polynomial return. The game owns actor translation. Turn is a neutral adjustment cycle for either facing direction; the game owns yaw, so this clip adds no second root turn. Exact world-space foot locking during arbitrary runtime yaw changes is outside the asset's guarantees.

`author.py` generates PSXA v2 and NPZ source poses. `save_blends.py` puts them onto the existing packed Blender rig, verifies evaluated deformation, and renders review frames. `export_blend.py -- <name> [destination]` exports manual Blender edits. Regenerating overwrites authored poses; preserve manual revisions first. Run/turn include a duplicate loop endpoint; alert includes its final settled pose.

The full-body alert uses 120 reaction ticks (2.0 seconds) on both light-enemy character variants and the placed training controller. Hips lead the torso yaw, the gaze counters the torso, and the free arm settles late. The run and turn clips are unchanged.

The alert plants the leading foot 4800 model units forward and 850 outward, with 3400 clearance. The trailing foot keeps the same position and orientation throughout. The hips lead a larger shoulder wind-up; landing compression and a delayed free-arm sweep give the point weight. At least one foot supports the character throughout. Only the leading foot steps back during recovery, ending at the initial stance.
