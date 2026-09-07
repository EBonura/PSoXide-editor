# Cortex Ignition opening

New Game plays the opening after world loading. Respawning does not replay it.
The two orbit shots hold Aletha on the first frame of the latest wake-up take;
one orbit across her front shows the entire ground punch and stand-up without
a cut or zoom change. The sequence has no music. Movement, attacks, enemy AI, inventory
and gameplay HUD wait for it.

| Shot | Time | Action |
|---|---|---|
| Wide orbit | 0–5 s | Opposite-side establishing view; fade out before the cut |
| Nearer orbit | 5–10 s | Reveal the nearer orbit, then fade out before the next cut |
| Wake-up front orbit | 10–39.7 s | Fade in, then play the entire animation with an orbit |
| Handoff | 39.7–42.4 s | Fade out, switch cameras at full black, fade into play |

Every camera change happens at full black. Fades last 1.25 seconds on each side,
with a 0.2-second black hold and smooth easing. The wake-up waits until its reveal
finishes. Its duration comes from the selected clip, including the editor playback
speed and frame range, so a replacement cannot be cut off by the old four-second
timer. Take 3 plays frames 0 through 338 at its recorded speed (about 28.2 seconds).
The action binding trims 24 frames from its 363-frame, 12 Hz clip, removing two
seconds from the ending without modifying the source animation.
All three shots orbit counter-clockwise when viewed from above. The nearer
orbit starts at its previous endpoint, 26.37 degrees farther around than before.
Its distance, height, focus and 5.273-degree-per-second orbit speed are unchanged.
The final shot uses the selected 13%–84% section of its previous full orbit:
73.56 degrees to the right of frontal through to 66.36 degrees to the left.
It crosses about 139.92 degrees over the same shot duration, including both fades,
so the movement is slower while the animation keeps its existing timing.
The final shot has a lower eye and focus, with a slightly wider radius
to keep her hands visible while prone and her head visible while standing.
Distance, eye height and focus height stay constant within the shot.
Skipping holds the current shot throughout fade-out.

The fade modulates the composed framebuffer with an opaque, dithered 16-bit
texture pass instead of subtracting a constant from every colour. This keeps
dark materials visible as they dim. It uses the current framebuffer in place
with exact pixel coordinates and flushes the texture cache before reading it;
no extra VRAM or CPU pixel processing is required.

Hold X for 0.5 seconds to skip. A bar shows the hold; releasing X resets it.
The New Game confirmation must be released before a skip can register. Skipping
uses the same black-screen handoff and ends in the standing idle pose.

Shot timings and framing live in `engine/examples/editor-playtest/src/opening_sequence.rs`.
Distances scale with the character height. Authored cinematic shots bypass the
gameplay camera collision solver. While a cinematic shot is active, BSP visibility
is selected from a point above Aletha, allowing exterior camera positions to see
the level; frustum and face culling still use the actual camera. Normal camera
collision and camera-based visibility resume at the black handoff. Before freezing the motor, trace below the spawn and settle Aletha
onto the walkable BSP floor. Otherwise the authored spawn clearance leaves her
suspended until gameplay gravity resumes. Saved combat settings are unchanged.

The original source is `source_assets/animations/player/wake_up_take3_original.glb`
inside the Cortex project. `tools/aletha_intro.py` retargets it onto the delivered
Aletha rig in headless Blender. The retarget preserves the source vertical root
trajectory and aligns the final standing pose to the floor with one constant
offset. Do not ground individual frames by their lowest mesh vertex: moving
hands and feet introduce artificial whole-body dips that way. `cargo run -p psxed-project --example
cook_aletha_intro -- editor/projects/cortex-ignition-tech-demo-0.4b` bakes the clip
with `aletha_wake_up_take3` as the final argument
and checks that it matches the existing model's joints and quantization.

The PS1 build interns identical cooked poses using the v5 animation dictionary.
This retains the v4 pose bytes, frame count and sample rate. Frame and fractional
sampling comparisons cover all clips; no animation is resampled for this saving.
The format definition is in psxed-format and its reader is in psx-asset, so both
SDK changes must accompany the editor cooker when updating component pins.

Take 3 uses its unmodified retargeted arm motion. The experimental hand-to-knee
correction was removed: its moving elbow pole swung the forearm across the body
and its unreachable wrist target left the arm locked straight. A visible contact
gap is preferable to distorting the original gesture.

The later ground slam at cooked frame 181 (source frame 452, 30 Hz), around
26 seconds into the opening, triggers the existing
dash mesh breakup once: world-anchored fragments, wireframe, then reconstruction
over the live pose. It uses the normal 48-tick visual lifetime and does not start
a dash, move the motor, change stance or alter invulnerability. The cue follows
the sampled animation frame, so skipped render frames cannot miss it. Skipping
suppresses an unplayed cue; the black camera handoff cancels any remaining cloud.

Camera projection is fixed at focal length 320 on a 320 by 240 frame (horizontal
field of view 53.13 degrees, vertical 41.11 degrees). Shot size comes from camera
distance; there is no lens zoom. The authored distance/eye/focus triples are
7500/3500/220, 3000/1900/220 and 2800/1100/600, scaled to the player's height
(1024 authored units). At nominal cut boundaries, relative orbit angles are
243.28 -> 216.91 degrees for shot 1 and 190.55 -> 164.18 for shot 2. Zero is
Aletha's front, 90 her right, 180 behind her and 270 her left. All angles decrease.
The final camera travels from +73.56 to -66.36 degrees across the entire shot,
including its fades, at about 4.52 degrees per second. Its framing is unchanged.

Each of the three shot starts queues the same resident `IntroShot` sound once,
including the first reveal and the two camera cuts at full black. The selected
2.2-second cyborg excerpt continues through fade-in at normal pitch. A skip does
not queue another shot sound, and the final gameplay handoff does not replay it.

After the cinematic, the three welcome panels hold player and camera controls until
the final panel finishes closing. X still completes the text and advances pages;
Start cannot open inventory during this introduction. The third panel explains
left-stick movement and right-stick look. Pickup and closing demo messages keep
their existing input behavior.
