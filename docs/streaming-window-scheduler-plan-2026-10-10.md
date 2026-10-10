# M9 plan: the guest window scheduler (2026-10-10)

Status: not started. Written down so the next session can begin from it.

## Why this is the real remaining work

The guest streamer installs every region once, in disc order, into one slot per
region (`region_stream.rs`: "There is no eviction yet"; `world_stream.rs` loads
with `load_streamed_exact` and a slot count equal to the region count). The host
cook and the sweep prove a 16x16 world fits the drive and the pool when only
the player's window is resident, but nothing in the guest keeps only a window
resident. A 16x16 seed holds about 5 MB of payload against a 369,383 B pool, so
it cannot boot until the guest has a scheduler. Booting it on the emulator is
the proof of the whole design, and it needs this scheduler first.

## What already exists

* Map level: `begin_install`, `install_feed`, `link_region`, `uninstall_region`
  (unlinks the children back to the stub and frees the slot), `free_slot`,
  `slot_of`, `unresident_region_at`, and the `pvs_missing` counter (a rank
  dropped because the region it names is not resident, which is exactly
  "visible but not resident", i.e. pop-in).
* Streamer level: `RegionStreamer` reads one region at a time through
  `RegionRead` with retries and read restarts, installs sector by sector, waits
  for textures, links. It is hard-wired to "all regions, disc order" and to at
  most `MAX_STREAM_REGIONS` (64) regions.
* Index: `RegionEntry` carries the region box (`mins`/`maxs`, i16 engine
  units), `sector_start`, `payload_bytes`, and `V(R)` (`vis_list`, which starts
  with the region itself).
* Cook gate: `Need(p)` is the union of `V(q)` over regions `q` whose box is
  within `viewer_radius` of `p`; the lead ring adds `Need` of regions reachable
  within `h_lead` walking distance over the aperture graph; the home pin holds
  the start region's `Need`. See `brush_region/gate.rs`.

## Design

1. A pure policy type in `psx-game-runtime` (host-testable, no hardware):
   `WindowScheduler` over the parsed `StreamingIndex`. Input per update: the
   player position (engine units) and the resident set. Output: regions to
   request, in priority order, and victims to evict.
   * `need(p)`: `V(q)` for every region box within `VIEWER_RADIUS` of `p`.
     Hard requirement: every member must be resident.
   * `lead(p)`: regions whose box is within `H_LEAD + hysteresis` of `p`,
     nearest first. Euclidean distance is a superset of the cook's walking
     distance, so the lead set can be larger than the cook assumed. Lead
     requests only take a free slot, or a slot held by a region outside
     `need` and outside the lead set; they never displace anything nearer.
   * Eviction: only when a request has no free slot. Victim order: not in
     `need`, then outside the lead set, then farthest by box distance, then
     least recently used. Never a region with an install in flight. Eviction
     happens at a frame boundary (`uninstall_region`, then the slot is free).
   * `VIEWER_RADIUS` and `H_LEAD` come from one shared constant per value, used
     by both `PartitionParams::default` and the scheduler, so the guest cannot
     drift from the gate. Do not add constants to the cooked manifest of a
     whole-map project (legacy cooks must stay byte-identical).
2. Generalise `RegionStreamer` from "walk the disc order" to a bounded queue of
   requested regions. Size its per-region arrays from the region count (heap,
   once at `begin`) instead of the 64-region fixed array. Keep one region in
   flight; keep the sector budget per pump. The loading screen asks for
   `need(spawn)` plus the lead ring and waits for `need` only.
3. Glue in `editor-playtest`: call the scheduler from `BspRuntime::update_motor`
   (it already calls `probe_stub(position)` every motor update), at most every
   few ticks or after the player moved a set distance. Publish scheduler
   counters through `PSX_WORLD_STREAM` (resident regions, evictions, requests,
   need misses, `pvs_missing`, slot high-water) and the player position through
   a second `#[no_mangle]` symbol so a route can be driven closed-loop from the
   emulator's `--route-watch-u32`.
4. Textures: v1 keeps every material resident once a linked region uses it
   (`required_materials` only grows). The stress world uses few materials. Per
   region texture release is M9b.

## Known limits to state in the docs

* Hook landing closures are not in the index, so v1 ignores them. An
  unresident region is solid (invariant I3), so this is safe but a hook toward
  a region that is not resident hits a wall.
* The lead set is Euclidean, so it can ask for regions behind a wall that the
  cook's graph lead would not. Slots they take are released first.
* Slots are fixed size (`SlotCaps`, the per-lump maxima over all regions). The
  cook gate sums exact resident bytes. Measure `slot_bytes` of a real 16x16
  container first: if `pool_available / slot_bytes` is below the worst
  `need + lead` region count, tighten the caps in the cook or move to a packed
  page pool before anything else.

## Proof to run (in this order)

1. Host tests for the policy: need is always resident after an update given
   enough slots; no victim is in `need`; backtrack at a boundary does not
   thrash; fast-forward requests nearest first.
2. Cook a 16x16 seed through the production path (seed 7 and seed 27 are the
   ones the sliver rule fixed), build the disc with
   `PSXED_STREAM_WORLD=stream` and features `cd-stream-bench world-stream`,
   and read `slot_bytes`, container bytes and region count from the report.
3. Boot headless and walk a route that crosses many regions. Drive the route
   with `--press` stick tokens, steering offline from the watched position
   (the emulator is deterministic, so re-run from boot per correction).
   Gates: `pvs_missing` 0 and stub hits 0 along the route, 0 retries and 0
   dropped sectors, display flips advancing, no frame over the 30 fps budget
   attributable to a load (route log bus-cycle deltas against the install
   pumps), heap high-water inside the pool.
4. Legacy identity: the four projects still cook byte-identically to editor
   main without streaming, and the Graybox Reach whole-map frames are
   unchanged.

## Single owner of the skeleton (state of the work)

Branch `feat/stream-skeleton-single-owner-2026-10-10` (host-tested only):
the container becomes a UI.PAK chunk read into the idle font/sky staging
scratch at gameplay entry and loaded with `load_streamed_exact_from_slice`;
the image keeps no copy of the streaming index lump, there is no temporary
index buffer, and `PXBSP_WORLD` is empty for a streamed world. The pool plan
and gate charge the skeleton once there. Still to do before it can land:
build the guest, measure link map headroom and heap high-water against the
current streamed build, and show lockstep frame identity against the whole-map
reference (the harness is `variant.sh` plus `shots3.sh` from the M7 notes).
`RAM_BUDGET.stream_overhead_bytes` must be re-derived from the new link map.
The skeleton is currently held up to four times in the guest: the baked
container, the image copy, the image's copy of the index lump, and a leaked
temporary index buffer on the bump heap; the parsed index is the one copy that
has to stay.
