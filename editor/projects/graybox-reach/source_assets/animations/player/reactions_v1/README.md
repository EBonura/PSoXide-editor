# Aletha reaction review

Review candidates for Graybox Reach's existing 458-triangle, 26-joint Aletha. Project bindings and gameplay code have not been changed.

## Current implementation

The player animation set (61) binds HitReact to resource 87 (`gen/hit_react.psxanim`) at speed_q8 1024 (4x), and Stun to resource 54 (`aletha_stun.psxanim`). `interrupt_player_on_poise_break` in `engine/examples/editor-playtest/src/playtest_runtime.rs` always starts PlayerAnim::HitReact. It does not select Stun. Non-breaking hits retain the current action and use impact feedback. Thus there are not currently two separately played player reactions.

The reaction source has 43 stored samples at 15 Hz, approximately 0.683 seconds at the configured speed (sentinel excluded). The legacy Stun has 25 samples at 12 Hz, approximately 1.917 seconds at 1x, and is disconnected from the player poise-break path. These are source timings; cooker resampling and fixed-point playback can round the exact runtime duration.

The poise-break lock is clamped to 18..48 gameplay ticks (0.3..0.8 seconds at 60 Hz), so a longer replacement would require a deliberate gameplay timing change. Player poise uses a pool of 20; active heavy attacks can resist poise damage. No combat balance changes are included here.

## Candidates

- `poise.blend` / `poise.psxanim`: 0.8 seconds at 60 Hz, 49 poses plus endpoint sentinel. Initial backward chest recoil, folding compression, staggered pelvis/head response, one backward recovery step, free hand protecting ribs, weapon arm displaced, guard return.
- `hit.blend` / `hit.psxanim`: 0.6 seconds at 60 Hz, 37 poses plus endpoint sentinel. Smaller asymmetric recoil with both feet planted. Study only; adding ordinary-hit interruption is not part of this proposal.
- `old_active.blend`: comparison sampled at the current 4x setting, horizontal in-place correction applied.
- `old_stun.blend`: legacy unused Stun at source speed.

`author.py` recreates all studies from the preserved current character rig and clips. `render.py -- --full` renders source-animation review frames. `preview.py` builds 1080p review videos using Pillow and ffmpeg. The studies intentionally omit runtime scarf simulation and weapon attachments. They are Blender previews, not in-game captures.

Validation and MCP project-load evidence are in `validation/player-reactions-v1`. Review the animation direction before changing the gameplay binding; the active poise-break should use one complete reaction with recovery at 1x, with no new ordinary-hit interruption unless explicitly desired.
