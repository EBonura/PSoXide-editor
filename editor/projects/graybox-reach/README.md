# Graybox Reach

Open `project.ron` in the PSoXide editor and press **Play**. This replaces the
retired Graybox Highlands experiment. Graybox Valley remains available as the
smaller comparison. Shared character, animation and UI assets resolve through
`../default`; keep that project alongside this one.

## World texture-state fix, 2026-10-06

The user's recorded Play run reproduced black triangular gaps near the camera
and missing grid textures in the covered hall. Aletha's draw packets left a
texture window active. World polygons interleaved with those packets inherited
it and sampled the reflection atlas with the grid palette. Each page-local world
polygon now resets the texture window in its own GPU packet. This restores the
floor and wall textures without changing the level, character or near plane.

The recording completes in both DMA modes. Across polls 400 to 3120, default
DMA averages **26.220 fps**, compared with 26.264 before the fix. Strict FIFO
averages 26.177 versus 26.220. Both have 50.599 ms p95 intervals and approximately
101.2 ms maximum intervals on this recording, so this remains below a locked
30 fps. These are guest display-cadence measurements; the supplied 3,165-sample
CSV contains aggregate host timings and cannot establish guest framerate.

All 162 BSP and 451 engine unit tests pass, including the texture-window packet
regression. The rebuilt PS1 guest passes load-delay and scratchpad-stack guards.
Default replay captures at every 100 ticks and strict FIFO captures every 500
ticks show the reported world artifacts resolved. Physical-console validation
and primitive-overflow telemetry remain unavailable.

See [before/after captures](validation/texture-state-before-after.png) and
[measurements and packet evidence](validation/texture-state-fix.json). Replay
`validation/texture-state-user.pxtape` from the normal project spawn, stopping at
poll 3143. The rebuilt disc is ready for Play.

## Recovered animations

The approved Cortex Ignition v0.4b walk-transition v4 pass is installed, including
the longer startup, both two-foot stops and the cook/runtime support preserving
their timing. See [ANIMATION_TRANSFER.md](ANIMATION_TRANSFER.md) for its source
branch, exact transfer, saved masters and current validation. Other approved
locomotion and attacks were already present. Use the rebuilt editor when opening
this project. The performance table below records the earlier geometry baseline;
the post-transfer main-route check measured 28.568 fps. Current expansion results are below.

## Character model and material

Aletha now uses the exact **458-triangle / 235-vertex closed-fist model** and
reflective material from **Cortex Ignition 0.5**. The 26-joint skeleton, sockets,
restored animations, action timing and level are preserved. Model and texture
assets are local to Reach; the matching Blender and GLB masters are saved under
`source_assets/characters/aletha_closed_458`.

Before the expansion, after optimizing reflection submission, normal Play averaged **27.830 fps**
on the main route and **29.412 fps** on the side route, up from 25.412 and
28.706 with the same 458-triangle model/material. The stricter DMA FIFO test
improves from 25.296 to 27.747 fps on the main route. The 30 fps target remains
unmet in heavier views; physical-console performance is unverified.

The renderer now culls/maps/emits eligible facets in one traversal, uses its
guarded scratchpad stack, removes redundant quantization divisions, and keeps
mapping in the walker to avoid per-face call overhead. Tests preserve the old
UV coordinates, packet contents, draw order and counters. The model, texture,
animation assets and project settings are unchanged. All 451 engine tests pass,
as do the guest stack/load-delay checks and native playback. Details and hashes
are in `validation/reflection-optimization.json`; model provenance remains in
`validation/model-458-transfer.json`. Older texture/model performance records
are historical comparisons.

## Layout

The expanded playable bounds span 57,344 × 131,072 authored units, or **56 × 128
player heights** (previously 40 × 76). The original Picotron 64×64 grid is used at 400% face scale: a tile
spans 4,096 authored units, and its label denotes texture pixels.

- Arrival basin: 32 × 24 player heights, with low ruins, a hillside, gate towers
  and an accessible raised landing.
- Roofed gatehouse and great hall: a broad central aisle, four columns, a west
  dais and a screened exit leading into the northern basin.
- Northern basin: 32 × 32 player heights, two Custodians, a central headland,
  branching paths and a raised shrine reached by ramps at both ends.
- Alternate east loop: an eight-player-height-wide roofed gallery, balconies
  overlooking both basins, and long ramps connecting to ground level.
- New far court: 32 × 28 player heights, with a screen ruin, beacon dais, and
  a ramp to the western landing. The roofed northern gate bends between the
  basins to stop direct long views; its narrowest passage is four player heights wide.
- New western return: a raised covered route, a 16 × 12 player-height hall,
  and bends connecting the far court back to the original shrine. This forms
  a second complete loop. No additional enemies were added.

Geometry was authored through `psxed-mcp`; `tools/build_graybox_reach.py` records
its original construction. `tools/expand_graybox_reach.py` records the incremental
expansion while preserving character resources. The scene has 89 brushes in named
groups, seven baked lights,
a 4,096-unit BSP patch extent and a 60,000-unit draw distance. The editor opens
with an overview; Play starts Aletha in the arrival basin facing the gatehouse.

## Current expansion validation — 2026-10-06

The editor MCP audit passes sealing, degeneracy, extent, coplanar-overlap and
64-unit-grid checks. The expanded cook contains **1,051 world faces and 92
non-solid leaves**. The original two collision routes and the complete new
17-leg loop in both directions pass with the 188-radius, 1,024-height player hull. The existing
missing Aletha turn-clip warning remains. The full resource section is byte
identical to the pre-expansion project, preserving the 458-triangle model,
reflection material and recovered animations.

Normal Play, without guest telemetry, using the same host display-cadence
measurement and default DMA mode as the pre-expansion comparison:

| Test | Mean displayed fps | p95 / maximum frame interval |
| --- | ---: | ---: |
| Original main route | 27.117 | 50.599 / 50.599 ms |
| Original side route | 29.285 | 33.733 / 50.599 ms |
| New far court: traversal and camera sweep | 29.645 | 33.733 / 33.733 ms |
| New western return: traversal and hall sweep | 29.645 | 33.733 / 50.599 ms |
| New gate: camera sweep | 27.357 | 50.599 / 50.599 ms |

29.645 is nominal 30 fps at this video cadence. The court maintained that cadence
throughout its measured window. The return includes occasional longer intervals;
the gate and original main route remain below target. Main/side performance before
expansion was 27.830/29.412 fps. This is an expanded playable test level, not a
locked-30 result or an enemy-capacity guarantee.

The first expansion layout reduced the old main route to 23.762 fps. Revising
the gate's bend recovered it to 27.117 fps without reducing the outdoor court.
The court's worst sampled PVS fell from 606 to 281 candidate faces. This is a
useful example of MCP counts guiding changes, while runtime measurements decide
acceptance. PVS counts omit camera direction, actual submitted geometry, dynamic
actors, animation and material cost; there is no calibrated universal face cap.

New-area checks use temporary copies outside the project picker, with the same
resources and geometry but a local player spawn. They cover the gate sweep,
court traversal/sweep, and southern covered return/hall sweep; they are not an
end-to-end gameplay recording of the full new loop. That full loop is separately
verified with the engine collision walker. No sustained combat or physical-console
benchmark was performed. Missing primitive-overflow telemetry remains unknown.

See `validation/expansion-performance.json`, `expansion-routes.json`, the
`expansion-*.pxtape` files, and `expansion-area-budget.json`. The temporary spawn
coordinates and replay windows are recorded in the performance JSON; do not run
those new-area tapes from the ordinary arrival spawn and treat them as equivalent.
Raw captures, logs and temporary projects stay in `build/graybox-reach/expansion/`.

## Historical initial geometry baseline — 2026-10-06

The full editor MCP audit passes geometry and sealing: no degenerate brushes,
extent violations, coplanar overlaps or leaks. Both complete collision routes
pass with the standard 188-radius, 1,024-height player hull. Cooked geometry has
642 world faces and 66 non-solid leaves. The existing missing Aletha turn-clip
warning remains.

Normal Play guest (`cd-stream-bench`, **without** `emulator-telemetry`), default
DMA model (`PSOXIDE_EXPERIMENTAL_DMA_FIFO=0`), host display-flip measurements:

| Test | Mean displayed fps | 95th-percentile frame interval |
| --- | ---: | ---: |
| Arrival to great hall, polls 650–1800 | 29.568 | 33.733 ms |
| Hall resting view, polls 2000–2180 | 28.817 | 33.733 ms |
| Hall exit and northern traversal, polls 2300–3700 | 27.884 | 50.599 ms |
| Northern camera sweep, polls 3800–4230 | 28.187 | 50.599 ms |
| Main route plus sweep, polls 650–4230 | 28.585 | 50.599 ms |
| Side-gallery traversal, polls 2200–3300 | 29.645 | 33.733 ms |
| Side route plus sweep, polls 650–4850 | 29.532 | 33.733 ms |

29.645 is nominal 30 fps at this video cadence. This is a playable large-level
baseline, **not a locked-30 guarantee**: the measured northern views include
50.599 ms frames. Two enemies are present, but these tapes are traversal and
camera checks, not sustained combat benchmarks. Physical-console performance
has not been measured. Normal guests do not expose primitive-overflow counters;
those values remain unknown rather than being reported as zero.

The initial straight-through hall approach averaged 24.429 fps. Adding a screen
wall at the rear of the hall improved the same input window to 29.568 fps while
keeping the outdoor dimensions unchanged. Total cooked faces increased from
640 to 642: sightlines, camera direction and submitted work matter more than a
single global face count. Raising patch extent to 8,192 was a no-op because the
editor clamps it to 4,096; that test produced a byte-identical disc.

`validation/` contains the full audit, per-area PVS samples, collision routes,
normal-Play measurements, replay tapes and source/disc hashes. Use the sample
areas with MCP `area_budget`; no universal face or enemy limit is assumed.
Raw route/GPU logs and screenshots are under `build/graybox-reach/` outside the
project picker. The main replay visibly reaches the northern basin; the side
replay covers its raised gallery and landing. The exact full ramp/return routes
are separately verified by the engine collision walker.

## Reproduce

From the PSoXide-editor repository root:

```sh
EDITOR_PLAYTEST_FEATURES=cd-stream-bench PSOXIDE_GUEST_STAGE_ROOT=/tmp/psoxide-graybox-reach \
  target/release/frontend build-project-disc --project editor/projects/graybox-reach
mkdir -p build/graybox-reach/replay
PSOXIDE_EXPERIMENTAL_DMA_FIFO=0 target/release/frontend launch \
  --path editor/projects/graybox-reach/baked/graybox_reach.cue --embedded-playtest \
  --input-tape editor/projects/graybox-reach/validation/main-orbit.pxtape \
  --stop-at-poll=4250 --steps=8900000000 \
  --route-log build/graybox-reach/replay/route.csv
python3 benchmarks/engine-stress/measure_route.py build/graybox-reach/replay 650 4230
```
