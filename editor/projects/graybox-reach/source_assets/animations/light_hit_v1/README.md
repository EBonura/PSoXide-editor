# Light enemy / hit reaction

A 0.9-second asymmetric recoil at 30 Hz, 1x playback. 27 intervals plus the final guard pose and a duplicate sentinel: 29 stored poses. Shares the approved neutral guard and corrected claw attachment. Both feet remain planted; a small pelvis shift and soft knees absorb the impact.

| Frames | Beat |
|---|---|
| 0–4 | Chest snaps away from the hit; no anticipation |
| 4–8 | Chest recoil holds while head and claw follow |
| 8–14 | Pelvis absorbs the force; head finds the player again |
| 14–22 | Torso unwinds and claw returns toward guard |
| 22–27 | Small overlapping settle into the original stance |

Resource 92 / HitReact. The accepted Stun108 stays untouched. Current enemy runtime only triggers Stun on a poise break, using HitReact as a fallback when Stun is absent; it does not trigger a separate animation for an ordinary non-poise-breaking hit. This clip is authored and bound for editor review; normal-hit playback is a separate gameplay integration decision.

Rebuild with author.py, validate.py, save_blends.py -- --render in headless Blender. preview.py makes the two-view 1080p review. export_blend.py -- hit [path] exports the saved rig. Regeneration overwrites manual edits.
