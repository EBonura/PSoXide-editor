# Aletha Horizon light combo, historical candidate v3

Retired on 30 September 2026. The default now uses the approved two-hit [v12 combo](../cybernetic_light_combo_review_v12/README.md). The settings below describe the preserved historical candidate.

Three R1 strikes with one sword cast. The first strike uses the approved light attack v5 (clip 84) unchanged. The two new clips are candidates for visual review, not user-approved replacements.

| Action | Clip / source resource | Files |
| --- | --- | --- |
| LightAttackFollowup | 204 / 206 | `light_attack_followup.blend`, `light_attack_followup.glb` |
| LightAttackFinisher | 205 / 207 | `light_attack_finisher.blend`, `light_attack_finisher.glb` |

Each editable master has 40 frames, 0–39 at 30 Hz (1.3 seconds). The GLBs contain the Aletha rig and skinned meshes, without the preview sword or stage. The reverse cut continues the opener's forward stance; the finisher raises the sword into a weighted descending cut. Both have their own recovery to idle so stopping after either strike works.

The current compact bake stores 14 poses at 10 Hz, with runtime interpolation. Keep `in_place: false`, speed 256, no controller push, and non-looping playback. The animation set is 61, character 62, weapon 30. Do not run the generic pack importer against the default project without restoring the event and chain settings below: importing a pack updates bindings and playback metadata.

## Input and event contract

Frame numbers below refer to the registered `.psxanim` sample rate, not the 30 Hz Blender masters.

| Strike | Input window | Handoff | Damage / trail | Sword |
| --- | --- | --- | --- | --- |
| Approved opener, 30 Hz | 20–34 | 34 → second | 22–29 | Existing cast 6–12; original recovery if not chained |
| Reverse cut, 10 Hz | 5–9 | 9 → third | 5–8 | Fully materialized at frame 0; dissolve 11–13 if stopping |
| Finisher, 10 Hz | None | Recover to idle | 6–9 | Fully materialized at frame 0; dissolve 11–13 |

Each continuation blends for four simulation ticks. Each strike does 25 damage and 25 poise, using its own one-hit-per-target swing state. Release and press R1 again during the input window; holding R1 does not queue the entire combo. A missed input completes the current recovery. R2 does not join this chain. Evade intent, interruption, death, respawn and stance changes clear queued continuation; this adds no early dodge cancel.

The second-to-third baked entry poses match exactly. The first entry follows the approved opener's frame 34 and uses the runtime blend to bridge bake differences. The original opener and walk assets retain their approved checksums.

`source-validation.json` records evaluated mesh ground contact across every authored frame. Current authoring scripts, build logs and videos are under `review/light-combo-v3/`; earlier candidates remain in their review directories. The project-wide status ledger is `ANIMATION_REGISTER.md` and `animation-register.json`.

## Verification and PS1 memory

The native emulator replay covers one, two and three R1 presses and completes 2,800 controller polls without a guest exception. The baked guest passes the MIPS load-delay hazard scan. Runtime tests: 239; playtest tests: 78; targeted chain/cooker/editor tests: 4. Hardware has not been tested.

The cooker now rejects sample-rate reductions that increase encoded size. This retains the original 12 Hz Light Enemy Hit React and Heavy Enemy Strafe Left/Right clips, saving 12,724 resident bytes compared with the previous resampling decisions. Their registered source assets remain unchanged. The approved walk and opener keep their playback settings and checksums.

The built guest leaves 13,536 bytes between static data and the reserved stack boundary; after startup allocations the remaining boot heap is 4,660 bytes. The stack reserve was not reduced. `runtime-validation.json` records the native replay, guest/disc hashes and measured memory figures. Current 30 fps videos are `review/light-combo-v3/native-one.mp4`, `native-two.mp4` and `native-three.mp4`.

## Candidate v2 arm polish

Following feedback about the hand movement, v2 keeps the same three-hit timing and body performance while revising the right upper arm, forearm and wrist. It uses a consistent elbow bend, a stable local grip and a smooth recovery to neutral. The baked pose connecting strike two to strike three remains byte-identical on both sides.

The v1 wrist changed by up to 145 degrees between authored frames. V2 limits the wrist's local change to under 0.6 degrees per 30 Hz source frame; intentional sword swings now come from the arm and body. The upper-arm recovery flip is removed. Only right-arm and attached finger joints change materially in the baked clips; the approved opener and walk are untouched. Prior candidate checksums remain in the register's revision history.

V2 native videos and diagnostics are retained in `review/light-combo-v2/`. `motion-validation.json` records this pass's source measurements. The timing and sample rates above are unchanged.

## Candidate v3 centered follow-ups

Following feedback that both follow-ups swept too far into the upper left, v3 brings the second wrist path lower and across the front of the torso. The finisher loads closer to the centre and descends through it; its elbow bend is aimed from the intended blade path. The wrist keeps the v2 grip, changing by less than 0.6 degrees per authored frame. The existing body, footwork, weight, input windows and hit timing are preserved.

Measured over the active authored frames, the second blade arc averages 0.34 m lower and its mean absolute lateral tip offset reduces from 0.96 m to 0.64 m. The finisher's mean absolute lateral tip offset reduces from 0.88 m to 0.10 m. These measurements use the existing preview sword/socket relative to the model's forward centre plane, not screen coordinates.

The two new trails now retain two frames at 10 Hz (0.20 seconds), closer to the opener's five frames at 30 Hz (0.17 seconds). This removes the oversized lingering fan without changing damage windows. All 62 other registered animation assets retain their checksums. The baked second-to-third pose still matches exactly.

`motion-validation.json` preserves earlier wrist measurements and adds the v3 joint and centring results. `runtime-validation.json` records the current native replay and build hashes. Current videos and authoring diagnostics are in `review/light-combo-v3/`.
