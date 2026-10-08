# Cortex Ignition world streaming: design

Status: design proposal, read-only research (2026-10-08). Nothing was built, run or edited.
Inputs: `streaming-survey.md` (same directory, cited as "survey"), `docs/p6-pxbsp-streaming-followup.md`, hk-psx `main`, SDK `main` (EBonura/PSoXide, `git show main:`), PSoXide-editor `main` 6b1219d5.

Labels used for every number:
- [M] measured or stated in code/docs (source cited)
- [D] derived by arithmetic from [M] values (shown)
- [E] estimate or tunable default I am proposing, to be replaced by a measurement
- [?] unknown, needs measuring at head

Requirements taken as decided (not reopened): seamless world, stream everything, music only in combat via a combat radius, cooker splits automatically, shared code in SDK/engine, integer math, no visual trades without approval, PIO/seek-first transport only. Two later decisions from Manny are folded in: the whole grid world is removed (section 10), and hk-psx's streaming code moves into the shared SDK/engine (section 11 and the milestones).

Repo map used throughout:
- SDK = `EBonura/PSoXide` (`sdk/crates/*`, `crates/psx-iso`). The editor repo materialises `sdk/` from it at the revision pinned in `components.lock.json`, so an SDK change lands in PSoXide first and then bumps that pin in PSoXide-editor.
- ENGINE/EDITOR = `PSoXide-editor` (`engine/crates/*`, `editor/crates/psxed-project`, `editor/crates/psxed-mcp`).
- HK = `hk-psx` (pins the SDK via `sdk.lock.json`), CS = `cs-psx`.

---

## 0. The three ideas the whole design hangs on

1. **A region is a convex cell cut by the cooker, and it is a subtree of the BSP.** The cooker forces a few top-level cut planes, builds an independent BSP under each cell, and the top of the tree stays resident. Streaming a region is then "install a subtree and patch one child pointer in the parent". The tuned render loop does not learn about streaming, it just sees a node whose child changed between frames.
2. **Residency is a function of position, decided by a rule the cooker can check.** Required set = potentially visible closure of where the camera/player can be, plus authored teleport edges (hooks), plus the combat scope, plus the home pin. Everything else is prefetch, ordered by walk distance. The cooker computes, for the real level, the worst bytes-per-unit-of-travel that enter the required set and refuses to cook (or warns loudly) when that exceeds what the drive can deliver at the player's top speed. That is the "nothing missing" guarantee, as a proof about the data rather than a hope about the runtime.
3. **An unresident cell is solid to collision and never reached by the camera.** The fail-safe is physical (an invisible wall) rather than visual (pop-in). It should never engage, a counter proves it, and any gate with counter != 0 fails.

---

## 1. Architecture overview

```
 DISC (ISO9660, one WORLD.PAK-style pack + CD-DA tracks)
   region payloads in layout order | shared texture packs | archetype packs | SFX packs | skeleton | CD-DA combat track
        |
        v
 +---------------------------------------------------------------------------------------------+
 | TRANSPORT  psx-cdstream (SDK)   IRQ driven, PIO, SetLoc/SeekL/SetMode/ReadN/Pause bracket    |
 |   1 sector per CD IRQ into a caller sink, abort at next sector, contiguous requests chained  |
 |   without re-seek, counters, host-testable state machine behind a CdHw trait                 |
 +----------------------------------+----------------------------------------------------------+
                                    | owner token: Data | Audio(CD-DA/XA) | Boot(blocking)
 +----------------------------------v----------------------------------------------------------+
 | DRIVE ARBITER (same crate)   lease_to_audio() / reclaim()   <---->  engine CddaPlayer        |
 +----------------------------------+----------------------------------------------------------+
                                    | Request{lba,count,sink,class,deadline,id}
 +----------------------------------v----------------------------------------------------------+
 | SCHEDULER  psx-residency (SDK, no_std, host tested)                                          |
 |   classes: P0 Demand | P1 CombatFill | P2 Lead (by walk distance) | P3 Background | P4 Audio  |
 |   coalesce / adopt in-flight, EDF inside a class, retry+backoff, per-frame CPU budget        |
 +----------------+------------------------+------------------------+--------------------------+
                  |                        |                        |
 +----------------v------+  +--------------v---------+  +-----------v----------------------+
 | REGION RESIDENCY      |  | TEXTURE / VRAM         |  | ARCHETYPE + AUDIO                |
 | slots + PagePool      |  | VramRuntime slots,     |  | model+clips+atlas packs,         |
 | Req(P) closure,       |  | refcount per region,   |  | SPU RAM map for SFX banks,       |
 | lead ring H(v_max),   |  | upload queue behind    |  | SPU ring (psx-spu::stream)       |
 | pins: combat scope F, |  | the GPU access guard   |  |                                  |
 | home, hook edges,     |  |                        |  |                                  |
 | evict: dist + LRU     |  |                        |  |                                  |
 +----------------+------+  +--------------+---------+  +-----------+----------------------+
                  |  install pipeline: land -> verify (FNV, sliced) -> relocate indices (sliced)
                  |                    -> VRAM upload -> link (patch parent child) -> ADMIT
                  v
 +---------------------------------------------------------------------------------------------+
 | CONSUMERS (engine, PSoXide-editor)                                                           |
 |  BSP render: slotted map, top tree + subtrees, canonical-rank PVS rows     (psx-bsp)         |
 |  Collision: hull0 = render tree, hull1/2 = per-region hull BSPs, UNRESIDENT stub = solid     |
 |  Actors: spawn records per region, archetype residency, dormant state table                  |
 |  Combat radius: scope F, music gate, CD-DA lease                                             |
 +---------------------------------------------------------------------------------------------+

 COOKER (host, psxed-project)
   brushes -> CSG surfaces -> region cut planes (top tree) -> per-cell surface BSP -> portals
   -> PVS (leaf level, bounded by far reject) -> region closure V(R) -> payloads
   -> per-region hull BSPs (hull1, hull2) -> disc layout (spectral order, optional duplication)
   -> pack + StreamingIndex lump + COOK REPORT with feasibility verdict
```

Budgets are per category (RAM pool for region pages, VRAM texture slots, SPU RAM, CPU slice per frame, drive time), each with its own pool and its own eviction, so one category cannot starve another. Table in section 4.5.

---

## 2. Transport: one shared CD component

### 2.1 What exists today (survey rows 1, 8, 10, 12, 13, 15)

| Implementation | Mode | Start-up | Abortable | Console proof | Verdict |
|---|---|---|---|---|---|
| engine `psx-game-runtime/src/cd_stream/hw.rs` | polled, DMA ch3, no `dma::abort` | bare SetLoc+ReadN | by pause at poll boundary | DMA is a lottery on Manny's console [M] (survey 3.3) | retire |
| SDK `psx-io/src/cd/reader.rs` (`SectorReader`) (+ `psx-pack/src/cd.rs` on the older tree) | blocking or `try_read_sector`, PIO | SetMode, SetLoc, SeekL bracket (`start_read_seek_first`) | `stop()` | PIO and seek-first byte-perfect on silicon [M]; loads 1.4 MB EXEs, hl-psx 96 maps | keep as the boot/blocking path |
| hk-psx `game/src/cd_stream.rs` (364 lines) | IRQ driven, PIO, 1 sector per IRQ | SetLoc, SeekL, SetMode, ReadN, then Pause | yes: flag, handler pauses at next sector | shipped build 107; docs make no console timing claim [M] | **start here** |
| cs-psx `game/src/cd_irq.rs` | port of hk's | same, plus "CD source enabled in I_MASK only while a transfer runs" | yes | shipped | fold into the shared one as an option |
| quake `ChunkStream`, oot/alttp/wipeout loaders | blocking | SDK reader or DMA | no | shipped | leave on `SectorReader` |

### 2.2 Which one to start from, and why

Start from **hk-psx `cd_stream.rs`**. It is the only implementation that is simultaneously IRQ driven (the emulator loses sectors when a poll runs once per 16.7 ms frame against a 6.5 ms sector period, 36 lost on the Cortex pack before `wait_for_sectors` was forced [M], survey section 2 "Gaps"), PIO only, seek-first, and abortable at sector granularity. Its comments record real findings (a Pause issued from foreground while sectors were arriving lost the acknowledge, traced: no interrupt for 600 VBlanks, so cancel is always "flag it and let the handler pause at the next sector"). Its weaknesses are fixable in place:

- it patches the exception vector itself (`install()` writes `j hk_cd_exception_wrapper` to 0x80000080) and chains to `__psx_rt_exception_handler`; psx-rt documents that its handler owns VBlank only and other sources must be serviced ahead of it, and `psx_rt::interrupts::declare_stack_safe_handler` exists for exactly this case. The shared crate should own this install and declare the handler stack-safe, instead of every game doing a raw vector write.
- one transfer at a time, caller re-arms after `Done`, no queue.
- game-specific globals (`HK_CD_*`, `super::HK_CD_SECTORS_READ`) and a fixed 600-VBlank timeout.
- the state machine is welded to register access, so it has no host tests.

### 2.3 Shape of the shared component

New SDK crate **`psx-cdstream`** (depends on `psx-io`, `psx-rt`; the pure state machine builds on host).

- `trait CdHw` (register access + `vblank_count`), implemented by the real controller and by a scripted fake. All phase logic (`Setloc, SeekAck, SeekComplete, Setmode, ReadAck, Reading, PauseAck, PauseComplete, Done, Failed`) is exercised on host against scripted INT sequences including INT5, late IRQs, cancel in each phase and the stale-response cases the hk comments describe. This is the test gap the survey flagged for `cd_stream.rs`/`hw.rs` (zero tests).
- `Request { lba, sectors, sink, class, id }` where `sink` is a destination description (contiguous pointer, or a small scatter list). A page-pool run is one contiguous pointer. The SPU music FIFO uses two requests when it wraps.
- **Contiguous chaining.** If the next queued request starts at `lba + sectors` of the one that is finishing, the handler swaps the destination pointer inside the IRQ and keeps reading. No Pause, no re-seek. This is what turns a layout-ordered group of regions into one seek.
- **Abort.** `cancel(id)` sets the flag; the handler pauses at the next sector. Partial progress is reported (`received`), and the scheduler may resume with a new request at `lba + received`, so an abort wastes one seek, not the whole run.
- **Priority queue lives one level up** (scheduler), the transport has a depth of one in flight plus one pending for chaining. Keeping policy out of the interrupt path.
- Diagnostics: the existing counters (IRQ count, sectors, discarded sectors, max IRQ duration via Timer2, phase, error) become fields of a `StreamStats` struct. Symbols exported with `PSX_CD_*` names; hk keeps its `HK_CD_*` names with linker `PROVIDE` aliases so its tools (`profile_tape.py`, gate scripts) read the same symbols before and after migration.
- Optional `IrqMaskPolicy::OnlyWhileBusy` (the cs-psx variant).

Coexistence with `SectorReader`: `SectorReader::prepare` sets `I_MASK` to VBlank-only until `stop`. The shared crate defines the ownership token (`Boot` vs `Data`): boot-time blocking loads use `SectorReader`, then `psx_cdstream::install()` takes over. hk already does exactly this ("Install after the SDK's title-time polled header read has completed").

### 2.4 CPU cost of PIO, and what it means

PIO costs about 1.3 ms of CPU per sector more than a working DMA (SDK comment, survey 3.3) [M, one source]. At the full 2x rate (6.49 ms/sector [M]) that is 1.3/6.49 = 20% of CPU while a read is running [D]. The tax is per sector and cannot be throttled while ReadN is active (a late drain loses the sector). So the control knob is the **duty cycle**: read in bursts only when the scheduler has real work, and keep the average utilisation low (section 4.3 designs for U <= 0.5, so an average tax of about 10% with 20% peaks [D]). This matters because Cortex's measured frame budget is already near the 30 fps edge (29.62 fps at 136 faces, survey section 2) [M]. Two levers to measure in M0, not assume: the existing `HK_CD_IRQ_MAX_TIMER2_TICKS` counter gives the handler duration directly, and a 1x-speed mode halves the tax at half the bandwidth (speed-change cost on console is unmeasured, survey 3.6) [?].

### 2.5 CD-DA suspend/resume handoff (the arbiter)

The working assumption, now backed by one on-hardware observation in the engine, is that a data read kills CD-DA: the `update_ui_music` comment says starting music while the menu streams "makes the read hang and the music die on real hardware (one laser cannot read a UI image and play a CD-DA track at once)" [M, `engine/crates/psx-engine/src/game_app.rs` near line 2604]. The silicon probes (survey 3.5) only timed the read, not whether the audio survived, so M0 adds a probe that records it.

Arbiter API (in `psx-cdstream`):
- `owner() -> Data | Audio | Boot`
- `request_audio_lease() -> Pending | Granted`: stops accepting new reads, aborts the in-flight one at the next sector, waits for `Done` (including the Pause acknowledge), masks the CD source at the CPU (`suspend()` in hk's code), and grants. Only then does the engine's `CddaPlayer` issue its polled commands.
- `release_audio_lease()`: engine has Paused CD-DA (never Stop: the motor spin-down reports status 0x00 for 1 to 2 s and reads right after Stop failed on console [M], survey 3.7), arbiter does `resume()` (discard response, ack, unmask), and the first data read after audio pays a normal seek from the audio position.
- `CddaPlayer::release_for_data_reads` (game_app.rs:481, :909) becomes a client of the arbiter instead of a policy flag in `GameApp`.

The streamer-side protocol (what is allowed to happen while Audio owns the drive) is section 6.

---

## 3. Cooker partitioning

### 3.1 Facts about the current cook that shape this

- Pipeline (`editor/crates/psxed-project/src/`): `brush_world.rs::compile_brush_world` -> `brush_compile.rs` (CSG surfaces, `build_surface_bsp`, `choose_splitter`) -> `brush_portal.rs::portalize_surface_bsp` -> `brush_vis.rs` (Quake portal flow, Release; connected-component for Draft) -> `brush_pack.rs::pack_bsp_geometry_with_visibility` -> `brush_collision_hulls.rs` -> `brush_pxbsp.rs::build_pxbsp_with_submodels`.
- Hard format limits in the wire records: faces and vertices `u16` (`validate_limits` in `brush_pack.rs`), leaves, nodes, clipnodes `i16`, visibility lump `u16::MAX` bytes, PVS row at most `PXBSP_MAX_VISIBILITY_BYTES` = 1024 bytes = 8192 leaf bits (`pxbsp.rs`). **These are world-size limits today, independent of RAM.** Region-local index spaces remove them.
- Collision: hull 0 is the classified render BSP itself (comment in `brush_world.rs::compile_runtime_collision_hulls`), hulls 1 and 2 are box-expanded body hulls produced by `compile_collision_hulls`, which builds axial spatial splits down to `SPATIAL_LEAF_BRUSHES = 48` brushes and then a **sequential per-brush plane chain** per leaf (`build_brush_chain`). A brush that straddles a split is duplicated in both leaves. A 16x16 terrain is 512 closed convex brushes (docs/editor-terrain.md), so a trace walks long chains. This is the finding behind "70 to 80% of the frame on terrain".
- Far reject: the classic path rejects true depth beyond `4 * 2048 / S` = 2731 units at view scale S = 3 (`PXBSP_VIEW_SCALE_Q12` doc in `psx-bsp/src/render.rs`) [M]. **Nothing farther than about 2731 units (plus the 125-unit camera offset, authored 2000 / 16 [D]) from the camera is ever drawn.** The visible neighbourhood of any position is therefore bounded independent of world size. Call D_vis = 2860 units [D].
- Units: BSP projects cook at authored/16 ("engine units", Quake scale; `units.rs::WORLD_UNIT_DIVISOR`). All distances below are engine units.

### 3.2 Choosing the cuts (automatic)

Goal: regions that are subtrees of one BSP, balanced in payload bytes, cut at chokepoints, each within hard caps.

1. Run the existing front half (CSG, surface BSP, portals) once to get a baseline leaf/portal graph, polygon density and brush density. That is only an input to scoring, not the final tree.
2. **Top-down recursive bisection** of the world bounds. Candidate cut planes: (a) planes of existing large solid faces (walls and floors, these already exist as BSP splitters), (b) axial planes on a coarse lattice. Score each candidate with, in order: portal area crossing the cut (smaller is better, a door is nearly free, an open field is not), count of polygons split, balance of estimated payload bytes, and growth of the visibility closure. Recurse until a cell fits all caps (section 3.3). Deterministic tie-breaks, no randomness.
3. Insert the chosen planes as forced top splitters, then run the normal `build_surface_bsp` independently inside each cell. A region is therefore exactly one subtree and one convex cell. Polygons spanning a cut are split by the ordinary BSP splitter, no new geometry code.
4. Recompute portals and PVS on the final tree (leaf level, section 3.4), then a **second pass** evaluates actual closure sizes and payload fill, and merges adjacent under-filled cells or splits cells whose closure is too large. Bounded iterations, report the result.
5. **Degenerate case is exactly today's output.** A world whose total payload fits one region cooks with zero cuts and must produce the same PXBSP bytes as now (empty StreamingIndex). That is the back-compat gate for Graybox Reach and every existing project.

Hard caps per region (cooker parameters, defaults [E] until M0/M8 measure):
- payload target 16 sectors (32 KB), hard cap 32 sectors (64 KB). Basis: 16 sectors is 104 ms of transfer [D: 16 x 6.49] against a 137 ms 128-sector seek [M], so below that the seek dominates; the retired grid code used a 32 KiB chunk cap (`MAX_STREAMED_ROOM_CHUNK_BYTES`); the p6 doc's 1 to 4 sectors is too small for this drive.
- slot caps for faces, vertices, nodes, leaves, marksurfaces, clipnodes (section 3.5) with a target fill >= 70% of the binding cap [E]; the report prints fill so waste is visible.

### 3.3 Per-region payload

One pack chunk per region, sector aligned, FNV-1a checked (existing `PSOXWPAK` v1 entry, `psx-pack`/`psx-iso`; chunk id = `REGION_BASE + region_id`):

| Part | Contents | Index space |
|---|---|---|
| header | region id, counts, caps used, bounds, offsets, fixup table offsets, texture dependency list, archetype list | n/a |
| render | nodes (the subtree), leaves, marksurfaces, faces, vertices (+ baked light), local plane table | region-local, relocated at install |
| PVS rows | one row per local visible leaf, in the **canonical rank layout** (3.4) | rank-based, not slot-based |
| collision | hull1 and hull2 clipnodes + planes, built as real hull BSPs (3.6); hull 0 reuses the render subtree | region-local |
| materials | list of global material ids used; small textures (<= 4 sectors [E]) inline | n/a |
| spawns | entity records located in this region (enemy, props, pickups), SFX refs | n/a |
| link patches | list of (parent node, side) in the top tree that point at this subtree, so install/evict can patch them | n/a |

**Duplicating shared data versus dependencies.** Disc space is nearly free (a pack of tens of MB against a 650 MB disc), only read time matters. A seek is 79 to 310 ms [M], one extra sector is 6.49 ms. So:
- small shared textures (<= 4 sectors [E]) are duplicated into every region that uses them; the VRAM manager dedupes by texture id so a duplicate costs only read time, never VRAM.
- larger shared textures live in shared texture packs with their own residency and refcounts (this is the p6 doc's "shared textures are separate" rule), placed on disc beside the region group that uses them most.
- the cooker picks the threshold per texture with the cost model `(copies - 1) x sectors x 6.49 ms` against `expected extra seeks x ~137 ms`. The threshold is a cook parameter, tuned in M8.
- Stronger lever, optional later: write a region more than once at different layout positions (one per dominant approach direction) when the layout cost model says it saves seeks. Deterministic, report shows it.

**What stays global and resident (the "skeleton", baked into the StreamingIndex lump, small):**
- region directory: LBA, sector count, bounds, flags, FNV, slot cap class.
- region graph with portal apertures and portal-to-portal distances (cut-plane open areas), used for walk distance.
- visibility lists V(R) as sorted region id lists.
- top render tree and the top of each hull tree, whose leaves are the **stub leaves** (one per region) carrying the region id.
- global material table, texture directory (id -> pack, size), archetype directory.
- global entities: player start, checkpoints, cross-region triggers, hook points (cap 32 [M], docs/cortex/hook-points.md), spawn state table.
- Estimated size: tens of bytes per region [E] (about 50 to 100 B directory + graph + V list), so a 400-region world is on the order of 20 to 40 KB [E]. A 2000-region world would need a second paging level for the skeleton itself, out of scope here and stated as a limit.

### 3.4 Visibility across regions (the hard part of the format)

Leaf PVS across regions needs rows whose bit positions do not depend on which slot a neighbour happens to occupy.

- Visibility is computed at leaf level on the final tree (portal flow already exists), but **clipped by far reject**: leaves farther than D_vis from a leaf cannot be seen, so they are dropped from that leaf's row. This also keeps the cook tractable on a large world (the portal-flow cost is the cook-time risk, section 9).
- Region closure V(R) = set of regions containing any leaf visible from any leaf of R. A region's rows have width `|V(R)| x Lcap` bits (Lcap = per-slot leaf cap), laid out by **rank**: bit `rank * Lcap + local_leaf`, with rank = index of the region in R's sorted V list (R itself is rank 0). Row width must stay <= 1024 bytes, which bounds `|V(R)| <= 8192 / Lcap` (for Lcap = 256 that is 32 regions [D]). The cooker enforces this and reports offenders; it is also the cap on how visible a cell can be.
- At runtime, when the camera is in region R, the renderer builds a rank -> slot-base table once (at most 32 entries) and expands row bits to virtual leaf ids as `slot_leaf_base[rank] + local`. This is a new, additive PVS decode path in `psx-bsp/src/render.rs`, selected at map-load by a flag. The legacy path (quake, hl via PSB) is untouched byte for byte.
- Because V(R) must be resident while the camera is in R, **V(R) is also exactly the residency requirement**, so the rows and the residency rule cannot disagree.

### 3.5 Resident layout: slotted arrays, relocate at install

To avoid indirection in the tuned loops (packed 2-bit face state table, aligned halfword face decode), the resident map is the same set of lump arrays as today, but each lump is partitioned into S fixed-capacity slots: `faces[slot * Fcap ..]`, `vertices[slot * Vcap ..]`, `nodes`, `leaves`, `marksurfaces`, `clipnodes`. Region payloads carry region-local indices; install adds the slot bases in a sliced fixup pass (the same idea as hk's code relocation, `J26/HI16/LO16/W32` fixups applied per frame slice). Face ids stay below 65535 if `S x Fcap <= 65535`. The top tree (nodes `1..2R`, stub leaves `1..R`) sits ahead of the slots.

Linking: a parent's child halfword is either the stub leaf (`-(stub+1)`) or the subtree root node index in a slot. Install patches it after admission, evict patches it back. One aligned i16 store between frames, no renderer change.

Slot pool sizing: a uniform slot wastes (cap - actual) bytes. The sector page pool (section 10, salvaged from `StreamedRoomPages`) backs the raw landing buffers, so landing memory is exact to the sector, while the **render-lump slot arrays are the only fixed-capacity part** and hold only the relocated working copy. Whether to land compressed-then-expand or land raw and relocate in place (copy avoided, as hk does) is an M6 measurement.

### 3.6 Collision: real hull BSPs per region

Replace the brush-chain tails with a proper solid-leaf BSP, per region and per body hull. Output format is unchanged (`ClipNode` 6 bytes, 14-byte plane records, i16 indices local to the region), so the runtime tracer (`psx-bsp/src/collision.rs`) is untouched apart from the stub contents code.

Algorithm (cooker, f64 like the current code, `brush_collision_hulls.rs`):
- Input per hull: the convex expanded polytopes already produced by `expanded_hull_planes` (face planes, axial planes, edge-times-axis bevel planes, so the Minkowski sum with the body box is exact), clipped to the region cell, each with its contents precedence.
- Recursive build: pick a splitter from the live fragments' supporting planes; clip every fragment by it (fragments straddling the plane are split, not duplicated whole); recurse front and back. A cell is SOLID/contents-X when a fragment's remaining sides are all already used on the path (it equals the cell), EMPTY when no fragments remain.
- Splitter order of preference: axial planes on a lattice (this makes a heightfield collapse to O(log cells) depth), then large face planes, then bevel planes last. Score = fragment splits (heavily weighted), then depth imbalance, then plane count. Depth is capped; hitting the cap falls back to a short chain as today.
- Expected effect: depth is logarithmic in brush count instead of linear in the chain length, and a duplicated brush becomes a clipped fragment. **I expect a large trace-cost drop on terrain but this is a hypothesis.** Gate (M5): differential test against the current compiler (same contents and trace fraction within one `TRACE_PLANE_EPSILON_Q12` over a large sample of random traces on Graybox Reach, graybox-valley and a terrain fixture), plus a guest benchmark of hull nodes visited per trace and cycles per trace on the terrain fixture, before and after. The target ratio is set from the measured baseline, not asserted here.
- Region boundaries: the top of every hull tree uses the same cut planes as the render tree. Brushes (expanded) crossing a cut are clipped into both cells, so a point inside a cell never needs the neighbour. A trace that leaves the cell continues in the neighbour's tree.
- **Stub rule:** the top-tree child that stands for an unresident region is a leaf with a new contents code `CONTENTS_UNRESIDENT = -7`. The tracer treats it as SOLID, records `unresident_hit` plus the stub's parent so the streamer can raise the request to P0, and bumps a counter. Hull 0 (render nodes) and hulls 1 and 2 share this behaviour. An old loader that meets -7 fails validation (`BadLeaf`/`BadClipNode`), which is the correct fail-closed behaviour for a streamed file on a runtime that cannot stream.
- Actor reach: an enemy's patrol/leash ball adds its collision requirement (section 4.1), so an actor can never be standing on an unresident stub.

Impact on versioning: PXBSP container stays v6. The reserved `StreamingIndex` lump (kind 15) gets its own magic and version and is non-empty only for streamed worlds; `from_static`/`load` of a non-empty index returns `StreamedWorldUnsupported` on runtimes without the feature. Quake/HL golden-map tests and the guest code-size/cycle gates for quake-psx must pass unchanged (gate in M6).

### 3.7 Disc layout (seek locality)

- Objective: minimise `sum over region-graph edges (traffic weight x |LBA(a) - LBA(b)|)`, weight from portal aperture and the main-route likelihood. Linear arrangement is NP-hard, so: spectral ordering (Fiedler vector of the weighted graph Laplacian), then local 2-opt refinement, deterministic, cost reported.
- Group the V(R)/neighbour set that is usually requested together into contiguous groups so the scheduler can issue one chained request (one seek) for a batch.
- Seek model for the cost function uses the measured table 11 / 79 / 137 / 310 ms for 1 / 16 / 128 / 512 sectors [M]. Because that table is not smooth and swings about 2x run to run [M] (survey 3.2), the layout objective optimises sector distance with a distance-class weighting rather than trusting milliseconds, and the report shows the share of region transitions that fall in the <= 16, <= 128, <= 512, > 512 sector classes.
- Shared texture packs, archetype packs and SFX packs sit in the same layout, adjacent to their heaviest user.
- The combat track stays outside the pack as CD-DA (red book tracks), unaffected by layout.

---

## 4. Residency and prefetch

### 4.1 The rule

Definitions (all distances are walk distances over the region graph, lower bounded by Euclidean distance):
- `Viewer ball` = ball(player, r_cam) with r_cam about 125 + clearance [D from camera distance 2000 authored], so the camera region is included.
- `Req(Q)` = the regions in V(Q), their collision (hull subtrees), the textures/materials those regions use, the archetypes of spawns in those regions, and the SFX banks they reference.
- **Hook edges:** for every hook point inside V(Q) and within the 1600-unit hook range [M], `Req(region(hook))` is added. Reason: a hook flight covers up to 1600 units in about 42 ticks, 0.7 s [D from docs/cortex/hook-points.md: anticipation 12, capture 3, landing at tick 42, sim at 60 Hz], and the target must be camera-visible so it is always in V(player) by construction. The landing closure must already be resident when the hook can be chosen, because 0.7 s is far below one region load.
- **Home pin:** `Req(checkpoint)` for the last used checkpoint is permanently pinned, so a death and respawn never waits on the drive. (A respawn is a discontinuity; pinning is what keeps it from becoming a disguised loading screen.)
- **Combat scope F:** section 6.
- `Need(p) = union of Req(Q)` over regions Q intersecting the viewer ball, `+ hook edges, + home pin, + F`.

**Invariant I1 (hard):** at every frame, every region in `Need(p)` is admitted (installed, textures uploaded, linked).
**Invariant I2 (lead):** every region in `Lead(p) = union of Req(Q)` for all Q with walk distance(p, Q) <= H_lead is *requested*, nearest first. H_lead is the distance the player can cover while the drive delivers what the invariant needs next (4.2).
**Invariant I3 (fail-safe):** a trace that reaches an unresident stub is solid (section 3.6).

### 4.2 Sizing H_lead (arithmetic, all estimates labelled)

Inputs:
- v_run = 100 authored/tick units in `projects/graybox-reach/project.ron` (`run_speed: 100`); `units.rs::speed_q8` makes it `100 / 16 = 6.25` engine units per tick; at 60 ticks/s that is **375 units/s** [D]. Roll (46) and backstep (72) are slower than run [M, project.ron]. Hook is handled as a discrete edge. Terminal fall speed is 48 engine units per tick (2,880 units/s) with gravity 6 units/tick^2 [M, `character_motor.rs` `MAX_FALL_SPEED`, `GRAVITY_PER_TICK`]; the engine has no knockback velocity constant (an attack's push is its authored `action_push` distance spread over the push window, `player_action_push_speed`), so there is no separate knockback ceiling to read. Design speed uses a safety factor kappa = 1.25 [E]: **v_max = 470 units/s** [D].
- Drive: 6.49 ms/sector at 2x [M]. Region of 16 sectors: 104 ms [D].
- Seek, by sector distance 1 / 16 / 128 / 512: 11 / 79 / 137 / 310 ms [M]; run-to-run swing about 2x [M].
- Per-region service time T_r = seek + read:
  - neighbours inside 128 sectors: 137 + 104 = **241 ms** [D]
  - far (512 sectors): 310 + 104 = **414 ms** [D]
  - far with a 2x swing: 2 x 310 + 104 = **724 ms** [D, assumption that the swing applies to the seek only]
  - batching k regions per seek: k x 104 + 137 ms, e.g. k = 4: 553 ms for 4 regions = 138 ms per region [D].
- Install CPU (verify + relocate + upload scheduling) per region: [?] measure in M7, enters as T_inst.

Lead time for a queue of q regions ahead of the one you care about: `T_lead = q x T_r + T_inst`. Lead distance `H_lead = v_max x T_lead`:

| q | T_r = 0.241 s | T_r = 0.414 s | T_r = 0.724 s |
|---|---|---|---|
| 1 | 113 units | 195 units | 340 units |
| 2 | 226 units | 389 units | 681 units |
| 4 | 453 units | 778 units | 1361 units |

(470 x q x T_r, T_inst ignored) [D]. For scale: D_vis is 2860 units and the current Cortex level is about 3950 units long [M, render.rs comment]. So the lead ring is a meaningful fraction of the visible range in the pessimistic corner, and a fraction of one region in the optimistic one. The cooker computes the real q for each position from the actual graph (how many regions can be outstanding) rather than assuming it.

### 4.3 Bandwidth feasibility (what the cooker checks, "cook gate")

Let `B_eff` be the sustained region throughput:
- unbatched, neighbour-class seeks: 16 sectors x 2048 B / 0.241 s = **136 KB/s** [D]
- batched k = 4: 64 sectors x 2048 B / 0.553 s = **237 KB/s** [D]
- hard ceiling 150 sectors/s x 2048 = 307 KB/s [D].

Design target utilisation U <= 0.5 [E] (leaves room for textures, archetypes, combat fill, retries, and keeps the average PIO tax near 10%). At v_max the player generates `rho` bytes of newly required data per unit of travel, and feasibility is `rho x v_max <= U x B_eff`. So the cook must verify, over the region graph, the worst `rho` along any path:

`rho_max <= 0.5 x 136 KB/s / 470 u/s = 145 bytes per unit` (unbatched) [D, rises to about 252 B/u when batched at 237 KB/s] [D].

In plain terms: along any route, the data newly entering the required set may not exceed about 145 bytes per unit travelled (about 68 KB per 470 units, i.e. two 32 KB regions per second of sprinting [D]). The cook report prints `rho` per region edge, the worst path, and a pass/fail. Numbers are estimates until M0 and M12 replace them with console measurements.

The static memory side of the same check: `bytes(Need(p) union Lead(p)) <= pool` for every p (sampled on a lattice over walkable space), otherwise the cook fails with the offending position and the contributing regions.

### 4.4 Eviction

- Victim order among **unpinned** regions: outside `Lead(p)` and outside hysteresis first, then farthest walk distance, then least recently used. Pure LRU is wrong for a world traversed in a line (the thing behind you is LRU but may be needed on backtrack).
- Hysteresis: a region leaves the keep set only when its distance exceeds `H_lead + h`, with h = max(typical region width, 0.25 x H_lead) [E], so a player standing in a doorway cannot thrash two regions (p6 item 5).
- Pinned: Need(p) members, hook edges, home pin, combat scope F, anything with an install in flight, regions holding a live aggro'd actor.
- Eviction commits at a frame boundary only: unlink (patch parent to the stub), bump the layout generation, then free pages. Slices handed to consumers are generation-checked (the `ResidentRoomHandle` pattern from the retired grid code, section 10).
- Failure policy (from the grid scheduler): backoff 16 to 512 scheduling windows per consecutive failure, never abandoned, so a bad sector cannot churn the drive.

### 4.5 Budget table

"Known" values come from the survey; "needs measuring" values are not stated anywhere I could find. I do not fill in unmeasured cells. The last column was filled on 2026-10-08 by the M0 tooling: `occupancy-report` (`editor/crates/psxed-project/src/occupancy.rs`, also the `occupancy` MCP tool) reads the guest link map, executable header and cooked project, and the emulator dumps when they are supplied; every row says which. Cells still marked [?] are the ones that need an emulator run of the Graybox Reach disc (VRAM pages, SPU RAM contents, heap at a defined point) or the console.

| Pool | Known today | Proposed role under streaming | Needs measuring at head |
|---|---|---|---|
| Main RAM total | 2,097,152 B; static region cap 1,998,848 B [M, survey 2] | | Measured 2026-10-08 on editor main c08e72d8 with `occupancy-report` (below). Graybox Reach, default build: `.text` 933,200, `.data` 221,872, `.bss` 826,444, static image 1,981,516, **static headroom 17,332 B**, heap capacity 17,076 B [M, link map]; no guest code allocates (`extern crate alloc` is `cfg(test)` only in editor-playtest), so free heap should equal that capacity at any point, which a RAM dump would confirm [D]. Cortex Ignition 0.5 does **not link** at this revision: `.bss` overflows by 64,012 B (lld says the same number), `.text` 915,856, `.data` 366,192, `.bss` 780,812 [M, link map of the failed link]. The newest linked Cortex map on record is still 09-05 [?] |
| Static image | `.text` 712,032, `.data` 385,696, `.bss` 851,204 (0.4b, 09-05); free heap 41,040 B [M] | `.data` loses the baked PXBSP (91 KB generated tree) and BSP textures (97,576 B in the souls slice) [M] | Graybox Reach largest statics [M, link map]: `RUNTIME_ARENAS` 766,808 B (.bss; 643,072 B of it is the 314-page persistent-asset arena [D from the manifest]), baked PXBSP 90,892 B (.data), `SCENE` 46,448 B, `OT` 16,384 B, cube-sky packet cache 9,260 B, three 8,252/4,156 B baked textures. Cortex 0.5: `RUNTIME_ARENAS` 705,400 B, baked PXBSP **216,012 B**, `SCENE` 60,976 B. The BSP image is the item streaming removes from `.data`; the BSP textures stay in `.data` (8,252 B each) unless they move to the texture packs |
| Session-resident clips | 314 pages = 643,072 B (cap 985,088) [M] | becomes per-archetype residency (section 5) | Per-archetype resident clip bytes [M, cook]: Graybox Reach Aletha 392,884, Light Enemy 248,292, Zenith Projector Body 500, Sword1 Light Energy 36 (641,712 B in 314 pages, 65% of the 481-page ceiling); Cortex 0.5 Aletha 284,400, Light Enemy 154,004, Heavy Enemy 141,052 (579,492 B in 283 pages). Models, atlases and SFX per archetype are not separated yet [?] |
| Region page pool | none | the new pool P_world, sized as the reclaimed bytes minus a safety floor | P_world is a design output, not a measurement. Inputs [D]: Graybox Reach has 17,332 B of static headroom plus the 90,892 B BSP image it would stop baking, 108,224 B before a safety floor; Cortex 0.5 needs 64,012 B just to link, so the 216,012 B BSP leaves 152,000 B. Fragmentation margin [?] |
| Skeleton (directory, graph, V lists, top trees) | StreamingIndex empty | resident, scales with region count | bytes per region, measured on the stress world (est. 50 to 100 B) [E] (not done: needs the M4 cooker) |
| Install scratch (verify/relocate) | | small, sliced | |
| Packet arena | 192 KB `DEFAULT_PACKET_WORDS` [M, render.rs]; the playtest guest's own primitive arena is `PLAYTEST_PACKET_CAPACITY` 1,536 slots x 14 words x 4 = 86,016 B for both projects [D, manifest] | unchanged | |
| VRAM | 1 MB; two 320x240x16bpp framebuffers = 307,200 B [D]; 64-slot allocator, 64-halfword tpage stride [M] | texture slots refcounted by admitted regions and archetypes | The tool now prints it (`occupancy-report --vram`), but the page-by-page numbers need a VRAM dump from one emulator run that has not been made [?]. Cook-side demand [M, cook]: Graybox Reach asks for 20 textures, all 4bpp, 392,320 B of pixels plus 992 B of CLUT entries if all were resident, the 1536x256 cube sky alone 196,608 B (six pages); with the framebuffers that is 699,520 of 1,048,576 B, 349,056 B free [D] |
| SPU RAM | 512 KB; UI SFX 56,704 B in July [M] | SFX bank map with LRU; optional ring | Graybox Reach [M, cook]: 5 SFX samples, 8,960 B from 0x30000 to 0x32300, 318,704 B free above the bank, 192,496 B unused below it; the engine uploads at most 64 samples. The dump-side confirmation (`--spu`) needs the same emulator run [?] |
| CPU | PIO tax about 1.3 ms/sector [M, one source] | stream budget per frame via the `TaskLane::StreamingBudget` lane that exists unused in `psx-engine/src/scheduler.rs` [M] | Handler duration, emulator-measured by `hello-cdstream-probe` (psx-cdstream, 2026-10-08): longest handler 1,242 us, 1,239 to 1,248 us of foreground CPU per sector, 16.6% lost at 2x and 8.5% at 1x, 19 of 20 requests chained, 0 sectors dropped; these are the emulator's model, console numbers wait for Manny's disc. Install CPU (`T_inst`) [?] (M7) |
| Drive | 150 sectors/s; seeks as in 4.2 [M] | U <= 0.5 target | console re-measurement with the production transport: the probe disc is built (`hello-cdstream-probe`, EBonura/PSoXide `feat/cdstream-probe`); emulator run for scale: forward seek 35 / 107 / 169 / 353 / 332 / 345 ms for 1 / 16 / 128 / 512 / 2048 / 8192 sectors (emulator-measured, no rotational phase, not comparable to silicon) [M] |

The two hard unknowns that can invalidate numbers above are P_world and the VRAM texture budget. M0 produces both as printed tables before any design number is trusted.

---

## 5. Actors

- **Spawns live in the region payload** (entity records, 50 B each [M], `MapEntity::SIZE`). A spawn is instantiated when its region is admitted and its archetype is resident. A spawn's home region is where it is authored.
- **Archetype pack** (one per enemy type, 2 today): model, animation clips, atlas texture, SFX bank. Loaded as one contiguous page-pool run so `Model<'static>` views keep stable pointers; a generation counter invalidates parsed views on release (the existing `PersistentAssetStreamer` already has the generation counter; its bump allocator and never-called `compact()` are replaced by pool runs). Player archetype is permanently pinned ("core").
- **Requirement:** archetype A is required whenever any region in Need(p) contains a spawn of A (it may be visible from afar, and an enemy that appears with its region is not pop-in; an enemy missing its mesh would be). Cooker rule: the number of distinct archetypes in any V closure must be <= A_max (resident archetype slots, [?] pending the per-archetype byte measurement). Exceeding it is a cook error with the offending closure listed, not a silent LOD.
- **VRAM/SPU dependencies of an archetype** (atlas, SFX) are admission prerequisites of any region that references it, exactly like region textures.
- **Reach rule:** an actor must never be able to step onto an unresident stub. The cooker adds ball(spawn, leash) to the collision requirement of the spawn's region. Leash = aggro_radius x `GAME_ENTITY_LEASH_FACTOR` (2) [M, `psx-game-runtime/src/entities.rs:184`]; the current Graybox enemy has aggro 6400 authored = 400 units [D], so leash 800 units, which matches `docs/cortex/enemy-behaviour.md` ("800-unit spawn leash") [M].
- **When a region unloads:** the region is outside Need(p) and the lead ring, so none of its actors is visible or aggro'd. Dormant actors are despawned after writing a compact **spawn state** (alive/dead, a few flags) into a resident table (about 1 to 2 B per spawn [E]; 2000 spawns = 4 KB [D]). On reload the spawn restores from that state; patrol phase restarts. Dead stays dead until the rest/checkpoint reset the table, so streaming cannot resurrect enemies. A region holding an aggro'd actor or in combat scope F cannot unload (pinned), so there is no mid-fight despawn.
- **Entities that cross region boundaries** (projectiles, knockback) are clamped by I3: a projectile reaching a stub dies; an actor can only be driven into a stub by a force larger than the leash margin, which cooker margins make impossible for authored values.
- The legacy `input.player_room != record.room` leash/disengage tests in `entities.rs` and `entities/tactics.rs` are grid concepts and must be re-expressed as a region/leaf-based home test when the grid is removed (section 10 note).

---

## 6. Combat radius handoff

### 6.1 Definitions

- `Scope(e)` for an aggro'd enemy e: all regions whose walk distance from e's home is <= `r_c`, taking their `Req` closure, where
  `r_c = leash + v_max x T_exit`, `T_exit = T_duck + T_pause + T_first_read`.
  - leash = 800 units for the current enemy [D, above]. Combat ends when the player is beyond the leash (enemy-behaviour.md) [M], so the leash is the natural hard edge of a fight.
  - T_duck: the fast forced fade. The normal fade is `COMBAT_MUSIC_FADE_OUT_TICKS` = 120 ticks (2 s) and fade-in is 60 ticks [M, game_app.rs:352-353]. Using 2 s here would let a sprinting player outrun the frontier by about 940 units, so a **stream-forced duck** uses a short fade, default 30 ticks (0.5 s) [E, tunable, audio feel is yours to tune, it changes nothing structurally].
  - T_pause: time from Pause command to controller idle after CD-DA [?] (M0 probe).
  - T_first_read: first seek from the audio position plus one region, using 310 + 104 ms [D] as the planning value.
  - Planning value: `T_exit = 0.5 + T_pause + 0.414`; with T_pause unknown, T_exit is about 0.9 s + T_pause [D]. At 470 units/s that is **about 430 units + 470 x T_pause** beyond the leash [D]. So r_c is about 1230 units + [?] for the current enemy.
- `F` = union of `Scope(e)` over all aggro'd enemies and their groups, plus the archetypes, SFX banks, textures and hook-edge closures contained in it. F is computed once at aggro time and re-derived only when the aggro set changes.

### 6.2 State machine (streamer side, engine owns the music policy)

```
 Idle ---aggro--> Filling ---F complete + all prerequisites admitted--> Armed
   ^                |                                                   |
   |                | (fill timeout T_wait_max [E] or hard read error)  | engine: start CD-DA fade-in (60 ticks)
   |                v                                                   v
   |            Silent (combat continues without music; retry with backoff)   Quiescent (lease Audio, no data reads)
   |                                                                    |
   +---- combat ends (normal fade 120 ticks) ----------------- Releasing <+---- duck trigger (6.3)
```

1. **aggro**: P1 `CombatFill` requests for every missing member of F go in the queue; any in-flight P3/P4 (speculative or background) read is aborted at the next sector. P0 Demand (the viewer's own required set) still preempts. Combat gameplay starts immediately and is **not** blocked on the music.
2. **Filling**: the scheduler drains F nearest-first, batching contiguous groups. Completion means every region of F admitted (textures uploaded, archetypes resident, SFX banks in SPU RAM) and no request in flight.
3. **Armed**: the streamer reports F complete. The engine asks for the audio lease, the arbiter drains the transport (aborts nothing, none in flight), grants, and `CddaPlayer` starts the combat cue with the existing 60-tick fade-in. Read suspension is now in force: no data request is issued while Audio owns the drive. The combat music gate in `update_ui_music` (game_app.rs:2623) changes from "level fully resident after loading" to "streamer says F complete".
4. **Quiescent**: player moves freely inside F. By construction every region the player can reach within the leash, plus the exit margin, is already resident.
5. **Release** (combat ended normally): existing 120-tick fade, then Pause (not Stop), `release_audio_lease`, streaming resumes. The first read pays a seek from the audio position (up to the 310 ms class [M]).

### 6.3 Player leaves the radius mid-combat

Two cases, handled differently:

- **Normal disengage (crossing the leash).** Combat ends by the existing rule; the engine switches to the **forced duck** (30 ticks) instead of the 120-tick fade because the streamer reports "player is within `d_duck` of the F frontier" where `d_duck = v_max x (T_duck + T_pause + T_first_read)`. Release proceeds, reads resume, the exit margin in r_c has already covered the player's travel during that time. This is the "extend residency in advance" option, chosen as the primary mechanism.
- **Unexpected: the player is about to touch the F frontier while enemies are still aggro'd** (knockback, a dash/roll chain, a hook, a scripted push). Do not let I1 break. The streamer raises a P0 `Emergency` need, the engine ducks immediately (forced, 30 ticks), the lease is released, and reads resume. Music is the thing that yields; world integrity never does. Hook candidates whose landing closure is not fully resident while the lease is Audio are not selectable (the crystal does not light up). That is a gameplay-availability change, flagged as a question in section 9.
- **Re-arm** after a duck while enemies are still aggro'd: the same Filling -> Armed path runs again. Hysteresis: re-arm only after the player has been back inside F by at least `m = 0.25 x r_c` [E] for a few seconds [E], so music does not thrash on a boundary.

### 6.4 Failure cases

| Case | Behaviour |
|---|---|
| Fill never completes (read error, retries exhausted, T_wait_max [E] exceeded) | no music for this encounter, combat unaffected, retry with the scheduler backoff, log + counter |
| Aggro while a speculative read is in flight | abort at next sector, resume later from `lba + received` |
| A second group aggroes with a scope not inside F | treated as a new Filling for the union; if music is playing, forced duck, fill, re-arm (rare, authored groups aggro together) |
| Player dies mid-combat | death flow fades the music; the home pin is already resident, so respawn needs no read; lease released |
| Controller error / INT5 right after CD-DA | arbiter reclaim uses hk's proven recovery (Pause then re-seek); counter; the streamer never starts a read before the lease is released |
| CD-DA loop restart at track end | uses the laser for audio only (seek back, `docs/cortex/music-looping.md`), no data request exists then, fine |
| A stub is hit by a trace while Quiescent | solid wall, counter increments; by construction must be 0, any non-zero is a gate failure and means r_c or kappa is too small |
| Lid open, disc read failure | same as fill failure; world continues on what is resident |
| Boss arena | F is the arena closure; if the arena has no leash the cooker uses its authored bounds |
| Looping resume position | v1: new random start inside the first 15 s as today; capturing the position (hl-psx `music_suspend_for_stream`) is a later improvement |

### 6.5 Why not the alternatives

The SPU ring (hk-psx) would remove the suspension entirely but costs a 96 KB RAM FIFO and music re-cooked as 22 kHz mono ADPCM; you decided on CD-DA and the combat radius, so it is not the plan. The ring still moves to the SDK (section 11) because streamed ambient audio and long SFX need it and because it stays available as a contingency if console measurement shows the suspend model costs too much.

---

## 7. Editor, cooker integration, debug tooling

### 7.1 Cooker integration

- New modules in `editor/crates/psxed-project/src/`: `brush_region.rs` (partitioner and layout), `brush_region_hulls.rs` (hull BSP), extensions to `brush_pack.rs`/`brush_pxbsp.rs` (region payload writer and StreamingIndex), pack writer reuse via `psx-iso::build_world_pack`.
- No region authoring. A project setting block (budget, caps, kappa, U, hysteresis) with defaults; an optional `RegionHint` node (force a cut plane, or forbid cutting a volume, e.g. a boss arena) as an override, not a requirement. Not needed for M1 to M8.
- The playtest manifest gains a **stream budget** next to the existing fail-closed budget (`playtest/budget.rs`), and the "resident-asset ceiling" check becomes per-archetype once clips are no longer session-resident.

### 7.2 Cook report (always emitted, JSON + text, deterministic)

Region count, payload bytes (min / median / max, fill vs caps), `|V(R)|` distribution and the rank-row width, closure bytes per region, **rho per edge and worst path with verdict**, the largest `Need union Lead` found and where, archetype count per closure, texture duplication factor, disc layout cost and seek-class histogram, skeleton bytes, cook time split (portal flow is the expensive part), and a short list of "regions you may want to look at" (largest closure, worst rho, lowest fill). Any budget breach is an error with coordinates, never a silent clamp.

### 7.3 Editor and MCP

- Editor viewport overlay: region cells as wire boxes colored by payload bytes, closure of the selected region, hook edges, combat scope F for a selected enemy, shortest-walk path used for the distance metric.
- `psxed-mcp` queries (the server already exists, `editor/crates/psxed-mcp`, with an `audit` module and the engine-stress runner `benchmarks/engine-stress/mcpc.py`): `region_report`, `region_at(point)`, `closure(region)`, `feasibility(route)`, `scope(enemy)`, `layout_cost`, and `simulate_route(tape)`.
- **Host streaming simulator**: the scheduler/residency code is `no_std` and host-compilable, drive timing is a table-driven model of the measured seek and read numbers (with the 2x swing as a random factor), the player is a polyline at v_max. It runs in CI after every cook of a streamed project and fails on any deadline miss. It is the check that makes emulator-only sign-off unnecessary for logic, while console runs remain mandatory for timing (survey 3.2).
- Runtime overlay (debug build): residency map with colors for admitted / loading / queued / evicting, current Need and Lead, queue with ETAs, last 16 request latencies, pool usage and fragmentation, VRAM slot map, SPU map, combat state.
- Telemetry counters (emulator-readable symbols, same style as `HK_*`): per request latency histogram, P0 stalls, unresident stub hits (must be 0), deadline slack minimum, prefetch hit/miss/wasted, evictions, thrash detector, bytes read per class, IRQ max duration, discarded sectors, lease transitions, music-gate decisions. Gates read these.

---

## 8. Milestones

Each is small, independently testable, measured on Graybox Reach (`editor/projects/graybox-reach`, mixed outdoor/interior) unless stated. "Repo" says where the code lands. Milestones M5 can run in parallel with M1 to M4 from day one. Gates are the pass criteria.

**M0. Measure the unknowns (SDK probes + editor tooling).**
- SDK: a console probe disc (`hello-cdstream-probe` example) recording: seek/read table with the production transport, handler duration (Timer2 counter), PIO CPU per sector, whether CD-DA audio survives a data read (record), Pause-to-idle after CD-DA (`T_pause`), first read after CD-DA, 1x vs 2x tax, motor behaviour after Pause vs Stop.
- Editor: a link-map/occupancy printer for RAM, VRAM (page occupancy) and SPU RAM (the tool survey 2 says does not exist); fall and knockback terminal speeds from `character_motor.rs`.
- Gate: every [?] cell in section 4.5 and the T_pause, T_inst inputs have a number with date and build hash. Needs Manny's console for the probe. Repos: SDK (probe), PSoXide-editor (printer).

**M1. Shared IRQ transport (SDK, then hk and cs migrate; see section 11).**
- Land `psx-cdstream` v0 as a faithful lift of hk's `cd_stream.rs` behind `CdHw`, with the vector install owned by the crate and `declare_stack_safe_handler`, no new behaviour.
- Gates: host tests with the scripted controller for every phase and error path, cancel in each phase, double-speed; hk-psx switches (bump `sdk.lock.json`, delete local file, keep `HK_CD_*` via `PROVIDE` aliases): hk host tests plus `tools/profile_tape.py --require-seamless` and gate tour give identical sector hashes and counters before and after; cs-psx viewmodel stream the same. Console: hk build with the new transport boots, runs a gate tour, no new errors. Repos: PSoXide (crate), hk-psx, cs-psx.

**M2. Arbiter, queue depth-2 with contiguous chaining, abort resume, audio lease (SDK) and engine adoption.**
- Engine replaces `cd_stream/hw.rs` DMA with the shared transport behind the existing `CdController` call sites (`read_chunk_blocking`, `read_chunk_banded`, `UiChunkPlan`, `WorldRoomSlotsReadJob` as used by `asset_streaming.rs`); `CddaPlayer::release_for_data_reads` becomes an arbiter client.
- Gates: boot asset hashes identical on Graybox Reach; emulator discarded/lost-sector counter 0 (was 36 on the Cortex pack, survey section 2); the full existing GameApp music tests (about 60) green; a scripted test that issues lease/release during a read and checks abort + resume position. Repos: PSoXide (arbiter), PSoXide-editor (adoption, lock bump).

**M3. `psx-residency` (SDK).**
- Slot residency policy lifted from hk `room_residency.rs` (wanted list, protected, loading, stored->decoded), contiguous-run `PagePool` with generation handles lifted from `StreamedRoomPages`, retry/backoff and pin semantics from `RoomStreamScheduler`, priority classes, coalesce-or-adopt in flight, per-frame CPU budget; plus the host drive model used by the simulator.
- Gates: hk's residency tests (in `room_residency.rs`) and the 4 grid scheduler tests ported and green; new property tests (never evicts pinned or loading, handle stale-detection, fragmentation behaviour under churn); hk-psx switches `room_residency.rs` to the crate with unchanged tape gate. Repos: PSoXide, hk-psx.

**M4. Cooker partitioner and cook report, host only (editor).**
- `brush_region.rs`: cuts, closure, rho, layout; no format change, no runtime change. Runs on Graybox Reach, graybox-valley, cortex-ignition-0.5 and the terrain fixture.
- Gates: deterministic (second cook byte-identical report); a project below budget yields exactly one region; V rank-row width within 1024 B or a precise error; rho verdict printed; report reviewed by you on the three real projects. Repo: PSoXide-editor.

**M5. Real hull BSPs (editor + psx-bsp stub contents), independent track.**
- `brush_region_hulls.rs`, wired as a replacement of the chain tails in `compile_collision_hulls` behind a project flag, first on whole maps (no regions yet).
- Gates: differential vs current compiler on a large random-trace sample (Graybox Reach, graybox-valley, 16x16 terrain); guest benchmark of hull nodes per trace and cycles per trace on the terrain fixture with a ratio target fixed from the measured baseline; whole-map PXBSP still loads in quake-psx golden tests. This milestone pays off with or without streaming. Repo: PSoXide-editor.

**M6. Region format and slotted map, host only (psx-bsp).**
- StreamingIndex lump (magic + version), stub leaves and contents -7, region loader over `ReadAt`, slot install with relocation, canonical-rank PVS decode as an additive render path, link/unlink patches.
- Gates: with every region installed, the streamed map draws the same polygon set per frame as the whole-map render over a recorded tape (stable per-face cook ids from a debug table; screenshot diff reviewed); under-budget cook is byte-identical to today; quake-psx and HL/CS golden tests and code-size/cycle gates unchanged (legacy path untouched). Repo: PSoXide-editor (`engine/crates/psx-bsp`).

**M7. Guest: regions, all resident, real transport (Cortex).**
- Cortex loads Graybox Reach as regions into a pool larger than the world; no eviction yet. Install pipeline with sliced verify/relocate/upload; textures per region into VRAM slots.
- Gates: tape replay identical to the whole-map build (inputs and positions, plus frame-hash on a fixed set of frames); frame cost within a tolerance fixed from M0 and M6 numbers (the streamed PVS decode is the only new hot cost); measures T_inst. Console boot of the disc. Repo: PSoXide-editor.

**M8. Stress world generator + live eviction and prefetch (emulator).**
- Rust generator `gen_stream_world` (same family as `gen_quake_e1m1_project.rs`) that tiles a few module rooms, a corridor set and 16x16 terrain patches into a deterministic world with R of several hundred regions and total payload at least 10 times P_world, and a route set (shortest paths and random walks, sprint, plus hook edges). The engine runs with the production scheduler, forced small pool.
- Gates: host simulator and emulator agree on deadline slack within a stated tolerance; deadline misses = 0 and unresident stub hits = 0 on every route at v_max x 1.25; backtrack-at-boundary and fast-forward tests from the p6 doc (items 8); eviction never frees a pinned region; the cook rho gate rejects a deliberately dense variant. Repo: PSoXide-editor (generator, engine).

**M9. Collision per region and textures per region.**
- Hull subtrees streamed with stubs; trace hits on an unresident stub are solid with a counter; VRAM refcounts by admitted regions, uploads through `VramRuntime` behind the GPU access guard.
- Gates: injected delay on one region (test hook) makes the player stop at the boundary (wall) with no visual pop-in; counter increments; no wedge. All upload paths reviewed against the present-queue guard (SDK commits ef37b344a, 942bc0267). Repo: PSoXide-editor.

**M10. Actor archetypes.**
- `PersistentAssetStreamer` generalised to archetype packs in page-pool runs, spawn state table, dormant despawn/respawn, reach rule in the cooker.
- Gates: Graybox Reach with the two enemy types: kill an enemy, leave, return, it stays dead; archetype never evicted while a spawn is in Need; per-archetype bytes recorded (fills the last [?] in 4.5). Repo: PSoXide-editor.

**M11. Combat radius handoff.**
- Scope F, Filling/Armed/Quiescent/Releasing, forced duck, hook gating, engine music gate moved from "level resident" to "F complete".
- Gates: scripted tapes: aggro then music start only after F complete; no data request issued while Audio; sprint out through the leash (duck then reads resume, no stub hit); knockback to the frontier (emergency path); fill-failure injection (no music, no hang); the existing 60 GameApp music tests still green. Repo: PSoXide-editor (engine), SDK arbiter already done in M2.

**M12. Console stress disc (larger than RAM) and sign-off.**
- The M8 world at full size, built through the production path, burned and run on Manny's console with the tape routes plus a free-run. Telemetry symbols read back from emulator for parity; on console measured by capture.
- Gates: no loading screen, zero P0 stalls, zero stub hits, deadline slack above the target on every route; measured seek/read table within the model's band; PIO tax matches M0. Fails here send the numbers back to the cooker parameters (kappa, U, caps), not to hand tuning. Repo: PSoXide-editor; needs your console.

**M13. SPU ring and SFX streaming (SDK; hk migrates).** See section 11. Gate: hk's `audio_stream_runtime.rs` tests and tape gate unchanged, Cortex streams one long ambient bank through the ring without a missed boundary.

**M14. Relocatable code modules (SDK; hk migrates, Cortex optional).** See section 11. Not required for Cortex unless M0 shows `.text` pressure.

---

## 9. Risks and open questions

### Risks
1. **PIO tax versus the frame budget.** 20% CPU during reads on top of a renderer at 29.62 fps (survey) can drop frames. Mitigation: bursts, U <= 0.5, M0 measures real handler time, 1x mode lever. Gate: frame-time distribution with and without streaming on Graybox Reach in M7/M8.
2. **Open outdoor closure.** Wide-open terrain has large PVS closures; far-reject bounds the radius but not the bytes. The cook report will show closures; if one exceeds the pool, the only remedies are visual (fog/LOD/draw-distance), which need your approval before they exist. No such lever is designed in.
3. **Seek variance.** Silicon swings about 2x run to run and is not smooth in distance [M]. The model uses a 2x pessimistic corner and the layout optimises sector-distance classes, but only M12 on console settles it.
4. **Render PVS decode cost.** The new canonical-rank decode is on the hottest loop. It is additive and behind a flag so legacy maps are untouched; M6/M7 gates fix the tolerance.
5. **Cook time.** Portal-flow VIS on a large world is the expensive step. Far-reject clipping prunes it, a Draft mode (connected components) keeps editing fast, Release remains for ship builds.
6. **Fragmentation of the page pool.** Contiguous runs can fragment under churn; mitigation is the cooker's size classes, first-fit with a measured margin, M8 churn tests. If it bites, fall back to fixed slots sized by cap.
7. **hk migration regressions.** Mitigated by one migration per SDK pin bump, hk gates before and after, and symbol aliases.
8. **Docs disagree on data-versus-music.** Silicon probes timed the read only. M0 settles it before the combat model is trusted.
9. **Skeleton scaling.** About 50 to 100 B per region [E]; a world beyond a couple of thousand regions needs a second paging level. Out of scope, stated.

### Decided here, with reasoning (not questions)
- Disc duplication of small textures: yes, disc is free, seeks are not.
- Home pin of the last checkpoint closure: yes (a respawn must not wait).
- Initial boot into a save uses the existing loading scene; no loading screens inside the world.
- Combat scope derived from leash plus exit margin; forced duck 0.5 s default, tunable by ear.
- Relocatable enemy code for Cortex: not in scope until M0 shows `.text` pressure.
- Hull BSP before regions (M5 is useful alone).

### Questions that need you (minimum)
1. **What is the allowed behaviour when the guarantee still fails** (disc error, hardware stall, a bug)? My recommendation is the collision wall: the player stops at the boundary of unresident space, there is never pop-in, and the counter turns the gate red. The alternative is a speed governor. Both are gameplay stalls, neither is visual, but you said no disguised transitions, so I do not want to choose the stall style without you. Related: while the lease is Audio, hooks whose landing closure is not resident are not selectable. Is that availability change acceptable?
2. **Console time:** M0 and M12 need runs on your console (and M0 needs one listening/capture check that a data read kills CD-DA). Can you commit slots for those two, and is the CD-DA-death observation in `game_app.rs` enough for you to treat as confirmed until M0?
3. **Approval gate to pre-agree:** if the cook report shows an outdoor closure above the pool, do you want the build to hard-fail (my default) or to offer you the visual options first?

---

## 10. Grid removal: what to salvage before deleting

All paths are `PSoXide-editor` `main`. Warning to the removal agent first: **`WorldRoomSlotsReadJob`, `WorldChunkDestination`, `CdController`, `read_chunk_blocking`, `read_chunk_banded` and `UiChunkPlan` are used by non-grid code** (`asset_streaming.rs:15,191,235,522`, `vram.rs:287-377,1914-2032`, `editor-playtest/src/model_rendering.rs:735,768`, `runtime_arenas.rs:82`). Deleting `cd_stream.rs`/`hw.rs` wholesale would break model loading, UI images and the sky. Delete only grid code; keep these until M2 replaces them. `PSOXWPAK` / `psx-iso::build_world_pack` and `psx-pack` are shared with hl-psx, oot, alttp and wipeout and are not grid-only.

| Component (path) | Verdict | What to save, where it goes |
|---|---|---|
| `psx-game-runtime/src/room_streaming.rs` `StreamedRoomPages<PAGES,SLOTS>` (:1096 to :1480, behind `cd-stream-bench`): contiguous runs of 2 KiB pages, `ResidentRoomHandle` (slot + generation), `prepare_slot`/`can_prepare_slot`/`release_slot`, `WorldChunkDestination` impl (:1482) | **SALVAGE (strong)** | Becomes `PagePool` in `psx-residency` (M3). Keep: run allocation, generation-checked handles, "sectors land directly at their final RAM address", all-zero-valid `const fn new` so statics stay in `.bss`, the lifetime/staleness contract text (also in `docs/level-residency.md`). Drop the room-specific views: `streamed_room_chunk_view`, `compact_collision_room`, `surface_cache_*`, `resident_chunk_bytes`. Add: best-fit/first-fit policy, fragmentation stats, pin counts. |
| `RoomStreamScheduler<N,M>` (:136 to :900) | **EXTRACT POLICY, delete the rest** | Failure backoff (`stream_retry_backoff_windows`, 16 to 512 windows, :87-98), pinned-window flags and "protected prefix so prefetch never evicts" (`reconcile_residency` :270-322), epoch and `residency_generation` pattern, telemetry counters (requests, misses, prefetch requests, evictions, failed loads). Goes into `psx-residency` (M3) beside hk's slot policy. Port its 4 tests. `room_graph_ring` (:1541) is a portal-hop ring over rooms: replaced by walk distance on the region graph; keep it only as a test oracle until M8, then delete. |
| `cd_stream.rs` `WorldRoomSlotsReadJob<N>` (:257 to :730) | **KEEP until M2, then replace** | Used by `PersistentAssetStreamer`. Idea worth carrying into the scheduler: coalescing consecutive chunks into contiguous disc runs (`begin_next_group` :662) and per-chunk FNV verification with a status per chunk. The polled `poll_into`, `set_pause_at_poll_boundary`, `set_wait_for_sectors` and the sector budget per tick are dead ideas under the IRQ transport. |
| `cd_stream.rs` `CdController`, `read_chunk_blocking`, `read_chunk_banded`, `UiChunkPlan` | **KEEP until M2**, then delete | Boot/UI/sky readers. Replace by shared transport + `psx-pack` chunk load. |
| `cd_stream.rs` benchmark (`run_benchmark` :737, 32 sectors at LBA 992 plus a WORLD.PAK pass) | **SALVAGE the idea** | Becomes the M0 SDK probe (it is misnamed: feature `cd-stream-bench` is the CD-backed build shape, `cd-stream-benchmark` is the probe). Rename both when the code moves. |
| `cd_stream.rs` constants `SEEK_BREAK_EVEN_SECTORS = 8`, `drive_sectors_per_background_tick`, `CD_SECTORS_PER_SECOND_*` | **DELETE (stale)** | Break-even is derived from a pre-silicon figure; the silicon table (section 4.2) replaces it. |
| `cd_stream/hw.rs` (629 lines): `prepare_cd_read`, `start_cd_read_at_lba` (bare SetLoc+ReadN), `dma_read_sector` (ch3 DMA 0x11400100, no `dma::abort`), status codes | **DELETE after M2** | Nothing to save. The `STATUS_*` timeout taxonomy is covered by the new transport's diagnostic codes. |
| `psx-game-runtime/src/asset_streaming.rs` (`PersistentAssetStreamer`, `PersistentAssetStorage`, 14 tests) | **KEEP, not grid** | Evolves into archetype residency (M10): swap bump allocator for pool runs, keep the generation counter. `compact()` stays unused and can be removed. |
| `psx-game-runtime/src/vram.rs`, `vram/upload_queue.rs` | **KEEP, not grid** | Reused for region textures (M9). The grid-specific constants (`ROOM_TPAGE_BASE_X = 640`, 6 room tpages, room-material drops) go; the 64-slot allocator, rows-per-tick upload queue, `evict_unreferenced_vram` stay. |
| `psx-game-runtime/src/schedule.rs` (`RuntimeScheduleConfig`) | **EXTRACT** | Keep the background-work pacing knobs conceptually, folded into the `StreamingBudget` task lane (`psx-engine/src/scheduler.rs`). Delete the portal-depth and room-window knobs. |
| `room_window.rs`, `room_cache.rs`, `room_visibility.rs`, `world_cells.rs`, `world_visibility.rs` | **DELETE** | Grid-only (room window, portal-frustum room visibility, per-cell PVS, grid topology). Nothing reusable that is not already in psx-bsp. |
| `room_lighting.rs` | **DELETE after a check** | Grid per-room ambient/lights/fog view. Verify the BSP path does not import it before deleting. |
| `editor-playtest/src/active_room_streaming.rs`, `active_rooms.rs`, grid branches of `playtest_runtime.rs`, `bsp_runtime.rs` fallbacks | **DELETE** | Dead behind `USES_PXBSP`. |
| Cooker side: `playtest/manifest.rs` `validate_streamed_room_chunks`, TOC/slot-count and chunk-cap code (:26, :492-584), `psx-level` `MAX_STREAMED_ROOM_CHUNK_BYTES`, `LevelWorldPackEntryRecord` | **DELETE grid parts, KEEP pack machinery** | Keep the fail-closed budget-check style and `build_world_pack`; the chunk TOC logic is superseded by the region directory. |
| `entities.rs:2212`, `entities/tactics.rs:606-705` `input.player_room != record.room` leash tests | **REWRITE, do not just delete** | They are the "different room disengages" rule. Replace with a distance/region-home test; this is a gameplay change the removal must preserve. |
| Docs: `docs/level-residency.md`, `streaming-audit-2026-07-25.md`, `streaming-audit-2026-06-12.md`, `legacy-grid-boundary.md` | **KEEP as history, mark superseded** | `level-residency.md` lifetime contract and the audits' findings (failure backoff, one-job pipeline, miss accounting) are inputs to M3/M8. |

---

## 11. hk-psx streaming into the shared SDK/engine

Principle: move code that has no HK vocabulary; leave scene/gate/room-grid logic in HK. Move one piece per SDK pin bump so any regression bisects to a single change, with HK's own gates run before and after on identical inputs (host tests, `tools/profile_tape.py --require-seamless`, gate tour, then console).

| Piece | HK source | Target crate / repo | What stays HK-specific | Notes |
|---|---|---|---|---|
| IRQ CD transport | `game/src/cd_stream.rs` (364) | `psx-cdstream` / SDK (PSoXide `sdk/crates`) | gate-tour trace buffer behind a feature, `HK_*` symbol names as linker aliases | M1. Also absorbs cs-psx `cd_irq.rs` (adds `IrqMaskPolicy::OnlyWhileBusy`). Install via crate, not raw vector write. |
| Drive arbitration | `disc.rs` `Drive` struct and the "one drive" comments (prefetch / pool / music / XA / clip flags) | `psx-cdstream` arbiter / SDK | the specific owners (area music refill, XA title/boss) become callers of `lease`/`request` | M2, optional for HK until its next change; HK keeps its flags meanwhile. Prevents each game re-inventing the exclusion. |
| Slot residency policy | `game/src/room_residency.rs` (279, with tests): `Residency`, wanted list, protected upload, stored->decoded pipeline, `victim` scoring | `psx-residency` / SDK | `SLOT_COUNT = 5` as a const-generic parameter, region ids are `usize` handles | M3. Pure logic, moves with its tests. Cortex wraps it with the distance-aware victim rule. |
| Prefetch machinery | `disc.rs` `Prefetch` (hint -> plan -> read -> ready, adopt in flight `prefetch_take`, wasted/hit accounting, `preverify_step` idle-time hashing, `prefetch_guard`) | `psx-residency` (request lifecycle: Queued, Reading, Landed, Adopted, Wasted; budgeted FNV verifier) / SDK | the gate-specific plan (`stage_group`, group table, code-first policy) | M3/M8. "Adopt an in-flight read" becomes request coalescing in the scheduler. |
| Prefetch planner | `game/src/preload.rs` (neighbour cells, facing, vertical approach, `with_demand`) | **stays in HK** | all of it (2D side-scroller cell topology) | Only the interface moves: a planner emits an ordered `Wanted` list, `with_demand(id)` puts a required id first. Cortex writes a 3D planner (walk distance + velocity) against the same interface. |
| LRU RAM slots | as above | as above | | |
| SPU music ring | `game/src/audio_stream.rs` (376): ring state, FIFO of whole sectors, half-boundary detection via SPU IRQ address, `MAX_POLL_GAP`, boundary tick windows | `psx-spu` `stream` module (or `psx-spu-stream`) / SDK | `PITCH`, `VOICE`, `MUSIC_TRACKS`, premix cook and the refill chunk table | M13. FIFO is fed by the shared transport (sink = FIFO tail, two requests on wrap). Used by Cortex for streamed ambience/long SFX; also the contingency for music. Keep HK's `tests/audio_stream_runtime.rs`. |
| Relocatable code modules | `game/src/modules.rs` (728): install with J26/HI16/LO16/W32 fixups sliced per frame, hash verify, pool inside one 64 KiB `lui` window, LRU with pinning, import-site rebinding to a trap on evict | `psx-reloc` runtime / SDK, and the cooker ported to Rust (`host/code_modules.py`, 730 lines of Python today; "Rust only" is your standing direction) | scene->module masks, art/carry/ALWAYS chunk kinds, build-written `HK_MODULE_*` tables, gate flow | M14. Needs the post-link tooling (`psoxide-link`/hazard patcher) to emit import tables generically. Cortex adoption deferred. |
| Group/disc layout | `disc.rs` group table, "payloads sit on the disc in the order they are read" | cooker layout (editor) | the HK group definitions | The idea is generalised into section 3.7. |
| Scene admission / blackout gate | `scene_transition.rs`, `disc.rs` `admit_scenes` | **stays in HK** | everything | Cortex has no gate by requirement. |

**Order and non-breaking rules**
1. M1 transport: SDK lands the crate and host tests; HK bumps the pin and deletes `cd_stream.rs` in the same commit that adds the `PROVIDE` aliases; CS follows. Console check on the hk build before anything else moves.
2. M3 residency: HK replaces `room_residency.rs` with the crate type; same slot count, same tests, same tape results.
3. M13 SPU ring: after the transport is shared (the refill requests use it).
4. M14 relocation: last, the largest and the only one needing cooker work.
Each step: the hk-psx repo changes its `sdk.lock.json` pin and nothing else in that commit besides the deletion/alias; hk gates must produce the same payload hashes and counters as the previous pin.
---

## 12. Things I noticed and did not touch

- `psx-pack/src/cd.rs` has a function named `dma_read_sector` that is PIO (survey), misleading.
- `cd-stream-bench` (feature) is not a benchmark, `cd-stream-benchmark` is; the names invite mistakes.
- `SEEK_BREAK_EVEN_SECTORS = 8` is stale against the silicon table.
- `hk-psx` `install()` writes the exception vector directly; three SDK-adjacent consumers each wrap psx-rt's handler by hand.
- `hk-psx/docs/STREAMING.md` is stale against the later predictive-prefetch work.
- `PersistentAssetStorage::compact` and several grid paths are `#[allow(dead_code)]` "for later".
- hk's relocation cooker is Python (`host/code_modules.py`), against the Rust-only direction.
- `entities.rs` / `tactics.rs` still reason in grid "rooms" for leash and disengage.
- Visibility lump capped at `u16::MAX` bytes and rows at 1024 bytes are silent world-size ceilings of the whole PXBSP path today.
