# Light enemy / heavy overhead claw

A 2.2-second single overhead cleave on the original 22-joint, 457-triangle body. The approved light attack remains in light_attack_v2.

Arm revision: the shoulder now carries a connected upper-arm/forearm/hand chain in the moving chest's coordinate frame. The elbow hinges in one plane, folding to 118 degrees for the load and unfolding to 42 degrees through impact. The wrist inherits the forearm and adds only a -7 to +12 degree cock. This replaces the rejected independent world-space forearm and claw aiming. The elbow stays bent beside the head during preparation.

| Frames at 30 Hz | Beat |
|---|---|
| 0–10 | Compress over the planted rear foot |
| 10–24 | Raise elbow and claw overhead, arch torso and transfer weight back |
| 24–30 | Hold a readable high threat while lifting the lead foot |
| 30–36 | Step and drive the shoulder down; forearm and claw follow |
| 32–37 | One active damage window |
| 37–50 | Low, exposed follow-through |
| 50–66 | Shoulder, elbow and wrist recover in sequence; lead foot returns |

Playback is 1×, frames 0–66, with duplicate sentinel frame 67. Runtime HeavyAttack replaces the old three-swing combo; its original shared source remains intact. Both light-enemy variants use one claw capsule during frames 32–37, with 48 damage and 75 poise damage. LightAttack and the heavy enemy character are unchanged.

Run author.py, validate.py and save_blends.py in headless Blender. Use save_blends.py -- --render for both review angles, export_blend.py -- heavy [destination] for a PSXA export from the editable rig, and preview.py with system Python/Pillow/ffmpeg for the 1080p video. Regeneration overwrites manual Blender edits.
