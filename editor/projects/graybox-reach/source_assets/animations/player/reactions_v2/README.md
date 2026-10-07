# Aletha impact reactions, revision 2

Supersedes the rejected reaction studies in `reactions_v1`. These are review candidates, not installed in Graybox Reach.

The first pass read as a voluntary squat and protective gesture. This pass starts with an external impact: fast backward chest displacement and torsion, followed by delayed pelvis/head movement and loose asymmetric arm arcs. It contains no anticipation before contact.

- Hit: 0.70 seconds. Sharp off-centre recoil, feet remain planted, head whips after chest, then the body arrests and recovers.
- Poise break: 1.333 seconds. Larger backward recoil continues into loss of balance, the trailing right foot steps back to catch the body, then weight settles and the foot returns during recovery. The left foot remains planted.

Beat keys are authored in 30 Hz time units and sampled at 60 Hz. Exports have 44 / 82 stored poses including final sentinel. Existing 458-triangle, 26-joint model and base idle pose are preserved. Source `.blend` files are editable; `.psxanim` files are candidate exports only.

The current game's 48-tick poise recovery cap would cut the longer candidate short. Integration must explicitly resolve this timing cap and switch away from the current 4x clip speed; no runtime or project-binding changes have been made during this review.

`author.py` rebuilds both studies. `render.py -- --full` renders all review poses, `preview.py` produces 1080p videos at intended speed, and `validate.py` checks export matrices, subframe ground contact, planted foot movement, and exact recovery to the initial pose. Validation evidence is in `validation/player-reactions-v2`.

Previews use the character mesh alone; runtime scarf and weapon effects are not simulated. The background grid and clay shading are for motion review. Videos are Blender studies, not gameplay captures.
