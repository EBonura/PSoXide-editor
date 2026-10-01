# Animation register — Cortex Ignition v0.4b

Updated 30 September 2026. **9 approved replacements; 1 approved combo follow-up; 1 retired combo candidate; all remaining animations are original to this pass.**

“Original” means unchanged from the v0.4b project before these replacements, not necessarily an artist’s original capture. The baseline is Git commit `44d1f52bdcaacc0444a43af8dcc76ebcc205ea5a`. Resource IDs and paths remain stable so existing character bindings keep working.

## Approved replacements

| Character / action | Approved version | Clip | Source and rebake notes |
| --- | --- | --- | --- |
| Aletha / Walk | Cybernetic walk v7 | 73 · `gen_walk_fwd` | [Editable master and export](source_assets/animations/player/cybernetic_walk/README.md) |
| Aletha / Horizon LightAttack | Cybernetic light attack v5 + approved v12 head motion | 84 · `gen_light_attack` | [Editable master and export](source_assets/animations/player/cybernetic_light_attack/README.md) |
| Aletha / Horizon HeavyAttack | Cybernetic cross slash v2 | 85 · `gen_heavy_attack` | [Editable master and bake notes](source_assets/animations/player/cybernetic_heavy_attack_review_v2/README.md) |
| Aletha / WalkBackward | Cybernetic backward walk v3 | 81 · `gen_walk_bwd` | [Editable master and bake notes](source_assets/animations/player/cybernetic_back_walk_review_v3/README.md) |
| Aletha / StrafeLeft | Faster cybernetic side step v2 | 82 · `gen_walk_lft` | [Editable masters and bake notes](source_assets/animations/player/cybernetic_strafe_review_v2/README.md) |
| Aletha / StrafeRight | Faster cybernetic side step v2 | 83 · `gen_walk_rgt` | [Editable masters and bake notes](source_assets/animations/player/cybernetic_strafe_review_v2/README.md) |
| Aletha / WalkWindup | Cybernetic transitions v4 | 74 · `gen_walk_fwd_windup` | [Masters and bake notes](source_assets/animations/player/cybernetic_walk_transitions_review_v4/README.md) |
| Aletha / WalkWinddown | Cybernetic transitions v4 | 75 · `gen_walk_fwd_winddown` | [Masters and bake notes](source_assets/animations/player/cybernetic_walk_transitions_review_v4/README.md) |
| Aletha / WalkWinddownAlt | Cybernetic transitions v4 | 76 · `gen_walk_fwd_winddown_mirror` | [Masters and bake notes](source_assets/animations/player/cybernetic_walk_transitions_review_v4/README.md) |

The walk keeps its restrained body motion and wider arms. The light attack keeps the approved two-second performance, sword cast, deep load, lunge, strike and weighted recovery. All nine replacements retain authored body translation (`in_place: false`). The Zenith light attack is still the original.

## Approved two-hit combo

The [approved v12 X combo](source_assets/animations/player/cybernetic_light_combo_review_v12/README.md) is installed in the default project. One R1 press plays the opener; a new press from 0.40–1.13 seconds queues the opposite diagonal swipe. The handoff is at opener frame 34. Holding R1 does not chain, and repeated presses cannot add a third strike. The sword casts once and stays materialized between hits.

Clip 84 now includes the approved studio opener's head motion. Clip 204 is the approved edge-aligned return, baked at 15 Hz with interpolation; its 12/14/16 source-frame strike keys land exactly on baked samples 6/7/8. Clip 205 remains in the catalogue as a retired v3 candidate, with no active binding, appearance track, hitbox or chain. The Zenith attacks remain original.

Native emulator verification covers single, double, held, early, late and repeated R1 inputs; [validation](source_assets/animations/player/cybernetic_light_combo_review_v12/runtime-validation.json). The register retains all historical review sources and checksums.

## Approved heavy cross slash

Horizon R2 uses the approved v2 heavy cross slash: staggered heavy/right and light/left casts, deep load, 94 cm driving step, inward crossing cut and weighted recoil. Frames 0-60 at 30 Hz, speed 256, authored translation retained. Right blade casts over 6-11, left over 12-17; damage and both trails run over 23-29; blades dissolve over 45-50. Damage 38 and poise 50 are unchanged. Zenith heavy remains original. See the [master and validation](source_assets/animations/player/cybernetic_heavy_attack_review_v2/README.md).

## Character action checklist

### Light Enemy Animation Set

Used by: Light Enemy (113), Light Enemy / Charged Cannon (198).

| Action | Clip resource / name | Status |
| --- | --- | --- |
| Idle | 32 · `Light Enemy / Idle` | Original |
| Walk | 33 · `Light Enemy / Walk` | Original |
| Run | 34 · `Light Enemy / Run` | Original |
| StrafeLeft | 71 · `Light Enemy / Strafe Left` | Original |
| StrafeRight | 72 · `Light Enemy / Strafe Right` | Original |
| Death | 91 · `Light Enemy / Death` | Original |
| HitReact | 92 · `Light Enemy / Hit React` | Original |
| Stun | 108 · `Light Enemy / Stun + Recovery` | Original |
| HeavyAttack | 139 · `Light Enemy / Horizon Heavy (Three Swings)` | Original |
| LightAttack | 141 · `Light Enemy / Horizon Light (Single Strike)` | Original |
| VertLightAttack | 192 · `Light Enemy / Charged Shot` | Original |
| WalkBackward | 104 · `Light Enemy / Walk Backward` | Original |
| Turn | 64 · `Light Enemy / Turn In Place` | Original |
| Intro | 65 · `Light Enemy / Alert` | Original |

### Aletha Delivered Animation Set

Used by: Aletha (62).

| Action | Clip resource / name | Status |
| --- | --- | --- |
| Intro | 201 · `Aletha / Wake Up — Take 3` | Original |
| Roll | 36 · `aletha_dash_fwd` | Original |
| Death | 39 · `aletha_death` | Original |
| HitReact | 87 · `gen_hit_react` | Original |
| Idle | 45 · `aletha_idle` | Original |
| HeavyAttack | 85 · `gen_heavy_attack` | **Replaced - approved cross slash v2** |
| LightAttack | 84 · `gen_light_attack` | **Replaced — approved v5 performance / v12 head** |
| LightAttackFollowup | 204 · `gen_light_attack_followup` | **New — approved v12** |
| Run | 77 · `gen_run_fwd` | **Original retained by user** |
| Stun | 54 · `aletha_stun` | Original |
| WalkBackward | 81 · `gen_walk_bwd` | **Replaced - approved v3** |
| Walk | 73 · `gen_walk_fwd` | **Replaced — approved v7** |
| StrafeLeft | 82 · `gen_walk_lft` | **Replaced - approved v2** |
| StrafeRight | 83 · `gen_walk_rgt` | **Replaced - approved v2** |
| WalkWindup | 74 · `gen_walk_fwd_windup` | **Replaced - approved v4** |
| WalkWinddown | 75 · `gen_walk_fwd_winddown` | **Replaced - approved v4** |
| WalkWinddownAlt | 76 · `gen_walk_fwd_winddown_mirror` | **Replaced - approved v4** |
| RunWinddownAlt | 78 · `gen_run_fwd_winddown_mirror` | Original |
| RunWindup | 79 · `gen_run_fwd_windup` | Original |
| RunWinddown | 80 · `gen_run_fwd_winddown` | Original |
| VertLightAttack | 88 · `gen_vert_light_attack` | Original |
| VertHeavyAttack | 89 · `gen_vert_heavy_attack` | Original |

### Heavy Enemy Animation Set

Used by: Heavy Enemy (95).

| Action | Clip resource / name | Status |
| --- | --- | --- |
| Idle | 98 · `Heavy Enemy / Idle` | Original |
| Walk | 99 · `Heavy Enemy / Walk Forward` | Original |
| WalkBackward | 101 · `Heavy Enemy / Walk Backward` | Original |
| StrafeLeft | 102 · `Heavy Enemy / Strafe Left` | Original |
| StrafeRight | 103 · `Heavy Enemy / Strafe Right` | Original |
| Death | 105 · `Heavy Enemy / Death` | Original |
| HitReact | 106 · `Heavy Enemy / Hit React` | Original |
| Stun | 109 · `Heavy Enemy / Stun + Recovery` | Original |
| LightAttack | 137 · `Heavy Enemy / Light Attack` | Original |
| HeavyAttack | 107 · `Heavy Enemy / Heavy Attack` | Original |
| RangedAttack | 138 · `Heavy Enemy / Ranged Attack` | Original |
| Intro | 194 · `Heavy Enemy / Alert` | Original |
| Turn | 195 · `Heavy Enemy / Turn` | Original |

## Additional registered clips

These clips are retained in the catalogue but are not directly bound to the character actions listed above.

| Resource | Clip | Status |
| --- | --- | --- |
| 205 | `gen_light_attack_finisher` | Retired v3 candidate; unbound |
| 199 | `Aletha / Wake Up` | Original |
| 200 | `Aletha / Wake Up — Take 2` | Original |
| 43 | `aletha_hurt_a` | Original |
| 49 | `aletha_rest_pose` | Original |
| 50 | `aletha_run_bwd` | Original |
| 51 | `aletha_run_fwd` | Original |
| 52 | `aletha_run_lft` | Original |
| 53 | `aletha_run_rgt` | Original |
| 55 | `aletha_stun_recovery` | Original |
| 56 | `aletha_walk_bkw` | Original |
| 57 | `aletha_walk_fwd` | Original |
| 58 | `aletha_walk_lft` | Original |
| 59 | `aletha_walk_rgt` | Original |
| 93 | `Light Enemy / Zenith Light` | Original |

## Unregistered legacy files

The existing catalogue contains 14 unregistered legacy `.psxanim` files. All are unchanged; the machine-readable register lists each path and checksum. They are not new replacement candidates or newly introduced audit issues.

## Cook behavior

The combo build also prevents animation resampling from increasing resident bytes. Three original enemy clips retain their source 12 Hz sampling instead of a larger lower-rate bake. This changes no registered animation asset; details and build measurements are in the [combo source notes](source_assets/animations/player/cybernetic_light_combo/README.md).

## Maintaining this record

For each future approved replacement, update this checklist and [the machine-readable register](animation-register.json), preserve the editable master and exact approved GLB under `source_assets/animations/`, and record its version, source resource, timing, and checksums. Keep unchanged actions marked Original until explicitly replaced.

The JSON register inventories every registered animation and legacy clip, with current and baseline SHA-256 values. It also records approval versions, source/master hashes, action bindings, and attack event frames. Original tracked files and settings can be recovered from the baseline Git commit above.

## Approved backward walk

V3 replaces clip 81 while preserving its action binding. It combines weighted backward steps, sole contact pivots, torso counter-rotation and delayed arm follow-through. Source 209 retains the exact approved Blender master and GLB. Left/right strafes now use the approved faster v2 side steps. See the [source and runtime validation](source_assets/animations/player/cybernetic_back_walk_review_v3/README.md).

## Approved left/right side steps

V2 replaces clips 82/83 with faster, weighted side steps. Each source loop is one second, with a leading-foot step and trailing-foot recovery, stronger body compression, and arm follow-through. Sources 210/211 preserve the approved masters. Runtime speed 276 compensates for integer-rate cooking and the duplicate endpoint, giving a 0.99972-second cycle. Character movement speed is unchanged. See the [source and validation](source_assets/animations/player/cybernetic_strafe_review_v2/README.md).

## Main integration, 2026-09-30

The approved animation branch now includes local main `fc6c68f4` and remote main `4f7adcf9` on `codex/cortex-animations-main`. The editor release executable was rebuilt with the shared guest-performance debug panel, and the default 0.4b disc was recooked and rebuilt with the updated SDK and emulator pins.

All 78 existing animation asset hashes are unchanged. Approved attack event timing is retained alongside main's enlarged weapon capsules, including the combo follow-up. Validation passed: 458 project tests, 247 runtime tests, 81 playtest tests and 2 debug-panel tests; zero guest load-delay hazards; native gameplay smoke with all fault registers zero. The redesigned debug panel was also rendered offscreen and inspected. These checks do not constitute physical-console validation. Build hashes and integration revisions are recorded in `animation-register.json` under `main_integration`.

## Run review

The user chose to keep the original forward run (clip 77) after reviewing v1 and v2. Both candidates are rejected and were never installed. The original forward lean, urgency and cadence remain the reference. Walk start and stop transitions are the next review target.

## Walk transition review

The user described the v1 start and two foot-specific stops as "much better". This is the preferred review version, built around the approved walk and current idle. Sources are in `source_assets/animations/player/cybernetic_walk_transitions_review_v1`. V1 was a studio review candidate; v4 is now installed as clips 74, 75 and 76.

Walk transitions v2 adds stronger body loading and delayed torso/arm settling at the same timing and endpoint poses, following the request for more movement. Its source is preserved in `source_assets/animations/player/cybernetic_walk_transitions_review_v2`; the user then identified a limp or pause in the start and stop.

Walk transitions v3 replaces the fixed startup destination with an advancing stride and removes the two separate settling steps from each stop. The startup remains 0.467 seconds; stops shorten to 0.6 seconds. Source and video comparison with v2 are in `source_assets/animations/player/cybernetic_walk_transitions_review_v3` and `review/walk-transitions-v3`. V3 remains a historical studio candidate and was not installed.

Walk transitions v4 responds to feedback that v3 was too immediate and lost the two-foot adjustment. Startup now lasts 0.667 seconds and both stops 0.933 seconds, with overlapping first-foot placement and second-foot adjustment. The moving startup handoff is retained. Sources and validation are in `source_assets/animations/player/cybernetic_walk_transitions_review_v4`; the studio comparison is in `review/walk-transitions-v4`. V4 is approved and installed. The endpoint handoff and sampler padding are integrated; the disc and editor frontend are rebuilt. All 75 other animation files retain their pre-install checksums. See the v4 runtime validation record for tests, native replay and remaining limits.
