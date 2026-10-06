# Recovered animation pass — 2026-10-06

The approved work was authored in **Cortex Ignition v0.4b**, the repository's
`editor/projects/default` project. The later approved walk transitions were
parked on `wip/walk-transitions-v4`, commit
`52ee63d58db753e63d97ee3bc21b2da02be09538` (2026-10-01).
Its approval record says “better, let's use this”.

## What Reach already had

Byte-for-byte comparisons against that snapshot confirmed the approved forward
walk v7, backward walk v3, both side steps v2, Horizon light attack v5 with v12
head movement, Horizon heavy cross slash v2, and the two-hit combo v12 were
already bound in Reach. The original run remains selected; run experiments v1
and v2 were not approved. These clips still resolve through `../default`.
The animation register in that project records the earlier approvals.

## What was recovered

- `gen_walk_fwd_windup`: the longer preparation and moving handoff, 22 stored
  samples at 30 Hz, including the terminal sentinel; 0.667 s startup.
- `gen_walk_fwd_winddown` and its mirror: overlapping first-foot placement and
  second-foot adjustment, each 30 stored samples at 30 Hz; idle pose reached
  after 0.933 s. The mirrored variant reverses foot order.
- Their approved Blender/GLB sources, action options, calibration and provenance.
  Source resources 212–214 point into this project's `source_assets` directory.
- Missing cooker support for `preserve_samples`, plus the runtime handoff that
  avoids an extra held endpoint and frozen-pose crossfade.

The three transition clips retain authored translation, full sample rate and
Q8 action speed 256. Other clips retain the existing compression policy. Level
geometry, scene components, character model and all unrelated resources were
verified unchanged. Intro remains disabled so Play starts immediately in Reach.
The separate ongoing Aletha model/material work in Cortex Ignition 0.5 is not
part of this recovery.

## Provenance and rebaking

`validation/animation-transfer.json` records source hashes and the already
present clips. `source_assets/animations/player/cybernetic_walk_transitions_review_v4/`
contains the approved source snapshot, including its **historical** validation
reports. Those reports describe the original Cortex installation, not a fresh
Reach test. Current verification is recorded separately in `validation`.

The direct-matrix `bake.py` is required: generic GLB retargeting can apply bind
offsets again and change the approved poses. The approved walk Blender master
and raw reference animation have also been copied locally for that bake. The
original author/render comparison scripts additionally expect historical v3
review inputs; they are retained as provenance, not a standalone render pipeline.

`logs/project.ron.before-walk-v4` is the pre-transfer project backup.
The typed helper `restore_reach_walk_transitions` documents the exact migration;
`verify_reach_animation_transfer` checks the cooked samples, endpoint poses,
rotations and unchanged scene/resources. Regenerating Reach from the older Valley
seed with `tools/build_graybox_reach.py --replace` would overwrite these resource
bindings; reapply the transfer if deliberately rebuilding the fixture.

## Reach verification

The release editor/MCP and debug editor were rebuilt. The normal Play disc
completed 2,569 controller polls of repeated walk starts/releases, a light
attack/follow-up input, backward walking and side steps. Captures show gameplay
and the restored transitions. Cooked start-to-walk joint translations match
exactly; maximum rotation coefficient error is 1/4096.

All 83 runtime tests pass. The project suite passes 463 tests; two old enemy
fixture tests fail because they reference the deleted `quake-e1m1-geometry`
project. The dedicated transfer verification and full editor MCP audit pass
(the existing missing Aletha turn-clip warning remains).

The first disc was rejected by runtime asset checksum checks after an overlapping
project cook mixed build inputs. The final rebuild used matching generated
manifest inputs and completed both replays without an asset failure.

The normal main-route replay completed 4,250 polls and averaged **28.568 fps**,
versus the earlier **28.585 fps** baseline, with the same 50.599 ms 95th-percentile
frame interval. The longer startup slightly changes movement timing, so this is
a whole-route check rather than an exact matched-camera comparison. It does not
establish locked 30 fps or hardware performance.

`validation/animation-transfer.json` records the final disc hashes and results.
The review clip and frame sheet are under
`build/graybox-reach/animation-transfer/start-stop-review.mp4` and
`start-stop-contact-sheet.png`.
