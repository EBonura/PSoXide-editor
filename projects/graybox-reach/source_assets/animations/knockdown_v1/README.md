# Knockdown clips, version 1

The Knockdown animation role for Aletha and the light enemy. A poise break knocks a fighter down for its `knockdown_ticks`; the runtime plays the Knockdown clip once and holds its last frame until then. Before these clips the role fell back to the Stun + Recovery clip, cut off at 60 ticks.

Both clips start and end on the pose their Stun + Recovery clip starts and ends on, so the clip blends in from any action and back into idle or guard. Neither is a full fall.

| Clip | Shipped as | Length | Beats |
|---|---|---|---|
| Aletha, hand down | `assets/animations/knockdown_v1/aletha_knockdown.psxanim` | 157 samples at 60 Hz (2.6 s) plus the end sentinel | 30 Hz beat times: strike 1, stagger step 4 to 11, right knee down 20, left hand on the floor 20 to 46, helpless hold with breathing, push up 46 to 58, hand braces the thigh, stand 70, settle 78 |
| Light enemy, knee buckle | `assets/animations/knockdown_v1/light_knockdown.psxanim` | 67 samples at 30 Hz (2.2 s) plus the end sentinel | blown sideways and back 1 to 8, rear leg buckles 9 to 17, low list over the rear leg with the free claw resting on the floor 17 to 46, plants the lead foot and drives up 46 to 62, guard 66 |

Aletha starts on her idle frame 84, where the Idle binding starts playing; the enemy on the walk v5 guard (frame 0), the pose its Stun + Recovery uses.

Bindings: the Knockdown action of the Aletha Delivered and Light Enemy animation sets, speed 256 (1x), full range, not looping, in every project that uses those sets with these models. `knockdown_ticks` on every character and placed controller that uses the set equals the clip: 157 for Aletha, 134 for the light enemy (67 samples at 30 Hz).

Rebuild with headless Blender 5.2: `aletha_author.py` (all three reviewed candidates; A is the shipped one), `enemy_author.py` then `enemy_save.py` (C is the shipped one). Both write next to themselves and read the model, idle and walk sources already in this project. The candidates were keyed poses over each character's own idle or guard, with leg and arm IK, floor checks on the deformed mesh at every sample, and a claw floor guard on the enemy.
