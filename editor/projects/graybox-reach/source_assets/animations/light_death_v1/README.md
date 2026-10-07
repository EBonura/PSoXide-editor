# Light enemy / death

66 intervals at 30 Hz (2.2 seconds), plus final pose and duplicate sentinel. One-shot Death resource 91; the runtime holds the last corpse pose. Uses the approved 22-joint, 457-triangle body and aligned claw.

| Frames | Beat |
|---|---|
| 0–10 | Fatal chest recoil, delayed head/arms; feet still planted |
| 10–24 | Hips sink, one knee gives way, torso tips toward claw side |
| 24–40 | Support fails and body falls sideways/forward |
| 40–50 | Ground contact and small overlapping limb settle |
| 50–58 | Head, claw and trailing legs come to rest |
| 58–66 | Motionless corpse; no return to guard |

Animation is authored with ground collision correction, not a runtime ragdoll. Root travel is visual within the death clip. Approved stun and unused HitReact remain unchanged. Regenerate via author.py, validate.py, save_blends.py -- --render, and preview.py. Editable Blender rig exports with export_blend.py -- death [path].
