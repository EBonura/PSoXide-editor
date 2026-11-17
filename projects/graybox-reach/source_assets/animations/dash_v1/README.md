# Enemy dash clips, version 1

Dash Left and Dash Right for the light enemy and the heavy enemy. Binding a dash gives an enemy the mobility kit, so its bolt dodge is its roll (the motor's Roll profile, with its own roll data and i-frames) instead of a walk aside.

The clips are in place: the motor carries the body along its decelerating roll travel (`decelerating_roll_step` in psx-engine `character_motor.rs`, run 2 ticks per 30 Hz entity step). Every foot that touches the floor is pinned in the world: its model-space position is its world position minus that frame's motor travel, solved with leg IK frame by frame. Feet move only in the air, finish their sideways move before they come down, and land straight down. Measured world slip on every planted frame is 0 model units for all four clips (`measure_slip.py`).

| Clip | File | Length | Roll data it is keyed to |
|---|---|---|---|
| Light, Dash Left / Right (low bound) | `graybox-reach/assets/animations/knockdown_v1/light_dash_left.psxanim`, `light_dash_right.psxanim` | 21 frames at 30 Hz plus the end sentinel (0.70 s) | roll_speed 52, roll_active_frames 26, roll_recovery_frames 16: 1,352 editor units |
| Heavy, Dash Left / Right (shoulder lurch) | `default/assets/animations/knockdown_v1/heavy_dash_left.psxanim`, `heavy_dash_right.psxanim` | 27 frames at 30 Hz plus the end sentinel (0.90 s) | roll_speed 32, roll_active_frames 32, roll_recovery_frames 22: 1,024 editor units, about 75% of the light enemy's |

The roll values are placeholders on each Character (and on any placed controller that carries its own settings); roll_invulnerable_frames is unchanged. The light clips are pinned for the light enemy's 417 visual scale (its grid-dungeon placement); a placement at another scale moves its feet by the scale difference. Change the roll data and the clips need re-keying: the travel is baked into the feet.

Start and end poses: the light enemy's walk v5 guard (both directions); the heavy enemy's Strafe Left start pose for Dash Left and its Strafe Right start pose for Dash Right. Dash Right is Dash Left mirrored per limb: the other leg leads and the other pushes off, the sideways body channels and the travel change sign, the heavy's arm roles swap (the rigs are not symmetric, so it is not a joint mirror).

Rebuild: headless Blender 5.2 `light_dash.py -- light_A4` (both light clips); plain python3 `heavy_dash.py heavy_A4_ strafe` (Dash Left) and `heavy_dash.py heavy_A4R strafeR` (Dash Right). Both write next to themselves; `heavy_dash.py` reads `psxdecode.py` from `../knockdown_v1`. Earlier review candidates (A, B, A2, A3) are still in the scripts; A4 / A4R are the shipped ones.
