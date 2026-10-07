# Light enemy / backward death

Replaces the rigid forward tip with an authored failed backstep and a constrained, offline physics collapse. The hips descend, the back lands, and head/arms settle with distinct timing. The current 22-joint model and corrected claw attachment remain unchanged.

30 Hz at 1x, 86 stored poses: frames 0–84 plus a duplicate sentinel. Motion settles by frame 56 (1.87 seconds); the rest of the 2.8-second clip holds the corpse exactly. Death resource 91 uses the existing runtime Dead state. No runtime ragdoll or combat-rule changes.

| Frames | Beat |
|---|---|
| 0–7 | Fatal recoil |
| 5–18 | One failed backstep; torso falls behind the support |
| 19–33 | Support fails; limbs follow the falling hips |
| 34–44 | Back and shoulders make contact |
| 44–56 | Head and arms settle asynchronously |
| 56–84 | Completely still corpse |

`author.py` makes the animation and an editable physics-source.blend; `save_blends.py -- --render` saves the packed character rig and renders two views. `export_blend.py -- death [path]` round-trips that rig. `validate.py` audits decoded geometry, fractional frame contacts, claw alignment and leg clearance. `preview.py` produces the 1080p full-speed video.

The bake projects solver drift back onto fixed joint anchors, corrects tiny ground penetration, and eases the tail into a reviewed rest pose before the rigid leg fins overlap. The guest plays ordinary PSXA keyframes. Regeneration overwrites manual edits.
