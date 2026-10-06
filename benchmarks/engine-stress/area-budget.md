# Superseded area-budget experiment — 6 October 2026

**The 120-face / 240-triangle / one-enemy authoring cap below is withdrawn.**
Read [the normal Play recheck](valley-recheck.md) for the correction. Static PVS
candidate counts and instrumented fixture timings do not establish a universal
frame-rate limit. The MCP no longer applies these defaults. It reports costs and
compares optional explicit project budgets; runtime validation is separate.

Graybox Highlands and its dedicated authoring helpers were deleted on 6 October at the user's request. [Graybox Reach](../../editor/projects/graybox-reach/README.md) is the replacement mixed outdoor/interior fixture.

The remainder preserves the original experiment and reasoning as historical evidence.
Its calibration table is diagnostic data, not current authoring guidance.

---

## Original experiment (superseded)

Use `area_budget` in **psxed-mcp** before growing a section. The working ceiling is
**120 potentially visible cooked faces AND 240 base world triangles per camera leaf**,
including adjacent areas visible through entrances. Reserve for Aletha and **one Custodian per visibility area**. This is a conservative design limit, not the engine's storage limit
or a universal prediction of frame rate. There is no bonus for using fewer actors until
that workload is calibrated separately.

## Measurement and ceiling

The controlled outdoor sweep keeps the 16,384 × 16,384 court, sky, lighting, materials,
player, two enemies, and camera fixed; it adds box pillars. Release cook, patch extent
4096 authored units, original Picotron 64px opaque grid at 400% UV scale, DMA FIFO enabled,
`cd-stream-bench emulator-telemetry`. Each run has 1,800 pad polls. Analysis uses polls
600–1,750, after loading, and actual emulator display-start changes. It does not measure
host execution speed or count guest render calls as displayed frames.

| Pillars | Cooked faces | Display fps | p95 frame interval | Intervals >2 vblanks |
|---|---:|---:|---:|---:|
| 0 | 49 | 29.645 | 33.733 ms | 0 |
| 4 | 79 | 29.645 | 33.733 ms | 0 |
| 8 | 105 | 29.645 | 33.733 ms | 0 |
| 12 | 135 | 29.645 | 33.733 ms | 0 |
| 16 | 161 | 29.645 | 33.733 ms | 0 |
| 18 | 180 | 29.645 | 33.733 ms | 0 |
| 20 | 191 | 23.726 | 50.599 ms | 229 |

Every run rendered two enemy instances and had zero primitive overflows. The tested
stationary boundary is **180 passing / 191 failing**; intermediate counts are untested.
The 120-face authoring ceiling leaves one third below the largest passing fixture
(and about 25% below the 161-face fixture). The 240-triangle limit prevents a mesh with
higher polygon valence from fitting the face count while exceeding this quad-based
workload. These margins are planning reserves; they do not prove all combat effects,
close camera positions, transparent materials or different actors will fit.

Nominal PSX “30 fps” here means one displayed frame every two emulated vblanks:
33,868,800 bus cycles/sec ÷ 571,240 cycles/vblank ÷ 2 = approximately 29.645 fps.
A 33.33 ms literal threshold would incorrectly reject the engine's normal cadence.

The earlier stress suite also demonstrates actor dependence: 492 faces achieved
29.62 fps without enemies, 24.94 with one and 19.73 with two. Do not promote the
no-enemy result into a combat budget.

## Camera and actor load change the limit

The follow-up Highlands test matters more than the stationary maximum for level design.
With two Custodians in the same court, camera rotation plus repeated player attacks
achieved only **24.268 fps** (p95 50.599 ms), even with a cube sky and at most 99 PVS
faces in that court. The two model draws alone peaked at **1,137,296 bus cycles**,
against **1,142,480 cycles for the entire two-vblank frame**. Reducing world geometry
cannot rescue that frame. The profile therefore supports **one Custodian per visible
encounter**, and explicitly rejects a two-enemy assumption. This is a model-specific
limit, not a statement that all PSX games must have only one enemy.

The procedural Panorama sky also reached over 550k cycles in some traversal views.
The blockout now uses the existing Cube sky. The MCP reports sky mode and this caveat.
Two-enemy stationary evidence is retained above as a geometry calibration reference;
it must not be mistaken for a two-enemy combat guarantee.

## MCP authoring contract

```json
{"areas":[{"name":"Bridge exit","enemies":1,"samples":[[4096,1536,6016],[4096,1536,7168]]}]}
```

Call `area_budget` with the above arguments. Samples are **camera positions in authored
units**, including eye height, resolved through the cooked BSP tree. They are not
selected by approximate leaf surface bounds. Give each area multiple samples covering
camera orbits, entrances, exits and combat positions. Omitting areas returns whole-map
limits and the heaviest leaves. All current staged edits are cooked once.

The JSON includes a versioned profile, limits, cost and remaining margin per sample,
maximum per named area, over-budget leaf count, and calibration scope. Negative margin
means over budget. More than one assumed enemy is `unsupported_actor_load`.
A point in solid/exterior space is `invalid_sample`, never a zero-cost pass.
`audit(depth="full")` includes the same numerical ceiling without a second cook.

PVS faces are a conservative *candidate set* before frustum and backface culling.
Base triangles exclude sky and runtime clipping/subdivision. Neither equals the final
submitted packet count. A single large leaf can union the views from both ends of a
passage even when one camera cannot see both courts at once. Split such passages with
staggered terrain and remeasure; merely naming two areas does not divide their cost.
Keep separate sky caps and make occluding terrain reach them.

`within_budget_runtime_unverified` deliberately requires the next step: replay traversal,
transitions, camera rotations and the intended combat load. Require sustained two-vblank
display cadence, no primitive overflows and no missing geometry. This calibration is
emulator evidence, not a hardware certification.

## Reproduce

Run from the editor repository, with the current release frontend and psxed-mcp built:

```sh
python3 benchmarks/engine-stress/calibrate_areas.py 0 4 8 12 16 18 20
python3 benchmarks/engine-stress/measure_route.py build/area-calibration/pillars-18 600 1750
```

Generated test projects stay under `build/area-calibration`, outside the editor picker.
The sweep refuses to overwrite existing evidence; set `PSOXIDE_AREA_OUTPUT` to a fresh
directory for another run.
The raw runs retain MCP calls, exact project, build log, disc/frontend hashes, input
parameters, telemetry, display-route CSV and screenshots. Checked-in summaries and
per-case audits are in `results/2026-10-06/area-calibration/`. The sweep is stationary;
Highlands' separate traversal evidence lives with that project's validation files.

## Final authored example

Graybox Highlands now has 228 total world faces and at most 105 PVS faces in any leaf.
Named area maxima are 74 / 83 / 69 / 105 / 85 faces. The 69-face encounter court reserves
more than the generic 120-face ceiling because actor and camera costs matter.
Traversal measured 29.639 fps and the single-enemy orbit/attack test 29.527 fps; both
have a 33.733 ms p95 interval and zero primitive overflows. There are still occasional
50.599 ms frames (32/2549 and 18/502 respectively), so **neither is certified locked 30**.
The geometry limit remains provisional, and `within_budget_runtime_unverified` must not
be promoted to a pass. The remaining spikes need actor/render-scheduling investigation,
not a claim that a static face count guarantees the frame time. Exact evidence and
limitations are in `results/2026-10-06/area-calibration/highlands-runtime.json` and the
project's `validation/` directory.
