# Light enemy / poise-break reaction

44 stored poses at 30 Hz: frames 0–42 plus a duplicate sentinel at 43. Resource 108 / Stun plays at 1x for a 1.4-second recoil/recovery. The cooker derives an 85-tick stun window, letting the final recovery pose appear before the AI resumes. Explicit Stun action options select authored timing; unconfigured enemies retain the legacy 3x / 32-tick behavior.

| Frame | Beat |
|---|---|
| 0–3 | Fast asymmetric chest recoil |
| 2–8 | One short backward catch step; rear foot remains planted |
| 8–14 | Head and arms follow; body folds into a low guard |
| 14–23 | Stunned hold |
| 24–37 | Shift weight back and replace the leading foot |
| 37–42 | Settle into the existing guard |

The claw/forearm attachment stays fixed. Arm rotations carry the whole chain; the elbow bends within its hinge plane. Model, light attack, heavy attack and movement files remain unchanged.

Run author.py, validate.py and save_blends.py in headless Blender, using -- --render for review frames. preview.py creates a 1080p preview at actual game speed. The editable rig and runtime both play at 30 Hz, 1x. export_blend.py -- stun [destination] exports the rig. Regeneration overwrites manual edits.
