# Engine stress limits for the soulslike (2026-10-03; continued 2026-10-06)

**Latest findings (October 6):** the large two-enemy room reaches **29.62 fps** when
4,096-unit BSP patches and 400% texture scale are used together (136 cooked faces),
versus **19.73 fps** with the original settings (492 faces). The sky-sealed outdoor
courtyard reaches **26.23 fps** under FIFO timing. BSP far-vista settings currently
cook disabled, so their rendering cost remains unmeasured. See the
[continuation](#continuation--2026-10-06) for all 22 runs and limits of these conclusions.

> Historical baseline: the October 3 measurements below predate the world-redraw fix
> (`fe74ce65`) and queued presentation (`b85d4ea3`, `7d100449`). The October 6
> continuation records fresh measurements and supersedes the unresolved second-enemy
> diagnosis. Face budgets are fixture-specific estimates, not PS1 or BSP limits.

Original question: how much level can the engine draw at 30 fps with the player and two enemies on screen?
The October 3 fixture model estimated **150 visible world faces at 30 fps**, or about
**420 at 20 fps**. In that baseline, characters and fixed costs took two thirds of the
30 fps budget before any level was drawn. These estimates are historical, not current limits.

## Method

Synthetic projects, generated through psxed-mcp from the current `default` template:
- **Scene:** a sealed room, Draft (fullbright) cook unless lights are on. The player stands still
  at a fixed spawn, and two Intake Custodians idle in view (aggro radius patched to 1).
- **No menus:** boot is `Gameplay` and the authored loading screen is disabled, so the run reaches
  gameplay at tick 258 with no input at all. Earlier runs used scheduled CROSS taps, which leaked
  into gameplay on slow scenarios and moved the player. Those runs were discarded.
- **Build and run:** `EDITOR_PLAYTEST_FEATURES='cd-stream-bench emulator-telemetry'`, then
  `frontend launch --embedded-playtest --stop-at-poll 5400 --profile-log`.
- **Window:** the last 1,200 sim ticks, with no CD activity (gameplay only).
- **Cost accounting:** per presented frame, `visual_render_task`, plus `update` per 60 Hz sim tick.
  30 fps holds when one render plus two sim ticks fit two NTSC fields (1,130k cycles).
- **Faces:** "visible faces" is `audit depth=full`'s count for the spawn leaf, which is the number
  the audit already reports per leaf.
- **Where it lives:** harness, per-run profiles and frames are in
  `PSoXide/output/stress-2026-10-03/` (`gen.py`, `run.sh`, `analyse.py`).

## Results

| scenario | visible faces | render work | sim/tick | 30 fps load |
| --- | --- | --- | --- | --- |
| small room 8k x 12k, player + 2 enemies | 112 | 980k | 62k | 98% |
| medium room 16k x 20k | 232 | 1,117k | 62k | 110% |
| large room 24k x 28k | 492 | 1,647k | 62k | 157% |
| huge room 32k x 36k | 848 | 2,315k | 62k | 216% |
| large room, player only | 492 | 1,038k | 33k | 98% |
| large room, player + 1 enemy | 492 | 1,234k | 50k | 118% |
| large room + 8x4 pillar field | 884 | 2,365k | 64k | 221% |
| large room, 16 baked point lights | 492 | 1,662k | 62k | 158% |
| pillar field, 8 baked point lights | 884 | 2,380k | 64k | 222% |

**Model:** render work = 734k + 1,856 x visible faces, with the player and two enemies. It fits
all four room sizes within 4%. Simulation is about 62k per tick with two enemies, so about 14k per
enemy for AI. Two pillar fields larger than 16x4 displaced the player at spawn with no input; they
are excluded and listed under bugs.

**Budget:**
- **30 fps:** 1,130k - 2 x 62k = 1,006k for render, which leaves room for **about 150 visible faces**.
- **20 fps:** 1,695k - 3 x 62k = 1,509k for render, which leaves room for **about 420 visible faces**.

## What it means for level design

- **Characters dominate.** The first enemy adds 197k of render work. The second adds 413k, of
  which about 220k unexpectedly lands in the room band (see bugs). The player alone is roughly
  200k. Three characters plus fixed costs are about 734k, two thirds of the 30 fps budget.
- **Area counts as much as detail.** A visible face is a cooked face after geometric patch
  subdivision and any additional splits to fit the 255-texel UV window. One large wall is
  many faces, and the empty large room (492 faces) already runs at 157% of the 30 fps budget. At 30 fps, a space the player sees all of
  at once should be about the small room's size: 8k x 12k x 3k authored units, roughly 8 x 12
  player heights. Texture scale can reduce UV-driven splits, but the October 6 probe below
  shows that it does not reduce this fixture's geometric patch count.
- **Baked lighting was cheap in these fixtures.** 16 baked point lights changed measured
  render cost by +0.9%. This does not measure dynamic lights or bake time.
- **Distant scenery needs its own budget.** These room measurements did not test far-vista
  panels or simplified distant geometry. They do not establish that real distant geometry
  is impossible; the original recommendation to allow only sky/cards was too strong.

## Original levers (historical; revised below)

1. **The second-enemy anomaly (about 220k).** Adding the second enemy raised the room band from
   714k to 934k with identical geometry. If that's a bug, fixing it nearly doubles the 30 fps face
   budget (to about 265 faces).
2. **Character cost (about 200k each).** Distance LOD or decimation for enemies; one 5,000 units
   away costs as much as one up close. The 2026-09 survey already had a 0.6 decimation probe.
3. **Per-face cost (about 1.9k cycles).** July's corridor study measured about 1.6k per considered
   surface: the engine spends its time deciding, not drawing. This is the engine-side campaign.
4. **Identify the splitting limit before changing texture scale.** Larger texture scale can
   remove UV-window splits, but it does not bypass the geometric BSP patch-extent limit.
   The October 6 continuation separates these controls.

## Bugs found

- **Player displaced at spawn in dense geometry.** In 16x8 and 24x10 pillar fields the player is
  not at the spawn after loading, with no input. Not investigated.
- **Second enemy adds about 220k to the room band** with identical world geometry (base vs one
  enemy). Resolved for this fixture by the later redraw/scheduling changes; see October 6 below.
- **The player spawns facing +Z** (`player_view_yaw_q12` 2048). The 2026-07 stress notes say -Z.
- **The `fixed_update_task` profile column is always 0**; per-tick simulation cost is in `update`.

## Proposed gate

`audit depth=full` should flag any leaf whose predicted render work, 734k + 1,856 x visible faces,
exceeds the chosen frame budget, and show the worst leaf as a percentage of it. Re-fit the
constants whenever the engine changes, using the harness above.

## Continuation — 2026-10-06

### Scope and reproducibility

Eleven stationary fixtures were built and run under both GPU DMA models (22 completed
runs). Every fixture uses the existing player and zero, one or two Intake Custodians,
with the original no-input, aggro-radius-1 setup. This is an engine/content cost study,
not a combat benchmark. The new fixtures are separate `stress-20261006-*` projects;
the October 3 projects and measurements are preserved.

Guest source is editor `7d100449e9721d6e31c29201245e732764c07770`, including the
pre-existing uncommitted `present-queue` default-feature edit. Extra build features are
`cd-stream-bench emulator-telemetry`. SDK pin: `942bc0267`; emulator pin in the component
lock: `9eff8c8f`. The existing host frontend binary is fingerprinted in every manifest;
these are measurements of that binary, not a claim about an independently rebuilt host.

Each run stops at 5,400 controller polls, allowing the final presentation to advance a
few polls. Analysis uses the last 1,200 completed telemetry rows and aligns emulator
route/GPU records to their bus-cycle window. No room-chunk loads or BSP redraw counter
increments occur in any measured window. GPU timing is the emulator's estimated work
in CPU/bus cycles, not a hardware capture.

- **FPS** counts display-start changes in the route log, rather than guest render calls.
  The nominal 30/20 fps cadences here are about **29.62/19.75 fps**, because of this
  emulator's video period. A 33.73 ms display interval is two fields; 50.60 ms is three.
- **Render/world cycles** are means per render call. Stage timers may include waits;
  parent and child timers overlap and must not be added together. In particular, a
  scheduling change can move a wait between the world and model stages.
- **Cooked faces** are total world records, not the number of triangles submitted or
  pixels drawn. The unsealed courtyard includes exterior/underside faces. These totals
  cannot be substituted directly into the old single-room linear model.
- DMA is explicitly `PSOXIDE_EXPERIMENTAL_DMA_FIFO=0` for legacy and `=1` for FIFO.
  Leaving the variable unset is not an independent control in the current implementation.

Reproduction scripts and frozen baseline projects are in
[`benchmarks/engine-stress`](../benchmarks/engine-stress/README.md). Committed evidence:
[summary](../benchmarks/engine-stress/results/2026-10-06/summary.json),
[full table](../benchmarks/engine-stress/results/2026-10-06/results.md),
[build fingerprints](../benchmarks/engine-stress/results/2026-10-06/provenance.json),
and [audits](../benchmarks/engine-stress/results/2026-10-06/audits).
Full local CSVs, discs and display captures remain in the sibling SDK checkout at
`PSoXide/output/stress-2026-10-06/`; these large generated outputs are not checked in.
Each scenario retains its authored project, MCP edit
receipt, audit, build log, source revision/diff, frontend hash and disc hashes. Each run
retains profile, counter, route, GPU and CD logs, launch arguments and screenshots.
Software display captures at route ticks 1,800/3,600/5,400 provide scene checks. Camera
position telemetry is a constant sentinel in this BSP path and was not used as position
proof. These are not pixel-equivalence tests across different workloads.

### Results

| Fixture | Cooked faces | Legacy fps | FIFO fps | FIFO render kcycles | FIFO world kcycles | p95 display interval |
|---|---:|---:|---:|---:|---:|---:|
| Large room, player only | 492 | 29.62 | 29.62 | 1021.6 | 722.9 | 33.73 ms |
| Large room, +1 enemy | 492 | 24.94 | 24.94 | 1216.4 | 730.1 | 50.60 ms |
| Large room, +2 enemies | 492 | 19.73 | 19.73 | 1500.2 | 723.6 | 50.60 ms |
| Small room, +2 enemies | 112 | 29.62 | 29.62 | 954.1 | 270.5 | 33.73 ms |
| Large, texture scale 400% | 492 | 20.44 | 20.41 | 1450.8 | 723.5 | 50.60 ms |
| Large, patch extent 4,096 | 492 | 19.73 | 19.73 | 1500.2 | 720.2 | 50.60 ms |
| Large, both changes | 136 | 29.62 | 29.62 | 914.7 | 231.1 | 33.73 ms |
| Open courtyard, unsealed | 612 | 25.49 | 24.58 | 1206.2 | 478.7 | 50.60 ms |
| Same courtyard, sky sealed | 225 | 27.12 | 26.23 | 1128.8 | 407.6 | 50.60 ms |
| Unsealed +8 vista panels (inactive) | 612 | 25.49 | 24.58 | 1206.2 | 478.7 | 50.60 ms |
| Unsealed +16 vista panels (inactive) | 612 | 25.49 | 24.58 | 1206.2 | 478.7 | 50.60 ms |

The small room still holds the nominal 30 fps cadence. The large two-enemy room now
holds the nominal 20 fps cadence, while the one-enemy case alternates two- and three-field
presentations. An average near 25 fps is therefore not a steady 40 ms cadence.

### The original second-enemy anomaly is explained

`fe74ce65` prevents a predicted packet-arena shortage from drawing the world and then
redrawing it after a fence. `7d100449` places the world before or after other passes
according to available arena space; `b85d4ea3` supplies queued presentation.

The fresh large-room world stage is about **723–730k cycles** with zero, one or two
enemies, versus **934k** with two in the old report. The redraw counter stays zero.
Displayed FPS increases from the original **19.8 to 24.94** with one enemy (~26%) and
**17.3 to 19.73** with two (~14%). These old/new comparisons include the later engine
commits and the preserved feature configuration; they are not isolated A/B measurements
of each individual commit.

The two-enemy model stage is still more expensive than simply twice the one-enemy
stage. The emulator estimates GPU work at roughly 24–33% of elapsed cycles across these
fixtures; this does not suggest a saturated rasterizer. Because stage timers can
include packet-arena waits, this report does not assign
all of that residual to animation or skinning. The original claim that removing 220k
from the room stage automatically doubles the usable face budget is superseded.

### Two subdivision limits must be tested together

The four controlled large-room fixtures vary only texture scale and BSP patch extent:

- Baseline: extent 2,048, texture scale 100%, **492 cooked faces**.
- Texture scale 400% alone: **492 faces**; world cost remains about 724k cycles.
- Extent 4,096 alone: **492 faces**; world cost remains about 720k cycles.
- Both together: **136 faces**, a **72.4% reduction** in cooked face records.

The cooker first makes geometric patches and then fits their texture coordinates into
the 255-texel window. Relaxing either limit alone allows the other to keep this fixture
split. This corrects the original advice that increasing texture scale alone directly
reduces its face count. The combination reaches **29.62 fps** (about 50% above baseline); world cost falls
from **723.6k to 231.1k cycles** (~68%) and total render work from **1,500.2k to
914.7k cycles** (~39%). These gains apply to the combined configuration, not to
texture scaling alone.

Neither adjustment is visually neutral: 400% scale makes the texture pattern coarser;
larger patches change interpolation and lighting sampling, and may need additional
runtime subdivision near the camera. The stationary results do not establish acceptable
affine distortion or crack behavior during movement. These are fixture settings, not a
change to engine defaults or a recommendation to apply the maximum extent globally.

### Open-looking space versus an actual BSP leak

The outdoor floor has the same 24,576 by 28,672 footprint as the large room, with four
768-unit-high boundary walls, no visible roof, two enemies and 400% texture scale.
The unsealed version intentionally opens to the void. Its audit reports a leak,
18 non-solid leaves and 612 world faces. It runs faster than the enclosed 492-face room
because the geometry that survives runtime selection/projection is different. Total
face count alone is not a reliable cross-layout performance predictor.

`outdoor_sealed` adds five sky-aperture brushes above the boundary walls and across
the top. These close the outside flood while preserving the sky view; sky apertures
participate in topology but do not submit ordinary textured world polygons. Its audit
confirms a sealed world with one non-solid leaf and 225 face records (including five
sky apertures). FIFO frame rate rises from **24.58 to 26.23 fps** (~6.7%) and world
cost falls from **478.7k to 407.6k cycles** (~14.9%). The inspected display retains
the open sky and courtyard. This helps, but still does not hold the 30 fps cadence.

All new tests use Draft/fullbright cooking. They do not measure Release portal-flow
visibility on a connected landscape with occluding hills, corridors and buildings.
The courtyard proves one stationary outdoor composition can run; it does not prove
that an arbitrary landscape, viewpoint or encounter has the same cost.

### Far-vista settings currently produce no BSP workload

Both the 8- and 16-panel projects enable the ring and select existing sky texture
resource 21. The BSP cooker nevertheless creates `PlaytestFarVista` with
`texture_asset_indices: Vec::new()` and `flags: 0`
(`editor/crates/psxed-project/src/playtest.rs`, BSP room construction). The saved
[emitted record](../benchmarks/engine-stress/results/2026-10-06/far-vista-emitted-record.txt) confirms `texture_assets: &[]` and `flags: 0`, even
though the authored project is enabled and has 16 segments.

The runtime returns immediately when the enabled flag is absent. The tiny far-vista
stage is therefore call/timer overhead, **not the cost of drawing distant scenery**.
For both DMA modes, both enabled probes' tick-5,400 display captures are byte-identical
to the disabled courtyard. Their frame rate and GPU work also match it.

This is a reproduced inactive feature path. Far-vista drawing must be connected through
the BSP cooker before the panel budget can be measured. Even once enabled, the existing
ring is camera-relative backdrop geometry; it does not provide world-space parallax,
reachable distant terrain, mesh LOD or streaming. No such capability was benchmarked here.

### Next measurements that would change an engine decision

1. Wire the far-vista texture assets and flags through the BSP cook, then repeat the
   paired disabled/8-panel/16-panel probe and verify actual submitted geometry.
2. Replay a moving third-person camera through the coarser-patch fixture, including
   near walls and floor edges, to judge interpolation, clipping and frame-time peaks.
3. Build a representative Release-cooked outdoor route with elevation, an occluding
   ridge, a distant reachable landmark and active combat. Measure visibility, character
   cost and presentation cadence together before selecting a landscape architecture.
4. Validate the chosen workload on a console. Neither DMA lane substitutes for silicon.

The measured evidence supports continuing the current engine while addressing its cook
and representation limits. It does not establish a universal 150/420-face budget, make
far-vista panels free, or prove that distant real geometry requires an engine migration.
