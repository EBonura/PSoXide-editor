# Graybox Valley normal Play recheck — 6 October 2026

**The default 120-PVS-face / 240-triangle / one-enemy restriction is withdrawn.**
It was an invalid generalization from particular camera views and instrumented
fixtures. Valley demonstrates why the static candidate count cannot be a universal
frame-rate ceiling. The broader Valley remains the outdoor reference; the constrained
Highlands layout is a diagnostic experiment, not evidence of the engine's maximum scope.

The user's observation was normal in-game editor Play, confirmed by the editor build
log: `--features "cd-stream-bench"`. Prior stress tests added `emulator-telemetry`,
which emits guest-side MMIO events and can affect timing. That difference needed
checking; it turned out to be a **small effect in this specific A/B**, not the main
explanation for the apparent contradiction.

## Controlled comparison

Two freshly built copies of the same Valley project, same release frontend, identical
3,600-poll input tape: 600 neutral, 2,400 forward, 600 neutral. Both retained the same
geometry, player, two Custodians and assets. Builds used a separate guest stage and
project directories under `build/valley-budget-recheck`. The live Valley project and
its geometry were not changed. Normal = the editor's default guest flags; instrumented
= the same flags plus `emulator-telemetry`. Both DMA models were measured explicitly.
FPS comes from host-observed display-start changes, not guest telemetry or host wall time.

| DMA model | Build | Initial view (300–580) | Traversal (650–2900) | Final view (3100–3550) |
|---|---|---:|---:|---:|
| Default | Normal Play | 25.198 | 27.653 | 29.645 |
| Default | Instrumented | 25.166 | 27.612 | 29.645 |
| Experimental FIFO | Normal Play | 24.140 | 26.638 | 29.645 |
| Experimental FIFO | Instrumented | 23.673 | 26.427 | 29.645 |

Nominal 30 fps at this emulated video cadence is 29.645 fps. The final view's p95
interval is 33.733 ms in every variant. The route as a whole is not locked 30, and the
initial view is heavier; the user's report of fairly smooth 30 in Valley is compatible
with the measured faster views. The numbers do not show a telemetry slowdown large
enough to explain the earlier restrictive interpretation.

The MCP reports 418 candidate faces at the arrival probe and **421 at the north terrace**,
with 421 total world faces. These candidate sets include geometry rejected by subsequent
frustum/backface tests. They are not actual faces or triangles submitted by one frame.
Camera direction, screen coverage, clipping, sky and model work change frame time even
when the PVS count stays essentially unchanged. This is the central error in the old cap.

## Corrected tool behavior

`area_budget` schema version 2 has **no default face, triangle or enemy limit**. It reports
exact cooked PVS costs, sealing, named camera samples and unknown runtime status. An
optional `budget` argument compares a project's explicitly chosen authoring constraints:

```json
{
  "areas": [{"name":"North terrace","samples":[[0,1536,32256]],"enemies":2}],
  "budget": {"pvs_faces":500,"base_world_triangles":1000}
}
```

Those example numbers are caller-provided constraints, not recommended FPS limits.
Without `budget`, limits and remaining margins are null and status is
`runtime_measurement_required`. With it, statuses distinguish authored-budget comparisons
from runtime verification. Full audits no longer declare a scene over a 30 fps budget
from static PVS counts. The old one-enemy prohibition is also removed.

For performance acceptance, measure the normal guest with host route/display logs.
Use an instrumented copy to diagnose stage costs and check its perturbation separately.
The analyser now accepts an absent/empty telemetry profile; missing overflow/actor
counters remain **unknown**, rather than being treated as zero. No new automatic FPS
ceiling is claimed, and no further geometry was removed to fit the withdrawn one.

## Evidence

`results/2026-10-06/valley-recheck/` retains the numeric results, matching project hashes,
variant guest/disc hashes, input tape and the MCP sample report. Raw logs, copied editor
build log and screenshots are under `build/valley-budget-recheck`. The older baked disc
was itself instrumented, so it was not mistaken for the live Play build in the final A/B.
The compared guests were both rebuilt from the same project copy. These are emulator
measurements; this run makes no new hardware-performance claim.
