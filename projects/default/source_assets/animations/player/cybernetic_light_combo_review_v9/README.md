# Light combo v9 — compact descending return

Studio candidate in response to further feedback on the second swing. The first swipe's Blender master and GLB remain byte-identical to the accepted v6 studio opener. This is still a two-hit X: first diagonal, then opposite descending diagonal, with one sword cast.

The second swing now uses an authored backhand instead of mirroring the first swing's entire follow-through. A compact lift brings the hand across the chest; shoulder, torso and arm turn together through the cut; the sword settles low on the opposite side instead of rebounding upward behind the body. Head orientation follows the torso. A fixed chamber endpoint makes the initial joint interpolation continuous. The elbow solution preserves its bending side, and forearm orientation follows the grip. Feet remain planted during the cut, followed by separate recovery steps.

The return has 40 frames at 30 Hz. Cutting frames are 12–17; the studio joins opener frames 0–33 to return frames 0–39. Two views are shown at normal speed then half speed. The opener-to-return entry matches within floating-point tolerance. Validation records opposite lateral/downward blade travel, blade-facing alignment, joint rotation steps and floor contact; these checks do not substitute for aesthetic review.

This candidate has NOT been baked or installed. The default project still contains the earlier v3 three-hit configuration. When approved for installation, retain only LightAttack → LightAttackFollowup, remove the follow-up → finisher chain, and align damage/trail events with the new return's baked stroke. Preserve historical finisher sources and records. Validate the eventual compact bake in native playback.

Authoring script: `review/light-combo-v9/author.py`. Video: `review/light-combo-v9/studio-two-hit-x.mp4`. Both paths are relative to the default project root.
