# Flood streaming

A level cut into many small cells (rooms) can stream by **budgeted portal
flood** instead of a ring of neighbours or a walk radius. The design is in
the unified-streaming lane notes; this page records what is built.

## What it does

From the observer's cell the flood walks the portal graph nearest portal
first, with an integer octagonal distance (`psx_grid::flood::octagonal_distance`:
largest axis delta plus 3/8 of the middle one plus 1/4 of the smallest,
within 10 percent of the straight distance). It stops at a radius or when the
next cell would take the resident set past a page budget, so the set is round
and deterministic.

Two radii, both in sectors, both in the flood block of the core:

* `near_radius`: cells within it of the **camera** are drawn, collide and run
  entities. A cell already drawn stays drawn one sector past it (hysteresis).
* `res_radius`: cells within it of the **player**, and within the page
  budget, are kept in memory ahead of need (the lead ring).

The camera's flood and the player's flood are cached until the observer
leaves its cell or drifts a sector, and run sliced
(`stream::FLOOD_POPS_PER_CALL` queue pops a call) with the last finished
result in force, so no tick pays for a whole search.

## Format

`LEVEL_BUDGET_FLOOD` (bit 6 of the level flags) puts a `FloodHeader` (8 bytes:
`pool_cap_pages`, `near_radius`, `res_radius`, `bias`) and one Q4 weight byte
per portal after the occupancy table, each part on a word boundary. A level
without the flag is byte for byte what it was (the reference cook pins prove
it). `pool_cap_pages` 0 means the whole level fits the pool: nothing floods,
every cell is resident and drawn as in an unbounded level.

## Settings and the cook gate

`StreamRadii.flood` (`FloodSettings`: `on`, `near_radius`, `res_radius`,
`quad_budget`) in the world document. The cook (`grid/cook/flood_plan.rs`)
runs the guest's own flood to check:

* the pool the busiest position needs (`plan_pool`, peak plus the largest
  cell) fits `GUEST_POOL_CEILING_KIB`;
* the quads in the plan cone at `near_radius`, worst over a spread of
  cameras, fit `quad_budget` (0 only reports).

## Tools

`grid_cells` (census, `sweep`, `rechunk_hex` preview) and `grid_flood` (one
camera) in `psxed-mcp`; the same code is in `psxed_project::grid::census`.
`rechunk-hex <project.ron> <size>` re-cuts a project in place.
