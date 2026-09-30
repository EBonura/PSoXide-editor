# Locked-on side steps, faster review v2

Approved on 30 September 2026 and installed as left/right clips 82/83 in the default project.

Responds to the request for faster walking with more body movement. Cycle shortened from 42 frames (1.40 s) to 30 frames (1.00 s), a 40% increase in cadence. Lateral foot excursion grows from 28 to 32 cm; source nominal travel rises from 0.333 to 0.552 m/s. These are source parameters, not changes to game motor speed.

Body movement increases with the cadence: 8 cm lateral weight transfer, 6.2 cm vertical compression/rise, hip and chest rotation up to 6.5 degrees, stronger lateral lean and pitch, 36-degree upper-arm arcs and larger elbow response. Chest, head, forearms and wrists retain their delayed follow-through. Return clearance is 9 cm. Lead/trailing feet alternate without crossing; sole pivots compensate for ankle rotation while supported.

Beat plan at 30 fps: 0 leading-foot contact, 3 compression, 4 sole settled, 8 support rise, 14 release, 15 trailing-foot contact, 18 compression, 23 support rise, 30 identical endpoint. Support occupies 58% of each foot's cycle, retaining double support rather than adding a flight phase.

Run author.py and render.py normally for left, then with -- --right for right. encode.py compares v1 and v2 at their actual authored speeds. Validation covers subframe floor clearance, endpoint equality, knee extension, contact stability and foot separation. Approval hashes are in approval.json; installation checks are in runtime-validation.json.

## Installed settings

Sources 210/211, player animation set 61, StrafeLeft/StrafeRight. Preserve in_place: false on both clip calibration and action bindings so lateral weight transfer survives. Source clips contain 31 samples at 30 Hz; runtime clips contain 14 samples at 13 Hz, retaining the duplicate endpoint. Q8 speed 276 gives a 0.999721-second loop including that endpoint interval. Existing movement speed is unchanged.

Rebake from the repository root:

```sh
target/release/import-locomotion editor/projects/default/project.ron editor/projects/default/source_assets/animations/player/cybernetic_strafe_review_v2 --pack gen --fps 30 --no-trim
```

Restore sources 210/211, approved tags, calibration in_place: false and action in_place: false / speed_q8: 276 after generic import. Build with target/release/frontend build-project-disc --project editor/projects/default/project.ron.

Validation passed for exact approved source hashes, cooked loop endpoints, timing/bindings, cooked studio pose inspection, native boot/gameplay, and zero mapped load-delay hazards. The native smoke route did not exercise lock-on strafe activation or world-speed foot contact; those remain unverified. All 76 other clip files retained their pre-import checksums.
