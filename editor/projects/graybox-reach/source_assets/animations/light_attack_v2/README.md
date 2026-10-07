# Light enemy / single claw strike

Version 2 builds on the approved dynamic slash retained in `../light_attack_v1/`. Its original source, runtime asset and preview remain intact. The baseline hashes are in `validation/light-attack-v2/approved-baseline.json`.

This pass strengthens the push from the planted leg, extends the lead step, adds a small shoulder recoil before release, and carries more torso weight into the follow-through. During recovery, shoulder, elbow and claw return in sequence, with a delayed wrist settle. The opposite arm retains a bend reserve to avoid overextension. Attack duration and hit frames remain unchanged.

Dedicated 1.6-second single sweep on the original 22-joint, 457-triangle body. One leading-foot step; rear foot remains anchored. No mesh or texture change.

The claw arm uses a shoulder-driven rotational swing. The elbow leads across the body while the forearm and claw trail, then release into the follow-through. The dynamic revision exaggerates the backward coil, raised claw, lead-foot step and torso twist. A five-frame accelerating slash contrasts with the anticipation and slow recovery. The opposite arm counterbalances the torso.

| Frames at 30 Hz | Beat |
|---|---|
| 0–8 | Compress over the support leg |
| 9–16 | Rise into a large backward coil; lift the leading foot |
| 16–19 | Raised claw holds behind the shoulder; pelvis starts to drive |
| 19–24 | Accelerating slash; lead foot plants at 20; contact window 20–25 |
| 24–33 | Torso and claw overshoot into a low, exposed follow-through |
| 34–48 | Slower arm and torso recovery; leading foot returns 34–46 |

The dedicated clip plays at 1× speed, frames 0–48. A duplicate final pose at 49 preserves the runtime's frame-count timing contract. The LightAttack capsule stays on the claw (joint 9); its active window is 20–25. Damage/poise values remain those of each character's single attack. Runtime cooking derives the committed attack duration from the clip and events.

Use `author.py` in headless Blender to regenerate PSXA and NPZ, `save_blends.py` to bake the packed editable rig and render, and `export_blend.py -- strike [destination]` to export manual edits. Regeneration overwrites manual edits. Rig and sole helpers are imported from the approved motion pass.

Integration also corrects the Charged Cannon variant’s previously reversed action tags: the three combo windows belong to HeavyAttack and its single-strike capsule belongs to LightAttack. Geometry, damage and poise remain attached to their original capsules. Approved run, turn and alert assets are unchanged.
