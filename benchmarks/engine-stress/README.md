# Engine stress fixtures

Continuation of the [engine stress report](../../docs/engine-stress-limits-2026-10-03.md).
The October 6 evidence covers eleven stationary scenes under two explicit DMA models.
The committed [results](results/2026-10-06/results.md), [summary](results/2026-10-06/summary.json),
[audits](results/2026-10-06/audits) and [build fingerprints](results/2026-10-06/provenance.json)
retain the measured baseline. Full CSVs and screenshots remain local.

## Reproduce

Run from the editor repository root with the locked components bootstrapped, the
release `frontend` and `psxed-mcp` binaries built, default-project assets available and
`psoxide-perf` built (`cargo build --release -p psoxide-perf`, binary `target/release/psoxide-perf`):

```sh
psoxide-perf engine-stress run e0 e1 e2 small2 scale4 outdoor vista8 vista16 patch4k patch4k_scale4 outdoor_sealed
psoxide-perf engine-stress analyse > build/engine-stress/results.md
```

`psoxide-perf mcp-client --project DIR --tools` lists the `psxed-mcp` tools; with a calls file
(`[{"tool": name, "args": {...}}, ...]`) it runs a batch in one server process. The runner and the
calibration sweep use the same client. `PSOXIDE_PERF_REPO` points the tools at another checkout root.

`PSOXIDE_STRESS_OUTPUT` overrides the output directory (default `build/engine-stress`).
Set it on both commands. The runner copies the four frozen RON baselines in `fixtures/`
into new `<output>/<scenario>/project` directories, linking their assets to
`editor/projects/default/assets`. These are the original measured inputs, not regenerated
from a potentially changed default scene. Project creation refuses to overwrite a directory.

An existing output manifest selects replay of its retained disc. Replay refuses a changed
frontend or disc hash. Choose a fresh output directory for a different baseline.
Generated projects stay outside the editor's project picker. Older local tests were
moved to `editor/archive/local-tests/2026-10-06/`; replay resolves a missing original
disc path there and still verifies the recorded hashes. Their original output manifests
remain valid. No asset bytes,
built executables, discs or generated CSVs are committed here.

Builds run sequentially because they share the canonical MIPS stage. Do not run another
guest build at the same time. Each disc then runs in two independent emulator processes:
`PSOXIDE_EXPERIMENTAL_DMA_FIFO=0` (legacy) and `=1` (FIFO). Guest timing, not host elapsed
time, determines performance. The queue default matches the configuration measured in
October 6's 22 runs; coarser world settings apply only to the designated fixtures.

## Fixture matrix

| Name | Change from the large two-enemy room |
|---|---|
| e0 / e1 / e2 | Zero / one / two enemies |
| small2 | Original small room, two enemies |
| scale4 | Texture scale 400% on all 36 authored faces |
| patch4k | BSP patch extent 4,096 rather than 2,048; original texture scale |
| patch4k_scale4 | Both extent and texture-scale changes |
| outdoor | Same floor span, four 768-unit walls, no roof, 400% texture scale |
| outdoor_sealed | Same courtyard, five additional sky-aperture brushes close the exterior flood |
| vista8 / vista16 | Unsealed courtyard with 8 / 16 configured far-vista panels |

The far-vista projects reuse existing sky texture resource 21 as a repeatable panel
workload, not mountain artwork. The BSP cooker emits empty texture assets and flags=0:
see the [emitted record](results/2026-10-06/far-vista-emitted-record.txt). Both probes have
byte-identical tick-5,400 display captures to the disabled outdoor baseline in each DMA
mode. They reproduce an inactive setting, not a zero-cost working backdrop. The runtime
ring also follows the camera; it is not reachable terrain or world-space LOD.

## Analysis contract

Each run stops at 5,400 controller polls, allowing the final flip to advance up to 5,404.
`complete.json` is written only after a successful emulator exit. The analyser requires
a completed run, the poll target, a full final 1,200-row telemetry window and no room-chunk
loads in that window. Route and GPU counters are aligned by bus cycles. FPS comes from
actual display-start changes. Parent/child stage timers overlap and can include waits.

Software display screenshots are captured at route ticks 1,800, 3,600 and 5,400. Those
are not controller-poll indices. Camera-position counters stay at a sentinel in the BSP
path and are not position evidence. Incomplete attempts without a completion marker are
excluded. Each scenario records its authored project, audit, MCP receipts, build log,
source revision/diff, component pins, binary/disc hashes, launch arguments and run logs.

These stationary emulator measurements do not establish combat, traversal, streaming or
console performance. Coarser patches require a moving-camera check for interpolation,
clipping and cracks before changing production defaults.

## Area authoring budgets

See [area-budget.md](area-budget.md) for the controlled outdoor sweep, the `area_budget` MCP tool, and the distinction between the geometry ceiling and actor/camera runtime limits. Generated calibration projects stay under `build/area-calibration`, outside the editor picker.

**Correction:** the initial automatic 120-face / one-enemy profile is withdrawn.
[Valley normal Play recheck](valley-recheck.md) is the current interpretation.
The `area_budget` tool measures costs and optionally compares caller-provided budgets;
it does not infer a universal FPS ceiling.
