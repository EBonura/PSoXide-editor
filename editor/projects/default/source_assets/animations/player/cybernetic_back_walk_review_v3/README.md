# Backward walk - contact and overlap review v3

Approved on 30 September 2026 and installed in the default project. Keeps v2's 1.4-second cycle, 48 cm step excursion, 5 cm vertical body movement, 6.8 cm lateral weight transfer, 6-degree hip/chest rotations and 42-degree upper-arm arc.

## Changes

- Support is calculated at the sole contact point. Ankle translation compensates for shoe rotation, removing the drift caused by rolling a foot around its ankle. Nominal translating speed remains 0.591 m/s. This is a source-space check, not game-motor validation.
- Backward toe contact settles over about seven frames at 14 degrees instead of the abrupt five-frame 16-degree roll. The front foot releases over about five frames with eight degrees of heel roll.
- Swing clearance peaks slightly earlier, giving the foot more time to descend into contact while retaining the 10 cm clearance.
- Chest and head follow the pelvis with progressive phase delays. Forearms reverse about 1.3 frames after upper arms; wrists add three degrees of delayed follow-through. Relaxed fingers and arm clearance remain.

## Beats

Frames 0/21: toe contact. Frames 4/25: deepest weight acceptance. Frames 7/28: sole fully settled. Frames 12/33: rise over support. Frames 19/40: release begins. Frames 24/3: lift-off. Frames 32/11: swing apex. Frame 42 is an exact copy of frame 0.

Recreate with author.py, render.py and encode.py. Comparison is v2 versus v3. validation.json records IK, nominal moving contact stability, subframe floor clearance, loop seam and camera framing. Approved master/export hashes are recorded in approval.json. Native validation is recorded in runtime-validation.json.

## Installed settings

Clip 81 / gen_walk_bwd; source 209; player action WalkBackward in set 61. Both clip calibration and binding use in_place: false, preserving the approved weight transfer. No root travel or motor speed change is introduced. Existing forward walk, strafes and attacks retain their bindings.

Source: 43 stored samples at 30 Hz. Runtime cook: 20 stored samples at 13 Hz, with the duplicate endpoint preserved. The runtime includes all stored frames in the loop period. Speed Q8 281 gives a 1.40322-second cycle including that endpoint interval. The 2-degree resampling budget is unchanged.

Rebake from repository root:

```sh
target/release/import-locomotion editor/projects/default/project.ron editor/projects/default/source_assets/animations/player/cybernetic_back_walk_review_v3 --pack gen --fps 30 --no-trim
```

After generic import restore source 209, approved tags, calibration in_place: false, binding in_place: false and speed_q8: 281. Build with target/release/frontend build-project-disc --project editor/projects/default/project.ron.

Validation passed: exact approved master/export hashes, cooked endpoint equality, project default test, native boot/gameplay smoke and zero mapped load-delay hazards. The native route did not acquire an enemy lock, so locked-on backward activation was not established by that replay; the actual cooked backward poses and binding were inspected separately. 17,276 bytes of heap remained at replay end.
