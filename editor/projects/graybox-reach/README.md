# Graybox Reach

Open `project.ron` in the PSoXide editor and press **Play**. This replaces the
retired Graybox Highlands experiment. Graybox Valley remains available as the
smaller comparison. Shared character, animation and UI assets resolve through
`../default`; keep that project alongside this one.

## Stance changes while moving, 2026-10-06

Tap **Triangle** while walking, or hold **Circle** to run and tap **Triangle**.
Both keep locomotion active while the body and scarf burst and reconstruct.
The authored swap cooldown is 300 ticks (five seconds at 60 Hz).

The saved disc was verified with a poll-bound walking/running replay under
strict DMA FIFO through poll 1712. This confirms existing behavior; no runtime
change was required. See the [1080p replay](validation/moving-stance/walking-running-stance-1080p.mp4),
[input tape](validation/moving-stance/walk-run.pxtape), and
[validation details](validation/moving-stance/report.json).

## Physical scarf, 2026-10-06

The optimized renderer batches the scarf's solid Gouraud triangles while retaining
all 16 particles, two solver passes, 30 triangles, shading and stance fragments.
Against the frozen folded-wrap build, full recorded gameplay improves from
26.068 to 26.198 fps (strict DMA: 26.068 to 26.155). The heavy dash at poll 1688
improves from 20.184 to 20.401 fps in both modes. This is a modest saving;
the route still does not hold 30 fps. Final gameplay and stance-review images
match the baseline exactly, and 880 isolated engine/BSP/runtime tests pass.
Four additional solver experiments were rejected for insufficient gains.
See [optimization measurements](validation/scarf/optimization.json) and
[stance comparison](validation/scarf/optimization-stance-review.png).
Timings isolate the scarf from concurrent camera/enemy work; physical-console
performance remains unverified. Rebuild & Play picks up the optimized code.

The neck wrap now uses uneven cloth contours: a narrow upper opening and a
wider lower edge with an off-centre front dip. This replaces the circular tube
while retaining 30 total scarf triangles. The [1080p close-up video](validation/scarf/folded-wrap-1080p.mp4)
shows the native PS1 image enlarged sharply; the [4x internal-render still](validation/scarf/folded-wrap-front-4x.png)
provides a cleaner geometry inspection. The complete standing/moving/both-stance
replay and all 260 runtime tests pass. See [capture details](validation/scarf/folded-wrap-report.json).

Follow-up: the standing tip now hangs below the waist. The initial implementation
used the lifted visual model origin as its floor; the solver now receives the
motor's actual floor height. The wrap is lower by 5% of character height so it
sits below the jaw. Its triangles now burst and reassemble on the body's stance
clock, then retain the active stance color while the body fades to silver.

260 runtime tests pass, including regressions for the hanging tip and cloth
burst/arrival timing. The recorded user route completes at 26.198 fps average;
a dedicated stop-and-switch recording verifies both stances in normal and strict
DMA modes. See [follow-up evidence](validation/scarf/revision-report.json),
[standing comparison](validation/scarf/revision-standing.png), and
[stance recording](validation/scarf/revision-stance.gif). Measurements below are
the original implementation's frozen comparison.

Aletha now has a neck wrap and a simulated tail attached to her animated neck.
The scarf follows the active stance: Horizon orange `(255, 113, 58)` or Zenith
turquoise `(108, 224, 198)`. The body still restores its neutral reflective
appearance after a stance change; the scarf retains the stance hue. Press
**Triangle** to switch, then run, turn and dash to see the tail react.

The model opts in through the `scarf_neck` socket on joint 9. The ribbon uses
16 fixed-point particles, two position-constraint passes at 30 Hz NTSC / 25 Hz
PAL, and 30 opaque double-sided triangles including the wrap. The attachment
updates every gameplay tick. Gravity, inertia, damping, torso and floor
collision run independently of rendering. Teleports reset the cloth. There is
no extra texture or VRAM allocation. Wall and self collision are not simulated.

The final normal Play build was compared with a control compiled from the
same frozen source and cooked assets, with only the scarf socket lookup disabled:

| Saved user tape, polls 400–3120 | Display cadence |
| --- | ---: |
| Scarf disabled | 27.267 fps |
| Scarf enabled | 25.893 fps |
| Scarf enabled, strict DMA FIFO | 25.849 fps |

This is approximately a 5% cadence cost, not a locked 30 fps result. Both full
replays completed through poll 3143. Six new scarf tests cover movement,
constraints, torso collision, teleports, fixed PAL/NTSC stepping, persistent hue,
near clipping and packet capacity; all 872 native engine/BSP/runtime tests passed
at feature validation. The final guest passes load-delay and scratchpad guards
(the cloth call tree uses 160 of 1004 available bytes). Physical-console timing
and runtime primitive-overflow telemetry remain unverified.

The saved disc includes the scarf. Use **Rebuild & Play** in Graybox Reach.
See [measurements](validation/scarf/report.json),
[stance captures](validation/scarf/stances.png), and
[movement recording](validation/scarf/scarf.gif).

## Dash follow-up, 2026-10-06

The second pass caches the fixed wire edges and batches their packets, skips
invisible body submission on the capture frame, projects departing fragments
on the GTE with CPU clipping fallbacks, and prepares reconstruction math once
per frame on a guarded scratchpad stack. Effect geometry, fragment count,
colors, animation and recovery timing remain intact. GTE projection can differ
slightly in fixed-point rounding from the previous CPU projection.

The saved user tape was compared against the previous reflection optimization
using **identical frozen reduced-enemy assets on both sides**:

| Window (starting poll; 96 polls) | Previous | Follow-up |
| --- | ---: | ---: |
| Heavy dash, 1688 | 20.184 fps | 22.076 fps |
| Heavy dash, 1791 | 20.596 fps | 22.951 fps |
| Outdoor dash, 729 | 27.461 fps | 29.333 fps |
| Later dash, 2525 | 27.122 fps | 29.645 fps |
| Gameplay, polls 400–3120 | 26.962 fps | 27.376 fps |

The two heavy windows contain **11 → 1 intervals longer than 50.6 ms**.
Their strict-FIFO counterparts improve from 20.184/20.815 to 21.912/22.556 fps,
with no intervals above 50.6 ms. Both full replays complete and pass visual
review. **Busy rooms still do not sustain 30 fps.** Diagnostic attribution
puts player rendering during the effect at approximately 12 ms, down from
20–24 ms; world and enemy drawing still leave total rendering near 39 ms.
Diagnostic timings are separate from the uninstrumented acceptance results.

866 unit tests pass, including line-packet/depth equivalence, every reconstruction
age, clipping/overflow fallback, and GTE projection/saturation cases. MIPS
load-delay and scratchpad guards pass. The edge cache adds 2,072 bytes of BSS;
text grows by 7,016 bytes over the second-pass control. Physical-console timing
is unverified. See [replay evidence](validation/dash-optimization-followup.json)
and [visual comparison](validation/dash-optimization-followup.png). Open this
project and use **Rebuild & Play** to test the changes.

## First dash optimization, 2026-10-06

Dash reconstruction disabled triangle splitting and consequently bypassed the
fused reflection renderer. It now uses that renderer when projection proves
the whole model is in front of the camera and within hardware limits. Unsafe
bounds retain the original general path. The scatter, wireframe, reflection
texture, recovery timing and gameplay are preserved.

On the saved user recording, fixed 96-poll windows around the two heaviest
dashes improve from **17.400 to 19.763 fps** and **18.099 to 20.401 fps**.
Their longest displayed interval falls from **101.197 to 67.465 ms**. An outdoor
dash window improves from 23.092 to 27.461 fps. Across polls 400 to 3120, mean
display cadence improves from 26.220 to 26.766 fps. Strict DMA FIFO confirms
the gain: 26.177 to 26.678 fps over that same window. Heavy dashes still dip
below 30 fps; this is an improvement rather than a locked-cadence result.

All 861 engine, BSP and runtime unit tests pass. The regression compares the
actual packet words, depth slots and semantic counters against the old unsplit
renderer. Both complete DMA-mode replays pass visual review; the last default
checkpoint is byte-identical. PS1 load-delay and scratchpad checks pass, static
RAM is unchanged, and code grows by 228 bytes. Hardware timing and primitive
overflow telemetry are unverified. See [full evidence](validation/dash-optimization.json)
and [dash captures](validation/dash-optimization-comparison.png). Use **Rebuild & Play**
to replace any already-running game with the optimized version.
These measurements use the original enemy assets; the separate enemy-reduction
work that arrived afterward is outside this dash-only comparison.

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
psoxide-perf engine-stress measure-route build/graybox-reach/replay 650 4230
```


## HRZ melee / ZTH crystal cannon checkpoint, 2026-10-07

Graybox Reach now binds HRZ to R1 light combos and R2 heavy melee. In ZTH,
hold L2 to ready the right-hand cannon and press R2 for each shot. R3 lock-on
does not replace L2. Free aim follows the camera centre. Triangle still changes
stance; Circle evades and cancels aim. Shots use the existing opposing-stance
damage rules, without a firearm parry or a melee-to-ammo replenishment loop.
Enemy stance presentation is unchanged.

The 176-triangle crystal iris cannon encloses the right hand; the left hand
braces underneath. Two three-petal collars counter-rotate around a fixed core.
Its 128x128, 4-bit gradient atlas uses native PS1 average blending for dark and
middle tones while keeping near-white highlights opaque. The procedural scarf
and stance fragments share crystal gradients in mint ZTH and amber HRZ.
HRZ remains melee and never displays an orange cannon.

Six aiming clips and their Blender source live in
`assets/animations/zenith_ranged_v1/` and
`source_assets/animations/player/zenith_ranged_v1/`. The selected iris study,
stance preview, texture generators and exporter live in
`source_assets/animations/player/zenith_cannon_v2/`. To regenerate the current
cannon, run `texture.py`, run `export_game.py` in headless Blender 4.4 with
`--factory-startup --python-exit-code 1`, then run `cook_texture.py` with Python.
The existing project binds its three-joint skeleton and looping collar clip.
The older v1 `install.py` is a historical prototype installer and overwrites
the iris mesh; do not run it to rebuild this checkpoint.

Aimed forward/backward/sideways walking uses baked full-body clips with the
ready upper body and the original lower-body motion. Firing has its own event
clock and samples the live hand socket, so it neither stops movement nor resets
the gait. Standing shots have recoil; additive moving recoil, vertical upper-body
aim, and a visible hand-to-cannon transformation remain animation polish work.
The current cannon appears when aiming and retracts when L2 is released.

Validation: 288 tests against the staged runtime snapshot, four input-buffer/readiness tests, and two Graybox
cook tests passed. The diagnostic controller replay recorded ten shots and ten
hits, no shots from locked R2 without L2, all four aimed gaits, and five shots
with uninterrupted movement. A separate free-aim/cancel tape also ran. The
ordinary guest was rebuilt, hazard-patched, stack-checked and replayed through
2,400 controller polls. These are emulator checks; console validation is pending.

The recorded 34-second encounter is
`validation/zenith-ranged/video/arm-cannon-gameplay.mp4`; the control tapes,
logs and validation JSON are in `validation/zenith-ranged/`. With a diagnostic
disc built using `EDITOR_PLAYTEST_FEATURES='cd-stream-bench emulator-telemetry'`,
`tools/zenith_ranged/record_video.py --disc <cue>` repeats the recording, and
`tools/zenith_ranged/verify_capture.py` checks its control/movement evidence.
Normal Play uses `baked/graybox_reach.cue` built without emulator telemetry.

The current 26-second gameplay review is
`validation/zenith-cannon-v2/in-game/crystal-gameplay.mp4`.
Its delivery manifest records the asset/disc hashes, native resolution and
capture timing. The full replay tape and recording script remain available.
This is an iteration checkpoint: collar readability, the material response at
native resolution, and the hand transformation still need visual polish.
