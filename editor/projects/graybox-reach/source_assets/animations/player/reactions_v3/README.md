# Aletha hit and stronger poise break, revision 3

The user selected revision 2's poise-break motion for the hit reaction and requested a stronger poise break. `hit.blend` and `hit.psxanim` are byte-identical copies of `reactions_v2/poise` (1.333 seconds). Hash evidence is in `validation/player-reactions-v3/hit-preservation.json`.

The new poise break is 2.067 seconds: a sharper backward chest recoil and head lag, failed catch, legs buckling into a one-knee collapse, a short vulnerable pause, then the head lifts and the free hand braces the forward leg to help stand. The left foot stays planted; the right foot steps back and rolls onto the toe. Its lowest mesh vertex is kept on the floor throughout the toe roll.

Authored beat times use 30 Hz units, exported at 60 Hz: impact 1, recoil 5, legs fail 10, knee down 19, stunned through 27, push up 38, stand 49, ready 62. Existing Aletha model, skeleton and idle are unchanged. 126 stored poses include the endpoint sentinel; the selected hit has 82.

The main level keeps training mode enabled for the encounter reset shortcuts. Training does not disable combat or damage; the earlier interpretation was incorrect.

Both clips are approved and installed in Graybox Reach: resource 87 binds HitReact and resource 54 binds Stun in animation set 61. Runtime copies live in `assets/animations/player_reactions_v3`. Both use full authored samples, speed 256 (1x), and preserve their authored root motion within the pose. The old 4x hit speed and 48-tick recovery cap are removed. Reaction starts and recovery deadlines now use the same absolute simulation clock as player update/render; the previous gameplay-relative start made the animation expire immediately after loading. At 60 Hz the animation locks are 81 and 125 ticks respectively, including the final pose sample.

The player poise pool is 60, as explicitly selected by the user. From a fresh pool, a 50-poise light claw causes the hit reaction and a 75-poise heavy claw causes the knee-drop. Consecutive light hits before poise recovery can also break poise. Ordinary unarmoured hits interrupt actions; active heavy-attack armour still suppresses an ordinary flinch. A poise break can upgrade an ongoing hit reaction, but further hits cannot restart or downgrade a locked poise-break animation. Death remains authoritative. The reaction restores dash-scattered polygons immediately so the impact pose is visible.

The local Bloodborne research informs the separation of input buffering, action permissions, and authored recovery timing. These poise values and motions are Cortex tuning, not recovered Bloodborne values. Future early-cancel windows should be explicit animation events rather than another fixed recovery cap.

Rebuild with headless Blender `author.py`; run `validate.py` for export and subframe floor/contact checks; render with `render.py -- --full`; encode the 1080p videos with `preview.py` using Pillow and ffmpeg. Reviews omit runtime scarf and weapons and are not gameplay captures. Validation and renders are in `validation/player-reactions-v3`.
