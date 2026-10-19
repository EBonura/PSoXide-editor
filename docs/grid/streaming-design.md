# Grid world streaming: design (2026-10-17)

Status: design only. Nothing was built, run or edited to write this, except this file. Written against `grid/main` c581c6c7 (worktree `PSoXide-editor-wt/grid-editor`), the SDK pinned by that tree (`components.lock.json`, sdk 629a222f, which already holds `psx-cdstream`, `psx-residency` and `psx-residency-sim`), the SDK repo `main` 84a220abc, hk-psx at `feat/fsm-v2-phase1-2026-10-15` (the streaming files are identical to `main` dd8958f0, `git diff --stat` empty), and old PSoXide-editor history read with `git show <sha>:<path>`.

**Update 2026-10-19:** the first phase is built (level core, records and rooms on the disc, rooms streamed by walk distance, no level data in the executable). What landed, what it cost and what is open: `streaming-runtime.md`.

This is the spec that `level-tools-design.md` section 13 reserves as phase T16 ("Streaming units ... its own spec once T15 lands"). It extends section 7.3 of `level-format-and-runtime.md`, which only sketches streaming. The goal Manny gave: the world is going to be huge, it has to be seamless, it does not fit in memory, and the work being built now must not make streaming impossible later.

Labels on every number: **[M]** measured (source named), **[D]** derived by arithmetic from [M] values or struct sizes (shown), **[E]** estimate or tunable default I propose, **[?]** unknown, needs a measurement. Where a number comes from a memory note and not from a file I could open, it says "memory".

---

## 0. Summary

1. **Most of the machinery already exists and is not in the engine yet.** The SDK has an interrupt-driven CD transport (`psx-cdstream`), a residency policy with priority classes, walk distance, deadlines, eviction tiers and failure backoff (`psx-residency`), and a host simulator with the measured drive model (`psx-residency-sim`). The PGL chunk format is already collision first and sector aligned, and the collision code already treats a non-resident room as a solid box. What is missing is everything grid-specific: the cooker stage that cuts big rooms into units, the guest glue that feeds `psx-residency`, per-unit textures, spawns and state, the combat-radius music handoff, editor overlays, and disc layout.
2. **Hollow Knight does not stream a continuous world.** hk-psx loads one scene at a time into one arena, every spatial view inside a scene is resident, and a scene exit is a blackout load that a predictive prefetch shortens. The parts worth reusing (IRQ transport, slot policy, relocatable code overlays) have already been lifted into the SDK, and hk-psx has not yet moved onto them. The part that makes HK feel seamless, music that keeps playing through loads because it comes from an SPU ring, is not in the SDK and is not usable by Cortex as designed (section 3.10). So "reuse HK's code" mostly means "use the SDK crates that came out of it".
3. **Unit = chunk = `RoomRecord`.** Authored rooms stay as big as the author wants. A cook stage cuts any room above a byte target into rectangular units on a 4-sector world lattice, after derived walls, bands, stitching, UV and light have been computed on the whole room. Each unit is an ordinary room at runtime, with portals derived between units. Activation and residency stop being counted in portal hops and become walk distance over the portal graph.
4. **Residency is a rule the cooker can check.** Need(p) is what the draw can touch this frame, Lead(p) is what the player can reach before the drive could deliver it, plus pins (home checkpoint, combat scope, hook landings). The cook computes the worst bytes entering Need per unit of travel and the peak pool use, and fails when the drive or the pool cannot keep up. Failure at runtime degrades to the existing collision wall, never to pop-in.
5. **Seven things in the current work conflict with this today** (section 4.2): whole-world caps (256 rooms, 16,383 faces, +-409 sectors, a 512-page cap), the all-resident `include_bytes!` runtime with a one-shot material latch, the world-space frustum test clamped to `i16`, hop-based activation, O(rooms) collision loops, the `room << 8 | local` mover id, and global static entity records with a 64 entity cap. None blocks the current phases. All are cheaper to fix now than later.
6. **Honest unknowns that decide the numbers:** free RAM of the grid guest [?], how big the world really is (Manny's call), seek time beyond 512 sectors [?] on a disc that is up to about 333,000 sectors long [D], and whether XA resumes cleanly after a data read on the console [?] (the step exists in the hardware tests, I found no decoded result).

---

## 1. Requirements and silicon constraints

### 1.1 Requirements

| Source | Text | What it forces |
|---|---|---|
| Manny, task brief 2026-10-17 | "the world is gonna be HUGE and should be seamless, since we won't fit the entire world in memory, we should have a streaming system, like the one hollow knight already has (perhaps we can reuse some code), we don't have to do it now but I want you to take this into account." | Whole-world residency is not allowed to be assumed anywhere new. Streaming is later work; keeping it possible is now work (section 4). |
| memory `cortex-streaming-requirements` (Manny 2026-10-08) | 1 "Seamless world like Hollow Knight, no loading screens or disguised transitions." 2 "Stream everything: world geometry, collision, enemies, their animations and textures." 3 Music only in combat; acceptable: suspend streaming during combat, or combat music in SPU RAM downsampled. 4 "The cooker splits the world automatically ... (no hand-authored regions)." | Units come from a cook stage. No loading-screen constructs. The combat-radius design is pre-approved. |
| memory `hk-psx-seamless-ethos` | "keep the experience as seamless as possible, no loading screens, and where they're unavoidable, make them as fast as possible" (2026-09-23); "we cannot accept pop-in" (2026-10-01) | A missing texture or unit may never show as a hole or a late appearance. Today's `material_missing` drop is exactly the behaviour that must stay at zero. |
| memory `cortex-streaming-handoff-2026-10-08`, section 0 | over-budget areas FAIL the cook, no silent detail loss | The cook gate is the guarantee. No quiet LOD to make a budget fit. |
| memory `cortex-streaming-requirements` (2026-10-08, later) | do not touch model and animation RAM residency in the streaming work; later the same night Manny pulled the compression pass forward as the streaming RAM fix | Archetypes are treated as a fixed cost in v1. Whether the compression pass landed is unknown to me (open question 6). |
| memory `ram-floor-dropped` (2026-10-10) | "don't bother about RAM floor, just use whatever RAM is available" | The pool is sized from the real link map minus a stack and heap safety margin, not from a 16 KB floor. |
| memory `all-games-xa-music` (2026-10-15) | XA does not survive data reads either; "Do not promise read-proof music" | No design below promises music that plays through a read, except the SPU-resident option. |
| memory `no-console-ab-discs`, `extend-hardware-tests-not-new-probes` | measurements go into the hardware-tests suite, never a bespoke probe disc | Section 5.2 lists steps for that suite only. |
| memory `cortex-grid-engine-preference-2026-10-16` | Cortex moves to a pure TR-style grid engine | This design is for the PGL world only. |
| memory `gameplay-timing-only`, HK `hk-psx-seamless-ethos` | load time is out of scope fleet-wide, except HK where it is in scope | For Cortex only gameplay frames count. A boot or respawn loading screen is allowed (as in the earlier design, `streaming-design-2026-10-08.md` section 9, "Decided here"). A transition inside the world is not. |

### 1.2 Hard constraints

Drive and transport.

| Fact | Value | Label and source |
|---|---|---|
| Sector rate, double speed | 151.4 sectors/s (one sector per 6.49 ms), single speed 75.6 | [M] silicon hwtest v1.28, memory `silicon-v128-cdstream`; 6.49 and 13.20 ms/sector also in `docs/emulator-accuracy-from-silicon.md` and `sdk/crates/psx-residency-sim/src/drive.rs:6-10` |
| Seek by distance | 11 / 79 / 137 / 310 ms for 1 / 16 / 128 / 512 sectors, not smooth, run-to-run swing about 2x, rotational phase dominates | [M] v1.6 and v1.7 (memory `cd-timing-calibrated-from-silicon`); do not calibrate to one run |
| Seek beyond 512 sectors | the simulator clamps to the 512-sector value (`drive.rs` doc, "seeks beyond 512 sectors are clamped") | [?] never measured. A 74 minute disc is 333,000 sectors [D: 74 x 60 x 75] and a huge world spans it |
| PIO cost | 1,286 us per sector, CPU lost 18.1% at 2x and 8% at 1x, handler peak 1.14 ms, 0 discarded, reads intact | [M] v1.28 psx-cdstream run |
| CD DMA cost | the CPU is held off for the whole burst: 35,488 clocks per 2048 B = 1.05 ms [D: 35,488 / 33,868,800 Hz], so 15.9% of the CPU at 151.4 sectors/s [D] | [M] v2.1 for the clocks. The difference to PIO is about 2 points, so DMA is not worth chasing for streaming |
| CD DMA reliability | chopping-burst ch3 was a state-dependent lottery on this console (CL1/CL2, CHCR busy forever); v2.4 ran eight CD chain variants (SDK, raw, purge, FIFO wait, PIO, CHCR) all clean, one 2x2048 lw DMA timed out once in three runs | [M] memory `console-cd-dma-unreliable`, `silicon-v24-findings`. The two do not agree. PIO is the proven path and stays the choice |
| Data read kills audio | CD-DA signal during a data read 0%, quiet within 13 to 15 ms, with or without the recovery Pause | [M] v1.28. XA is a drive read too (memory `all-games-xa-music`) |
| Pause, read after audio | T_pause 123 ms (emulator was 3.9), first data sector after audio about 945 ms (emulator 344), resume to playing about 1.0 s, resumes in place 3 of 3 | [M] v1.28 |
| Never Stop | a read right after Stop takes 2.7 s, settled 2.0 s; after Pause the first sector is 100 to 220 ms | [M] v1.28, v2.1 |
| Pause details | second response 5.33 / 6.17 / 8.39 sector periods, Pause during a seek is refused with INT5 | [M] v2.7 (memory `silicon-v27-findings`) |
| Unexplained | a read 15 s after Pause took 4,410 ms to its first sector (v1.28 measured 134 ms) | [M] v2.7, unrepeated. A repeat is queued for a later hardware-test version |
| XA after a data read | `Player::suspend` and `resume` exist on SDK main (84a220abc, not in the pin 629a222f); the engine restarts the song from the head position after a data read (`84e7512d`, on `grid/main`). Hardware-test v2.6 added the timing step (`7c8b0cc0`) | [?] no decoded silicon result found |

Memory.

| Fact | Value | Label and source |
|---|---|---|
| Main RAM | 2,097,152 B, guest region cap 1,998,848 B | [M] `streaming-survey-2026-10-08.md` section 2 |
| Free RAM of the grid guest | not measured. The last figures are BSP builds: 332,956 B static headroom on 10-09, then a corrected pool of 369,383 B once 35,436 B of streaming code and a doubled skeleton were charged | [M] `streaming-world-sweep-2026-10-09.md` (sweep section "Sliver regions"). The grid guest has no baked PXBSP, a different `.data` and different arenas, so these do not carry over [?] |
| Grid data in the EXE today | `grid_level.pgl` and `grid_rooms.pgr` are `include_bytes!`'d (`engine/examples/editor-playtest/src/grid_runtime.rs:52,61`) | [M] code |
| Transport code | `psx-cdstream` is 17.4 KiB of `.text` plus `.rodata`, +4.4 KiB `.bss` with the engine's two-window staging ring | [M] `streaming-design-2026-10-08.md` section 13 |
| Residency code size | `psx-residency` is about 3,400 source lines (tests excluded); guest size not measured | [?] |
| Per-cell cost of a chunk | 107,960 B for 858 cells in the Cathedral = 125.8 B per cell, 2.0 quads per cell | [D] `level-format-and-runtime.md` section 7.4 (one sample, an open hall) |
| Pool granularity | 2 KiB pages, a chunk wastes half a page on average | [D] |
| Chunk scratch | renderer scratch is sized once for the largest room, 27,748 B for the Cathedral | [D] `level-format-and-runtime.md` section 7.4 |

Video, audio, CPU.

| Fact | Value | Label and source |
|---|---|---|
| VRAM layout | 1 MB. Framebuffer 320 x 480 (two buffers, 307,200 B [D]), CLUT band 320 x 32 under it, two page rows 704 x 256 each right of the framebuffer (`vram_runtime.rs:15-40`). 64 VRAM asset slots per cook (`playtest/budget.rs:43`) | [M] code |
| Materials today | every level material is resolved and uploaded before the first frame; `draw` asserts `materials_ready` (`grid_runtime.rs:550-565,767-770`); texture bytes live in the EXE's `ASSETS` table | [M] code |
| SPU RAM | 512 KB. Cortex occupancy not measured. HK's streamed ring costs a 98,304 B main-RAM FIFO (`hk-psx game/src/audio_stream.rs:23-24`) | [?] occupancy |
| Frame budget at 30 fps | 1,128,960 cycles | [D] 33,868,800 / 30 |
| Grid frame today | world 237k to 280k, character about 470k, sky about 58k cycles, 25.9 to 26.5 fps on the Cathedral | [M] `level-format-and-runtime.md` section 1. Sample is one room |
| Player speed | run 375 units/s (project `run_speed: 100`, `/16` per tick, 60 ticks/s) | [D] carried from `streaming-design-2026-10-08.md` section 4.2. Design speed v_max = 470 units/s with a 1.25 safety factor [E] |
| Sector size | S = 80 runtime units (scale 5/4, the new-project default) or 84 (21/16) | [M] `level-format-and-runtime.md` section 2.3. At S = 80 the player runs 4.7 sectors/s and v_max is 5.9 sectors/s [D] |
| Combat leash and hook range | leash 800 units = 10 sectors (`aggro_radius` 6400 authored /16 x 2, `GAME_ENTITY_LEASH_FACTOR` `entities.rs:184`, `docs/cortex/enemy-behaviour.md:50`); hook range 1600 units = 20 sectors (`docs/cortex/hook-points.md`) | [D] |

---

## 2. What exists, and what is reusable

### 2.1 The Hollow Knight streamer

Files (hk-psx `game/src` unless noted; line counts are at the tree above):

| File | Lines | What it does | Coupling | Reusable as |
|---|---|---|---|---|
| `cd_stream.rs` | 599 | IRQ-driven CD transport: SetLoc, SeekL, SetMode, ReadN, PIO drain one sector per interrupt, abort at the next sector, chained to the SDK's VBlank handler by a raw write to the exception vector (`:1-9`) | `HK_CD_*` symbols, one transfer at a time | **Already lifted** into SDK `psx-cdstream` (`lib.rs`, `engine.rs` 1,027 lines, `console.rs` 687 lines, host-tested against a scripted drive). hk-psx has not adopted it: `sdk.lock.json` is at d3840528 and no `psx_cdstream` use appears in `game/src` |
| `room_residency.rs` | 537 | 5 equally sized slots, wanted list, protected upload, stored-then-decoded pipeline, victim scoring | `SLOT_COUNT = 5` | **Already lifted** into `psx-residency::slots` (674 lines) |
| `disc.rs` | 3,414 | disc groups laid out in read order (`GROUPS`, `:100`), `Prefetch` lifecycle (hint, plan, read, ready, adopt an in-flight read, wasted and hit counters, `:2712-2980`), idle-time hash of a landed prefetch (`preverify_step`, `:2830`), one `Drive` owner for room reads, music refills, XA, clips, pool (`:3089`) | scene manifest, HK groups | **Design only.** The request lifecycle and "adopt in flight" are in `psx-residency`'s engine. The idle-time verifier is a good idea not yet in the SDK |
| `preload.rs` | 231 | motion-aware plan over 2D cell topology (`plan`, `with_demand`) | all 2D side-scroller cells | **Interface only.** Cortex needs a 3D planner against the same `Wanted` list |
| `modules.rs` | 1,269 | relocatable enemy and boss code, J26/HI16/LO16/W32 fixups applied a slice per frame, pool inside one 64 KiB `lui` window, LRU with the current room pinned | build-written `HK_MODULE_*` tables, python cooker `host/code_modules.py` | **Partly lifted**: `psx_residency::overlay::Package` (SDK main only, not in the pin) and `modules.rs:295,509` already call it. Cortex does not need it unless `.text` pressure appears |
| `audio_stream.rs` | 421 | continuous ADPCM ring on one voice, fed from a 48-sector FIFO that the CD refills between room reads; half boundaries found by the SPU IRQ address latch | HK pitch, voice, premixed music cook | **Not in the SDK.** A candidate for `psx-spu`. Needed by Cortex only if the SPU-ring option is chosen (section 3.10) |
| `music.rs`, `xa_player.rs` | 840, 169 | area music policy; HK's own copy of the SDK XA player logic | HK | SDK `psx_io::cd::xa::Player` (with suspend and resume on SDK main) replaces it |
| `scene_transition.rs` | 75 | blackout, release GPU work, `cache.select(region)`, fade up | HK | **Not wanted.** This is the gate Manny's requirement 1 excludes |
| `shared/hk-cache/src/residency.rs` | 550 | 4 VRAM banks x 5 pages, 416 CLUTs | HK atlases | design only |

How HK splits the world. The unit is a Unity scene, a "group" on the disc. `docs/STREAMING.md` (stale in places: it contains the text-joining typos "contains106") says production builds use `scene_gate` residency: one 354,032 B scene arena and fixed VRAM slots belong to one scene at a time, an 84 KiB auxiliary arena holds the visibility bank, and "Every spatial view inside the active scene remains resident". The shipping slice is two scenes (Tutorial_01 with 86 views and Town with 20). Scene exits are a blackout load (`scene_transition.rs:1-75`). The later hk-finish work added a predictive prefetch of the gate's destination group into the arena tail the resident scene leaves free (`disc.rs:2700-2770`, hint from `main.rs:1325` and `frame.rs:1765`) and a 92 KiB relocating code pool (memory `hk-psx-seamless-ethos`, "EVERYTHING a room needs, including its enemies' and bosses' CODE, streams in as you approach"). Disc layout puts a scene's payloads contiguously ("5 seeks to 1", memory `hk-psx-scope-decisions-2026-09-23`).

How HK budgets. RAM: 354,032 B scene arena, 84 KiB aux, 92 KiB code pool (`docs/plans/HANDOFF-2026-10-02.md`), 98,304 B music FIFO. VRAM: fixed strips per scene (`docs/BUDGET.md`, not read). SPU: six ambience loops loaded through the empty arena at startup, then per-scene SFX banks at gates, music in an SPU ring (`docs/MUSIC.md`).

How HK handles music against data. Area music is premixed mono ADPCM streamed from the RAM FIFO into an SPU ring, so a room load never interrupts it (`audio_stream.rs:1-20`). Title and bosses use XA (and the False Knight CD-DA earlier), which "a read of any other sector stops", so any room load stops them first (`docs/MUSIC.md` 2026-10-04 section).

What HK taught about region streaming. Its earlier build streamed independent regions through five 256 KiB arenas and four VRAM banks with direction-based neighbour prefetch, and recorded "real late-destination movement holds", 646 boundary-wait ticks with a maximum of 90 in one run (`docs/STREAMING.md`, "Validation status and earlier streaming work"). It was replaced by whole-scene admission. The lesson for Cortex is that a prefetch guessed from facing direction is not a guarantee. The `psx-residency` design answers it with a cook-checked Need and Lead rule plus deadlines (section 3.4).

Verdict on "reuse HK code". The code Manny can reuse is in the SDK already. Do not copy `disc.rs` or `preload.rs`. The one real gap is the SPU ring, and it is needed only if the SPU-ring option is picked.

### 2.2 The retired grid room streamer, page pool, scheduler policy and region streamer

| Piece | Where | Size | State today | Verdict |
|---|---|---|---|---|
| `RoomStreamScheduler`, `StreamedRoomPages`, `room_graph_ring` | `e1e55509:engine/crates/psx-game-runtime/src/room_streaming.rs` | 1,690 lines | deleted 10-08 (`637cb4a4`); the pool and policy were salvaged by `358728f2` | Salvaged parts are on HEAD. `room_graph_ring` (hop BFS) is the wrong metric after unit splitting (section 3.4) |
| Page pool with generation handles | `engine/crates/psx-game-runtime/src/streaming/page_pool.rs` | 366 | on `grid/main`, used by nothing | **Duplicate** of `sdk/crates/psx-residency/src/pool.rs` (561 lines), which lifted the same code and added placement policy, locked runs, one-step compaction and fragmentation stats. Delete the engine copy when the streamer is built |
| Failure backoff, pin flags, counters | `streaming/scheduler_policy.rs` | 261 | on `grid/main`, used by nothing | **Duplicate** of `psx-residency/src/backoff.rs` and the engine's counters. Same action |
| Old disc transport | `cd_stream/hw.rs` (bare SetLoc+ReadN, ch3 DMA) | 629 | gone | do not resurrect (`old-grid-system-archaeology.md` section 5 item 11) |
| Engine CD reads on psx-cdstream | `engine/crates/psx-game-runtime/src/cd_stream.rs`, `cd_stream/ring.rs` | 1,561, 471 | on `grid/main`, used for boot assets | Keep for boot loads. A streamed unit should read straight into pool memory (`Request::new_raw` takes a destination pointer) and not through the two-window staging ring, which copies each 2 KiB sector |
| BSP region streamer | `a54e8d77:engine/crates/psx-game-runtime/src/region_stream.rs` (360), `cd_stream/region.rs` (228), `engine/crates/psx-bsp/src/pxbsp_stream.rs` (2,775) | | removed by `41ef48c7` ("Remove the brush data model, the BSP crates and the brush editor", 2026-10-17) | design only. It never got eviction (`docs/streaming-window-scheduler-plan-2026-10-10.md`: "There is no eviction yet") |
| BSP region cooker | `a54e8d77:editor/crates/psxed-project/src/brush_region/` (cuts, closure, graph, layout, gate, report, account; 5,445 lines), `stream_world.rs` (1,393 to 1,952), `stream_walk.rs` (307) | | removed | design and algorithms. `layout.rs` (Fiedler vector order plus bounded reversals, seek-class report) and `gate.rs` (the rho and pool-peak gates) are nearly graph-only and port well. `cuts.rs` and `closure.rs` are BSP-specific |
| Sweep results | `streaming-world-sweep-2026-10-09.md` | | docs only | Useful as a template and for one number: 14,336 B regions passed 50 of 50 seeds at a 369,383 B pool with a rho limit of 83.8 B per unit [M] |

### 2.3 What the SDK already provides

* `psx-cdstream` (`sdk/crates/psx-cdstream/src/lib.rs` header): PIO, one sector per interrupt, seek-first, abort at the next sector with the received count, a 4-deep priority queue (`QUEUE_DEPTH`, `engine.rs:13`), chaining of contiguous requests without a pause or seek, resume after a drive error, and an audio lease (`request_audio_lease`, `try_audio_lease`, `withdraw_audio_lease`) that stops a read at the next sector and hands the drive to CD-DA or XA code. End audio with Pause, never Stop.
* `psx-residency` (`lib.rs` header): `ResourceKey` (category plus dense index, categories WORLD_REGION, COLLISION_REGION, TEXTURE_PAGE, ARCHETYPE_PACK, AUDIO_BANK and game-defined ones, `key.rs`), `Wanted` entries with a `Class` (Demand, CombatFill, Lead, Background, Audio), walk distance and optional deadline, per-category page and entry budgets, contiguous page runs with generation handles, best-fit or first-fit placement, compaction one step at a time, eviction tiers (outside keep radius, inside hysteresis, wanted at lower priority), request adoption and promotion, failure backoff of 16 doubling to 512 windows, a bookkeeping budget per step, and the `Host` trait (`extent`, `walk_distance`, `submit`, `cancel`) that keeps it off the hardware.
* `psx-residency-sim`: runs the real `Residency` against the measured drive model (read 6,490 us per sector at 2x, seek table, `Variance::Pessimistic` doubles seeks) over a `RegionGraph` and a `Route`, and reports deadline slack and misses.
* On SDK main only: `psx-residency::overlay` (checked relocatable packages) and `psx_io::cd::xa::Player::{suspend,resume,is_suspended}` (`84a220abc`).

The engine does not use `psx-residency` anywhere yet (`grep` of `engine/`, `editor/`, `emu/` finds no use).

### 2.4 What survives on `grid/main` that matters here

The PGL format and writer (`engine/crates/psx-grid/src/format.rs`, `write.rs`, `view.rs`), `CollisionView::parse` that needs only the first `render_offset` bytes (`view.rs:207-235`), `GridCollision` that takes `&[Option<CollisionView>]` and treats a `None` room as solid (`collision/world.rs:30-45,52,72-84`), derived portals (`editor/crates/psxed-project/src/grid/cook/portals.rs`), the sequential `WORLD.PAK` placement (`cook/mod.rs:429-446`), `GridRuntime` (`grid_runtime.rs`, 947 lines), `GridBudget` and the overlay and heatmap code in `psxed-ui/src/grid_tools/`, the MCP grid tools, and XA music in the engine (`84e7512d`).

---

## 3. The design

### 3.1 Architecture

```
 DISC  ISO9660: EXE | WORLD.PAK unit chunks in layout order | texture packs | spawn/archetype packs | SFX | XA songs
   |
   v
 psx-cdstream (SDK)      IRQ, PIO, seek-first, queue depth 4, contiguous chaining, abort at next sector, audio lease
   |  Request{lba, sectors, dest = pool pointer}
   v
 psx-residency (SDK)     keys: unit, texture page, archetype, audio bank. Classes Demand | CombatFill | Lead | Background | Audio
   |  Host impl = GridHost (new, engine)
   |     extent      <- RoomRecord.pack_sector / pack_sectors
   |     walk_dist   <- Dijkstra over the portal graph from the player's position, capped at the lead radius
   v
 GridStreamer (new, engine/psx-game-runtime)      install pipeline per unit:
   land -> FNV per sector (sliced) -> header + layout hash -> RoomView -> material uploads -> link -> ADMIT
   |
   v
 GridRuntime (grid_runtime.rs)    draw set = Need set, collision table = resident slots,
                                  spawns per unit, mover state table, combat scope F, music gate

 COOKER (psxed-project)   authored rooms -> faces (derive, stitch, bands, UV, bake) -> UNIT PLANNER -> chunks
                          -> portals over units -> layout -> WORLD.PAK -> core -> cook report with gates
```

Each resource category has its own page pool and its own eviction, so textures cannot starve world data. The streamer owns policy only through `psx-residency`; nothing new decides what to load.

### 3.2 The unit

**A unit is one chunk, and one chunk is one `RoomRecord`.** This keeps every runtime path that exists (render, collision, portals, `locate`) unchanged. Authoring keeps big rooms. The cooker, not the author, cuts them.

Planner, run after all room-level face work (section 4 item 8 explains why the order matters):

1. Estimate the chunk bytes of a candidate rectangle from the cooked faces it would own: `40 B x quads + 8 B x vertices + 8 B x cells + 20 B x cells + 12 B x walls + sparse tables`, the record sizes in `format.rs` and `SECTION_RECORD_BYTES`. The Cathedral gives 125.8 B per cell [D], which is only a prior.
2. Rooms at or under the byte target stay one unit. Larger rooms are cut by recursive bisection on a lattice of 4 sectors, the size of a level-0 quadtree block (`level-tools-design.md` section 3.1), anchored to the world lattice so leaves never straddle a cut (the `origin_mod` rule of T10). Candidate cut score, in order: open cells crossing the cut line (a narrow cut is nearly free, an open field is not), faces split, byte balance, then deterministic tie-break by coordinates.
3. Hard caps per unit, cook errors with the unit named: bytes, quads plus LOD quads below 65,535 (`view.rs` check), vertices below 65,536, planes 64, and 255 quads per cell.
4. Each unit inherits `outdoor`, fog and ambient from its authored room and records `parent` (the authored room index), so fog continuity and editor names survive.
5. A planner run on a world whose rooms are all under target produces zero cuts and a byte-identical cook (the back-compat gate, the same rule the BSP design used).

Defaults [E]: target 16 sectors (32,768 B, about 260 cells at 125.8 B per cell [D], 16 x 16 sectors at S = 80, a 1,280 unit square), hard cap 32 sectors (65,536 B). Basis: one seek is 137 ms [M], 21 sectors of read time, so a unit under about 21 sectors is seek dominated; 16 sectors is 104 ms of transfer [D]. The BSP sweep found its cliff at 14,336 B for a world with different closure shape [M], so the target is a cook parameter and the sweep (section 5) decides it for this format.

**Disc groups.** Units smaller than the target (a 6 x 6 room is about 4.5 KB [D]) are read together: the layout pass (3.14) places neighbours contiguously and the streamer submits contiguous units as chained requests, one seek for the group. The residency key stays per unit.

**Atomic admission in v1.** The chunk already has collision first (`render_offset`), so `CollisionView::parse` can run on a landed prefix. v1 does not use it: a unit becomes collidable and drawable together, so no partial-landing path exists. Collision-first only softens the failure case (standing on an undrawn floor instead of hitting a wall) and the wall is already the safe answer. Keep the layout, add the early admit only if the simulator shows the prefix would remove misses.

### 3.3 World coordinates

* Entities, cameras, logic, lights and save positions are world-space `i32`. A unit's render and collision data are unit-local `i16` plus `RoomRecord.origin` (`format.rs:82-103`). This stays.
* Records carry `room: RoomIndex` but it is meaningless: the playtest cook writes `0` for every point entity (`editor/crates/psxed-project/src/playtest.rs:693-698`). No new code may use it; the only readers are `record.room != arc.room` and `r.room` passed to traces (`entities.rs:1492`, `entities/flow.rs:21`), both always equal today.
* Room membership is derived: `locate(hint, point)` (`collision/world.rs:317`). With hundreds of units the linear scan needs a coarse spatial index in the core (3.4 and section 4).
* Extent. The editor refuses any coordinate beyond +-524,272 editor units = +-32,767 runtime units (`limits.rs:41,513`), which is +-409 sectors at S = 80 [D], a world 819 sectors across and at most 670,761 cells [D], 84 MB at 125.8 B per cell if it were dense [D]. That is a disc-sized world, so the extent limit is not the binding one, but the world-space frustum test is built on it (`grid_runtime.rs:735` clamps room bounds to `i16`) and must become camera-relative before worlds approach the limit.
* Cross-unit seams. Adjacent units transform shared border vertices as `R*(origin - eye) + R*v` with separate rounding per term, so a shared lattice point may land one pixel apart [hypothesis, untested]. The gate for the planner is a lockstep frame diff of a cut room against the same room uncut (section 5.1, S2). Existing multi-room maps (the Dungeon) already carry this risk and nobody has measured it.

### 3.4 Residency: Need, Lead, pins

**One function decides both the draw set and the Need set.** Today `visible_rooms` takes the camera room plus `ACTIVE_HOPS = 2` portal hops and frustum-tests their boxes (`grid_runtime.rs:67,728-752`), and the same ring defines entity activation (`:303-318`). After splitting, hop count stops meaning distance (a 2-hop ring over 16 x 16 sector units is 32 sectors in a line and 2 sectors in a maze). Replace it with:

* `walk(p, U)`: shortest path length from the player (or camera) to unit U over the portal graph, edge weight = distance between portal centres (computable from `Portal.v`), found by a Dijkstra cut off at the lead radius. No format change.
* `D_draw`: a guaranteed draw-distance bound per unit. Fog has `fog_far` in the core (`RoomRecord`, `format.rs:82-103`), but `ViewProjection.far_world` defaults to `i32::MAX` and the grid runtime builds its projection from the default (`render/projection.rs:152-165`, `grid_runtime.rs:227-232`) [M], so an open hall is unbounded today. A streamed world needs the renderer to honour a far bound that the cook also uses. Proposal: the unit's `fog_far`, or a new `draw_far` core field when fog is off, and indoor rooms bounded by portal topology.
* `Need(p)` = units with `walk(p, U) <= D_draw` that the renderer can reach through portals this frame (the camera room, then neighbours through open portals within the distance), plus hook landings and pins below.
* `Lead(p)` = units with `walk(p, U) <= D_draw + H_lead`, requested nearest first.

If the sim shows the walk-distance Need over-fetches (it ignores occlusion), store a cook-computed conservative closure per unit (the BSP design's V(R), `streaming-design-2026-10-08.md` section 3.4) as sorted id lists in the core. Do not build it first.

Pins:
* **Home pin:** the Need set of the last checkpoint is always resident, so a respawn never waits on the drive. A respawn teleports, and a far seek is the one cost the layout cannot hide (3.14).
* **Hook edges:** the Need of a hook target within 1,600 units. A hook flight is a discrete jump of about 0.7 s (`streaming-design-2026-10-08.md` section 4.1), far shorter than a unit load. Hook candidates whose landing units are not resident must not be selectable. That is a gameplay availability rule and is open question 3.
* **Combat scope F** (3.10) and anything with an install in flight.

Sizing H_lead, from the measured drive and v_max = 470 units/s [E], for a 16-sector unit (read 104 ms [D]), `H_lead = v_max x q x T_r` with q units queued ahead:

| q | T_r = 0.241 s (neighbour class: 137 ms seek + 104 ms) | 0.414 s (far: 310 ms) | 0.724 s (far, 2x swing) |
|---|---|---|---|
| 1 | 113 u (1.4 sectors) | 195 u (2.4) | 340 u (4.3) |
| 2 | 226 u (2.8) | 389 u (4.9) | 681 u (8.5) |
| 4 | 453 u (5.7) | 778 u (9.7) | 1,361 u (17.0) |

All [D] (carried method from `streaming-design-2026-10-08.md` section 4.2, sectors at S = 80). Install CPU per unit enters as T_inst [?]. Chaining k contiguous units costs `k x 104 + 137` ms [D], so batching cuts the per-unit time.

Throughput feasibility. A crossing is feasible when `rho x v_max <= U x B_eff` with U <= 0.5 [E]. For 16-sector units B_eff = 32,768 B / 0.241 s = 136 KB/s [D], so rho <= 145 B per unit travelled, or 11.6 KB per sector walked at S = 80 [D], which is 92 cells at 125.8 B per cell [D]. For 14,336 B units the sweep's limit was 83.8 B per unit, 6.7 KB per sector, 53 cells [D]. In words: moving one sector may newly require no more than about 50 to 90 cells of data. A corridor, door or portal frontier passes easily. A wide open outdoor frontier of 100 cells or more across does not. This is the quantity that decides whether "huge and open" is feasible, which is why the cook computes it (3.13) and why question 1 matters.

Eviction and the rest come from `psx-residency`: tiers by keep radius and hysteresis, farthest then least recently used, never pinned or installing, commits at the frame boundary, handles go stale on free. Pure LRU is wrong for a line-shaped traversal and the engine does not use it.

### 3.5 Transport, scheduling, install pipeline

* `GridHost::submit` calls `psx_cdstream::submit` with `dest = pool pointer`, so sectors land at their final address. `QUEUE_DEPTH` is 4, so the streamer keeps one unit or chained group in flight and one pending. Cancel is a flag and the handler pauses at the next sector, so a Lead read that Need overtakes costs one seek, and `Request::remaining_after` continues it.
* CPU budget. A read costs 1.286 ms of PIO per sector [M], about 20% of a 33 ms frame while a read runs [D: 5.1 sectors per frame x 1.286 ms]. The tax cannot be throttled during ReadN, so the control is the duty cycle: submit Lead and Background only while the last frame fit its slot [E policy, unmeasured], and let `psx-residency`'s step budget bound bookkeeping.
* Install, sliced across frames: land, FNV-1a over each landed sector (accumulated per sector so the work per pump is bounded; the cost on the R3000 is [?], HK hashes landed prefetch in idle time with `preverify_step`, `disc.rs:2830`), header, `layout_hash` and section bounds in O(1) (`level-format-and-runtime.md` section 3.2), `RoomView::parse`, material uploads for the unit's `MATERIAL_USE`, link, then admit. There is no relocation because every offset is chunk relative.
* Chunks must be reached through `PageHandle` (generation checked), not through `&'static` slices, because `psx-residency` compacts runs by sliding them. Today `rooms` and `collisions` are leaked `&'static` slices (`grid_runtime.rs:223-224`). Any compaction or eviction must refresh the views.
* `GridCollision::new` computes its `foreign` and `planes` flags by scanning every room (`collision/world.rs:72-84`), so it cannot be rebuilt per frame at hundreds of units. Keep the flags incrementally.

### 3.6 RAM budget

Resident bytes = pool for Need, Lead, pins and F, plus the core (skeleton), plus renderer scratch for the largest unit, plus per-unit dynamic state, plus streamer code.

* Core: `RoomRecord` 40 B plus directed `Portal` 36 B each. At 400 units with 4 directed portals each that is 73,600 B [D, portal count assumed]; the BSP skeleton for 400 regions was 40,828 B [D, sweep]. Beyond a couple of thousand units the core itself needs a second paging level. State that as a limit. The core lives in `.data` today, so it is charged to headroom.
* View tables: `RoomView` is about 124 B on a 32-bit target (15 slice fat pointers plus a header reference, `view.rs:150-180`) [D]. A table indexed by unit id costs 51 KB at 400 units. Use a slot-indexed table sized to the maximum resident units, with a `slot_of[unit]` map.
* Scratch: `GridRenderer::for_rooms` takes the maxima over all rooms at start (`render/mod.rs:235-245`). With streaming the cook must write the maxima (vertices, cells, quads, LOD blocks, LOD quads) into the core, because the views are not resident at init. Splitting shrinks the maxima: the Cathedral's 27,748 B [D] would drop with units of 16 x 16 sectors.
* Pool: the cook computes the peak of bytes(Need union Lead union pins union F) over a lattice of positions (the BSP `PoolPeak`, `gate.rs`), plus a fragmentation margin that the sim measures [?]. The cook refuses a level whose peak exceeds the pool. The existing `POOL_PAGE_CAP = 512` pages (1 MiB, `cook/mod.rs:35`) is a whole-level sum capped at half of RAM and tied to no measurement, and must go (section 4).
* Headroom [?]: measure with the occupancy tool (`feat/occupancy-report`) on the grid guest before sizing anything.

Worked illustration, not a prediction: 25 resident units of 32,768 B is 819 KB, more than any headroom on record, and 9 units is 295 KB, the same order as the BSP sweep's 369,383 B pool. So the resident window is of the order of 9 to 12 units at the 16-sector target [D]. That fixes the lead radius at a few units and is why the draw-distance bound in 3.4 is not optional.

### 3.7 Textures, materials, VRAM

Today every material is resolved before the first frame and `draw` asserts it (`grid_runtime.rs:550-565,767-770`). Quads reference a level-global `u8` material (`Quad.material`, `format.rs:518-534`), 255 distinct materials per world. Each chunk already lists the materials it draws in `MATERIAL_USE` (section 8 of the chunk table, cooked, consumed by nothing).

Design:
* A unit's `MATERIAL_USE` becomes its texture requirement. Residency key TEXTURE_PAGE per asset id, with a refcount of resident units that list it. Upload goes through the existing `VramRuntime` queue and the GPU access guard (`vram/upload_queue.rs`, present-queue commits ef37b344a and 942bc0267 cited in the earlier design).
* **A unit is admitted only when its materials are resident.** Replace the global `materials_latched` with a per-unit ready bit. The existing drop-and-count of a missing material (`material_missing`) becomes a gate failure counter that must be zero.
* Shared versus duplicated. A 64 x 64 4bpp texture is about 2 KB plus a CLUT [D from `Texture`], two sectors, 13 ms of read time [D], and a unit with 8 new textures would add about 104 ms, more than a seek. So textures must not be duplicated inline into every unit chunk. Use shared texture packs placed on disc beside their heaviest user group, plus a pinned zone-wide common set. Duplicate a texture into a group only when `(copies - 1) x sectors x 6.49 ms` is below the avoided `extra seeks x 137 ms` (the earlier design's rule, section 3.3). The VRAM manager already dedupes by asset id.
* Budget. The cook counts distinct materials over every Need union Lead and fails above the VRAM slot budget (64 slots per cook today, minus the player's and enemies' atlases, fonts, UI and sky [?]). The worst case matters more than the average.
* Move texture bytes out of the EXE `ASSETS` table into disc packs. This is the largest RAM saving in the whole design and it is independent of units.

### 3.8 Entities and AI

* **Spawn records per unit.** Today the manifest holds global static arrays (`LevelGameEntityRecord` and others, `playtest/manifest.rs:1512`), and `GameEntities` spawns record `i` into slot `i`, clamped to `MAX_ENTITIES` (`entities.rs:544-552`, with a 64 bit swing mask asserted at `:1483`). A new chunk section `SPAWNS` holds a unit's records keyed by a stable global spawn id. The active entity pool takes a record by copy into a free slot. A format version bump adds the section (section 4 item 3).
* **Archetypes.** v1 keeps all models and clips resident (Manny: do not touch model residency; compression first). Per-archetype residency (ARCHETYPE_PACK keys) is later and depends on the compression result. The unit's spawn list names archetypes so the later step needs no new data.
* **Activation.** A spawn instantiates when its unit is admitted. A unit with an aggro'd entity is pinned. A dormant entity despawns when its unit leaves Need union Lead, after writing a one or two byte state (alive, dead, a few flags) to a resident table keyed by spawn id [E: 1 to 2 B per spawn, 4 KB for 2,000 spawns, [D]]. Dead stays dead until the checkpoint rule says otherwise, so streaming cannot resurrect enemies.
* **Reach.** An entity can only be in an admitted unit. A body that would step into a non-resident unit meets the solid box (3.9), and an entity whose `locate` returns `None` freezes rather than falls. Leash is 800 units = 10 sectors [D]; the unit set around a spawn within its leash is a pin while the entity is aggro'd (the combat scope).

### 3.9 Collision before render, and the unresident wall

Already built and correct: `GridCollision` takes `rooms: &[Option<CollisionView>]`, and "A missing room is treated as solid over its footprint for bodies and as non-blocking for line of sight" (`collision/world.rs:41-45`). Body scans use `scan_foreign` (`scan.rs:297-318`) and point traces test a foreign room's footprint box (`ray.rs:148-155`, `scan.rs:412-426`), so a camera arm cannot enter non-resident space either. The box comes from `RoomRecord.min_y..max_y` in the core, so it exists before the chunk does. This is invariant I3 of the earlier design and it is the fail-safe: a player who outruns the stream meets a wall, never a hole and never pop-in.

Admission rule: a unit is collidable no later than it is drawable (atomic in v1). Needs: the collision table stays slot-indexed (3.5), and `locate` must not mis-attribute a point in a non-resident unit to a resident room stacked above or below it (`locate` returns the vertically nearest room when none fits, `collision/world.rs:303-310`) [unverified edge case, test in S4].

Cost: `point_trace` loops `for room in 0..rooms.len()` per ray (`ray.rs:157,211`), `point_blocked` loops all rooms (`scan.rs:412-426`), `contents_at` loops all rooms (`world.rs:367-390`), `locate` scans in index order after the hint (`world.rs:317-360`). At the reference levels (1 to 2 rooms) that is nothing. At hundreds of units it is hundreds of footprint tests per trace, with the motor issuing up to about five traces per moving body per tick (`level-format-and-runtime.md` section 5.4) [E, unmeasured]. These loops need a resident-room list or a coarse spatial index before the unit count grows.

### 3.10 Music against streaming

Facts: any data read kills CD-DA and XA [M]; the engine already yields (`84e7512d`: "a data read still pauses the song, and it restarts from the point the head had reached"); a music-to-data switch costs about 1 s each way on silicon [M, v1.28], with an unexplained 4.4 s case [M, v2.7]; Manny's rule is music only in combat.

Options:

| Option | What it costs | Verdict |
|---|---|---|
| **A. Combat radius** (approved 10-08): fill the combat scope F, then start the song and suspend data reads until combat ends | no RAM, no new data; a window with no reads | **Recommended first.** Music only exists in combat, so everywhere else streaming is unconstrained |
| B. Combat loop resident in SPU RAM, downsampled | SPU ADPCM is 16 bytes per 28 samples [D]: 11,025 Hz mono 6,300 B/s, 30 s loop 189,000 B, 60 s 378,000 B; 22,050 Hz 30 s 378,000 B. SPU RAM is 524,288 B and Cortex occupancy is unmeasured [?] | Fallback if A's scope is too large for open fights |
| C. SPU ring fed from a RAM FIFO (HK) | 98,304 B of main RAM plus the ring code (not in the SDK) plus music cooked as ADPCM, loses CD quality | Only if the 1 s switch or the 4.4 s case proves unacceptable |
| D. XA with data interleaved into filler slots | not built in the encoder or player (`sdk/docs/XA-MUSIC.md`) | Not now |

Option A in grid terms. Scope F is every unit within walk distance `r_c` of an aggro'd enemy, `r_c = leash + v_max x T_exit`, `T_exit = T_duck + T_pause + T_first_sector + T_unit`. With T_duck 0.5 s [E], T_pause 0.123 s [M], first data sector after audio 0.945 s [M], one 16-sector unit 0.104 s [D]: `T_exit = 1.672 s` [D], so the exit margin is 786 units (9.8 sectors) and `r_c = 800 + 786 = 1,586 units = 19.8 sectors` [D]. The earlier design had used 0.414 s for the first read after audio, and silicon says 0.945 s, so its margin was short. The scope is by walk distance over portals, so a corridor fight needs a few units. The open-field worst case is a disc of radius 19.8 sectors, 1,232 cells, 155 KB at 125.8 B per cell [D], against a pool that is unmeasured [?]. That worst case is the argument for option B as a fallback.

State machine (the engine owns music policy, the streamer reports): Idle, aggro, Filling (CombatFill requests for every missing member of F, in-flight Lead and Background reads cancelled, Demand still wins), Armed (F admitted and no request in flight, ask the audio lease, start the song with the existing 60 tick fade), Quiescent (no data requests while Audio owns the drive), Releasing (existing 120 tick fade, Pause not Stop, return the lease, reads resume with a seek from the audio position). Emergencies (a player about to touch the F frontier while enemies are aggro'd, a knockback or a hook) duck the music at once, release the lease and resume reads: music yields, world integrity does not. Re-arm after the player has been back inside F by a margin for a few seconds [E] so the song does not thrash at the boundary. The engine's music gate changes from "level fully resident" to "F complete". Failure of Filling means no music for that encounter, never a stall.

Ambience that is not music (SFX banks, loops) lives in SPU RAM per zone as AUDIO_BANK keys and is loaded by Lead, as HK loads ambience per scene.

### 3.11 When the player outruns the stream

Ladder, strongest guarantee first:

1. **Cook gate (prevention).** The cook computes rho per unit crossing and the worst window, and the pool peak. A world that fails is a cook error with coordinates (3.13). Nothing below is the plan, only the safety net.
2. **Lead and deadlines.** `psx-residency` promotes a Lead request to Demand when the player nears the frontier and orders by deadline.
3. **Unresident wall.** The player stops at the edge of non-resident space and a counter increments (3.9). Gate: the counter is zero on every route; any non-zero is red and means a gate parameter (kappa, U, caps) is wrong.
4. **Governor options, only with Manny's approval** because they change feel: slow the player near the frontier, or shorten `fog_far` there. Not designed in.
5. **Authored slow constructs** (doors, lifts, long corridors) can buy time but are "disguised transitions" if they exist to hide a load (requirement 1). Allowed only as ordinary gameplay, never required, never relied on by the gate.

### 3.12 Save and restore

* The save block holds a checkpoint id, a saved room `u16`, a pose in world `i32`, and a 832-flag persistent bitset (`engine/crates/psx-game-runtime/src/save.rs:14-34`, capacity `:25-26`). Restore resolves the unit from the position through the core's spatial index and streams Need(spawn) behind the existing loading scene, then runs. A loading screen at boot or respawn is allowed.
* Checkpoint id is "logic-record index" today, an array index into global static tables. Per-unit records need stable global ids (a hash seeded from the authored node id), and the save already rejects a flag table of the wrong size, so ids must be stable across recooks that do not change the persistent set.
* 832 flags is one bit per persist-marked logic record. A world with more persistent things than that (enemy kill bits, pickups, doors) needs save format v3 or a zone-scoped layout. Count the persistent set before the world gets big.
* Mover state (door open, destructible broken) lives in a resident table keyed by mover id, not in the chunk image, because eviction drops the chunk and reinstall would reset it (`level-format-and-runtime.md` section 6.6 says broken destructibles are re-applied from a bitset; do the same for doors).

### 3.13 Editor support

Everything below extends existing surfaces: the budget strip (`GridBudget`, `psxed-project/src/grid/limits.rs:283`, shown by `psxed-ui/src/grid_tools/strip.rs:225-236`), the overlay and heatmap code (`grid_tools/overlay.rs`, T7), and the MCP grid tools (`psxed-mcp/src/grid/`, `plan.rs`).

* **Auto split.** The unit planner runs inside the editor's cook preview and draws unit rectangles as wire boxes over the 3D and top views, tinted by estimated chunk bytes, with the cut lines highlighted. No authoring step is required. Optional overrides, as hints and never requirements: force or forbid a cut plane, pin a unit resident (the format already has `ROOM_PINNED`, `format.rs:52`), and a no-split volume for boss arenas.
* **Budget overlays per unit.** Bytes against the target and cap, quads, materials (distinct), planes, spawns, and the unit's contribution to the worst Need union Lead.
* **Resident-set preview.** A draggable probe point shows `walk` distances, Need, Lead, F for a selected enemy, the hook edges, the pool bytes for that position, and the lead radius. Route mode takes a polyline (an MCP `route` already exists for playtests, `psxed-mcp/src/route.rs`) and plays it through the host simulator.
* **Cook report**, deterministic text and JSON: units (count, bytes min, median, max, fill), cuts, worst rho per crossing and worst window, pool peak and where, materials per Need union Lead, spawns per unit, layout cost with seek-class histogram, core bytes, and a short list of positions to look at.
* **MCP:** `units`, `unit_at(point)`, `need(point)`, `route_feasibility(polyline)`, `scope(enemy)`, `layout_cost`. The host simulator (`psx-residency-sim`) runs in CI after every cook of a streamed project and fails on a deadline miss, which is how logic is checked without the console.

### 3.14 Cook and disc layout

* Pipeline order, which is a constraint (section 4 item 8): authored rooms, faces (derive, stitch, bands, UV), light bake, then the unit planner, then per unit: lattice vertices, quads, LOD merge, collision tables, then portals over units, layout, `WORLD.PAK`, core, report.
* Pack placement is free. The cook writes `pack_sector` per record and today assigns it in room order (`cook/mod.rs:429-437`). Replace with a layout pass: order units to minimise the sum over portal edges of `weight x |sector distance|`, weight the open aperture length. Spectral (Fiedler vector) order plus bounded segment reversals, deterministic, as in `a54e8d77:editor/crates/psxed-project/src/brush_region/layout.rs`, which reports the share of crossings in the seek classes <=16, <=128, <=512 and >512 sectors instead of pretending to know milliseconds. Silicon seeks swing 2x run to run, so optimise sector distance, not time.
* Groups: neighbours that are usually requested together sit contiguously and are read as one chained request. Texture packs and SFX banks sit next to their heaviest user group. XA songs follow UI.PAK, after the world pack (as now, `84e7512d`), because music is only read in combat.
* Far seeks. A 100 MB world is 51,200 sectors and the disc has about 333,000 [D]. The seek model stops at 512 sectors [?]. Far seeks happen at respawns and any cross-world route, so the home pin matters, and a seek table out to the full disc belongs in the hardware tests (5.2).
* Duplicating a unit at two layout positions (one per dominant approach direction) is a later lever if the layout report shows a bad crossing.

### 3.15 Failure handling

| Case | Behaviour |
|---|---|
| Read error or INT5 | `psx-cdstream` resumes at `lba + received` within its budget; then `psx-residency` frees the pages and backs off 16 windows, doubling to 512; never abandoned |
| Checksum mismatch on a landed unit | treated as a failed read; pages freed; backoff |
| Header or layout hash refused | install fails; same path; a counter; the cook should have caught it, so this means disc damage or a cook/guest version skew |
| Pool cannot fit Need | cannot happen if the cook gate passed. If it does: evict Lead and Background first (policy), then the unresident wall and a red counter |
| Install CPU too slow | Demand is never deferred by the bookkeeping budget; Lead and Background wait |
| Drive wedge (no sector within the watchdog) | bounded waits only, never `while !ready {}`; the watchdog cancels, backs off, and the world continues on what is resident |
| Lid open, disc removed | same as a wedge; the player stops at the wall |
| Music Filling does not complete | no music for the encounter; combat is not blocked |
| Controller or INT5 right after audio | arbiter reclaim uses Pause then re-seek (the SDK lease does this); counter |
| Unknown to the emulator | the emulator is ideal hardware for the CD layer and `PSOXIDE_WEDGE_*` covers DMA and GPU only. Add CD fault injection to the sim and the `psx-cdstream` fake before trusting any of the rows above |

---

## 4. Constraints on work being built now

### 4.1 Checklist

Each line is a decision the current phases (T9 to T16, planes, movers, liquids, MCP, hardware tests) must respect. Status: **OK** holds today, **CHECK** I could not confirm, **CONFLICT** breaks today.

| # | Rule | Why | Status and evidence |
|---|---|---|---|
| 1 | **Chunk independence.** A chunk references only itself (own vertex table, own cells) and level-global tables (materials, portals, level core). No chunk index points into another chunk. New sections follow the same rule. | A unit must be installable and evictable alone. | OK. `Quad.v` is room-local, `Portal.v` is source-room local, `material` is level-global (`format.rs:113-121,518-534`). `validate_full` proves ranges. |
| 2 | **Unit boundaries on the world lattice.** Cuts fall on multiples of 4 sectors of the world lattice, the same lattice as quadtree level-0 blocks and `origin_mod` (T10). | Leaves and LOD blocks must never straddle a cut. | OK by design. T10 is unbuilt; keep `origin_mod` world-anchored, and run the LOD merge after the cut (item 9). |
| 3 | **Format has room for new sections.** The chunk header holds a fixed 15-entry section table and the layout hash covers it (`format.rs:181,228-247`). Adding SPAWNS, TEXTURES refs or a mover table is a version bump. | Spawns per unit are required (3.8). | OK, but ride the v4 bump that T10 already needs. `reserved` fields exist: `LevelHeader.reserved` (4 B), `RoomRecord.reserved` ([u8;3]), `Portal.reserved` (u16), `RoomHeader.reserved2` (4 B). |
| 4 | **No whole-world caps.** | The world is huge. | **CONFLICT**: `MAX_ROOMS = 256` (`limits.rs:24`) and `ROOM_SET_WORDS = 8` with `TooManyRooms` at runtime (`grid_runtime.rs:69,203`); `MAX_WORLD_FACES = 16,383` (`limits.rs:44`, an interim `GRD1` bound that the module comment at `limits.rs:9-13` says the cook phase would replace); `POOL_PAGE_CAP = 512` pages summed over the whole level (`cook/mod.rs:35,442`); `Quad.material` `u8` gives 255 materials world-wide; the editor extent +-409 sectors (`limits.rs:41,513`). Make the first three per-unit or resident-set caps. Accept the last two knowingly. |
| 5 | **Mover and entity ids are not room-packed.** | More than 256 units. | **CONFLICT** in design: mover id is `room << 8 \| local` (`level-format-and-runtime.md` section 6.6). Movers have no producer yet, so change it to a `u32` or a (unit, local) pair before one exists. |
| 6 | **Stable global ids** for spawns, logic, checkpoints, movers. Not array indices. | Records move to per-unit sections; saves must survive recooks. | CONFLICT in the same sense: `checkpoint_id` is a logic-record index (`save.rs:18`). Introduce stable ids when records become per-unit. |
| 7 | **No gameplay meaning for `RoomIndex`.** | Entities are world-space. | OK: always 0 (`playtest.rs:693-698`). Keep it that way and do not add readers. |
| 8 | **Derive and stitch before you cut, never after.** Derived walls (`derive.rs:202` `sector_at` is room-local, a missing neighbour makes an R3 closure wall), horizontal wall bands and neighbour stitching on shared vertical edges (T4), quadtree hanging-vertex stitching (T9), UV frames and the light bake all run on the whole authored room. The planner then assigns each face to the unit that owns its host sector. | Deriving after a cut would grow closure walls along every cut. | OK today (no planner). Must be a stated invariant in T9, T14, T16. |
| 9 | **Merges and LOD never cross a unit.** The cook merge (T14, `CELL_INDIRECT`/`MULTI`), the 4x4 distance-LOD blocks (`cook/lod.rs`) and node LOD (T15) run per final chunk. | A merged quad cannot reference two chunks. | CHECK. LOD blocks are anchored to the room origin (`lod_shift`, `RoomHeader`), not the world lattice. Run the LOD pass after the cut, or anchor to `origin_mod`. |
| 10 | **Planes live inside one unit.** The planner cuts a plane at a cut line into two planes (it is already one quad per sector, `level-format-and-runtime.md` section 9.4a). `COLPLANE_PRIMARY` and the `u8` plane id are per chunk. | Point rays test the primary copy only (`format.rs:372`). | OK: "a plane must lie inside its room's sector rectangle" (`limits.rs`, doc 9.4a caps). The splitter must preserve it. |
| 11 | **The light bake reads global inputs only**: all lights in world space and a whole-world occlusion query on the host, never per-chunk. | A border vertex must get the same colour from both sides. | OK: `CookInput.lights` is global (`cook/mod.rs:44-62`), and colour is per quad corner from the face normal, so both sides of a border agree. Keep the host-side whole-world `line_of_sight` bake (`level-format-and-runtime.md` section 4.6). |
| 12 | **Collision tables are slot-indexed, with a resident-room list or a spatial index.** `GridCollision` must support rooms appearing and disappearing, with its flags kept incrementally. | O(rooms) loops and per-construct scans break at hundreds of units. | **CONFLICT**: `rooms` leaked (`grid_runtime.rs:223-224`), `GridCollision::new` scans all rooms (`world.rs:72-84`), loops over every room in `point_trace`, `point_blocked`, `contents_at`, `locate` (see 3.9). The `None`-is-solid semantics are already right. |
| 13 | **Views are reached through handles, not `&'static` slices.** | Compaction slides runs. | CONFLICT as above (`grid_runtime.rs:223-226`). |
| 14 | **Culling is camera-relative.** Room boxes are clamped to `i16` in world space (`grid_runtime.rs:735-747`). | A world beyond +-32,767 units breaks culling silently. | **CONFLICT** (latent). Subtract the eye before the clamp. |
| 15 | **Activation and drawing are not counted in hops.** One function gives Need, entity activation and the draw set. | Hops stop meaning distance after unit splitting. | **CONFLICT**: `ACTIVE_HOPS = 2` (`grid_runtime.rs:67,309,732`). Also `LevelHeader.resident_hops` and `prefetch_hops` (`format.rs:56-80`, written as constants at `cook/mod.rs:37-39,453-454`) are hop counts nothing reads; replace them with radii in the v4 bump. |
| 16 | **A guaranteed draw-distance bound exists per unit** and the renderer honours it. | Need would be unbounded in an open room. | **CONFLICT**: `ViewProjection.far_world` defaults to `i32::MAX` and `GridRuntime` never sets it (`render/projection.rs:152-165`, `grid_runtime.rs:227-232`). |
| 17 | **Renderer scratch sizes come from the core.** | The views are not resident at init. | CONFLICT: `GridRenderer::for_rooms` reads every `RoomView` (`render/mod.rs:235-245`, called at `grid_runtime.rs:226`). Add the maxima to the core in v4. |
| 18 | **Materials per unit.** Per-unit ready bit, no global latch, no global resolve. | VRAM holds a window, not the world. | **CONFLICT**: `refresh_materials` resolves all and `draw` asserts it (`grid_runtime.rs:550-565,767-770`); `MATERIAL_USE` is cooked and unused. Keep `MATERIAL_USE` correct in every cook. |
| 19 | **Textures are not baked into the EXE.** | RAM. | CONFLICT: texture bytes come from the EXE's `ASSETS` (`grid_runtime.rs:111-125`). Needs a disc pack path, not urgent for correctness. |
| 20 | **`pack_sector` is a free permutation.** Nothing assumes it grows with room index. | Layout ordering. | OK: the blob is read by `pack_sector * 2048` (`grid_runtime.rs:210-215`). Keep the cook assert that record and pack entry agree. |
| 21 | **Entity records are per unit.** | Global static arrays do not stream. | CONFLICT: `GameEntities` is 1:1 with a global record slice and capped at 64 (`entities.rs:544-552,1483`); `visible_*_mask` queries cap at 64 bits (`grid_runtime.rs:312-340`). |
| 22 | **Mover and destructible state lives outside the chunk image.** | Eviction resets a chunk. | OK so far (no producer). State it in the movers phase. |
| 23 | **Portal derivation runs on the final unit map** and must not make a closure wall at a cut. | Units must connect. | OK in design: portals derive from lattice adjacency of rooms (`cook/portals.rs:100-160`), and closure walls come from derivation, which ran before the cut. Test it (S2). |
| 24 | **Camera and point traces treat a non-resident unit as solid.** | A camera must not enter void. | OK (`scan.rs:412-426`, `ray.rs:148-155`). Keep the tests (`collision/tests.rs`). |
| 25 | **Skeleton stays small and resident.** 40 B per unit plus 36 B per directed portal. Do not put per-cell data in the core. | The core is in `.data`. | OK. Limit about 2,000 units [E]. |
| 26 | **Hardware measurements go in the existing suite.** | Manny's rule. | Plan in 5.2. |

### 4.2 Already-built things that conflict (the short list)

In order of cost to fix later: the all-resident `GridRuntime` shape (items 12, 13, 17, 18, 21, `grid_runtime.rs:52-75,197-250`), whole-world caps (item 4), hop-based activation (item 15) and the missing draw bound (item 16), the world-space `i16` frustum test (item 14), the mover id (item 5), and entity records (item 21). None breaks a current phase. T10 (format v4) is the natural place to land items 3, 15 and 17 together, so the format bumps once.

---

## 5. Phases, tests, measurements, risks, questions

### 5.1 Phased plan

Phases S0 and S1 are small and ride the current work. S2 onward is the streaming build, only when Manny starts it.

| Phase | Work | Tests and gates |
|---|---|---|
| **S0 Keep it possible** (now, with T9 to T16) | Fix the cheap conflicts: camera-relative culling (14), per-chunk caps instead of whole-world faces (4), core carries scratch maxima, draw bound and radii in the v4 bump (3, 15, 16, 17), stable mover id type (5), `MATERIAL_USE` kept correct (18). No streaming code. | Existing grid tests green. A cook test that `MATERIAL_USE` equals the materials the quads use. A unit test that culling is unchanged when the whole world is translated by 100,000 units. |
| **S1 Measure** | Free RAM of the grid guest from a link map with the occupancy tool; bytes per cell and per unit from cook reports of the six reference levels; distinct materials per level; SPU occupancy; PIO cost under a frame load on the emulator. Nothing new on the console. | A table in the cook report. Replaces every [?] for RAM in section 1.2 and 3.6. |
| **S2 Unit planner (host only)** | The cook stage of 3.2, portals over units, layout pass, cook report with rho and pool peak. A synthetic world generator in the style of the old `stream_world.rs` (modules, corridors, an open field, stacked rooms) for tests and sweeps. | Determinism (cook twice, same bytes). A world under target cooks byte-identically to today. **Lockstep identity:** a room cut into units renders the same frames as the same room uncut, over a recorded tape, or differs by a reviewed list of seam pixels (settles the 3.3 hypothesis). `locate` and collision equal across the cut on every point of a fixture. Cap errors name the unit. Every one of the six reference levels cooks to the same bytes with the planner on and a huge target. |
| **S3 Chunks from the disc, everything resident** | `GridStreamer` loads every unit through psx-cdstream into the pool at boot, replaces the `include_bytes!` of rooms, slot-indexed views and collision, per-unit material state, spawns still global. | Frame identity against the `include_bytes!` build (the M7 lockstep pattern, `streaming-design-2026-10-08.md` section 14). `PSX_CD_STATS` shows 0 lost sectors, 0 retries. Boot time and heap high water reported. |
| **S4 Residency with eviction** | `GridHost`, distance Need and Lead, `psx-residency` with a pool smaller than the world, host sim in CI, emulator boot of an oversized generated world. | Sim and emulator agree on deadline slack within a stated tolerance. Zero Need misses and zero unresident-wall hits on every route at 1.25 x v_max, including the 2x seek corner. Backtrack at a boundary does not thrash. Eviction never frees a pinned or installing unit. `PSOXIDE_WEDGE_*` and the cdstream fake inject read failures: backoff, no hang. A point in a non-resident unit stacked over a resident room does not locate into the resident room. |
| **S5 Textures per unit** | Disc texture packs, TEXTURE_PAGE keys, per-unit ready bit, VRAM upload through the guard. | A route with zero `material_missing`. A cook test that distinct materials over any Need union Lead fit the slot budget. The upload queue never overflows (the BSP survey saw "128 upload-queue-full" once, unconfirmed since). |
| **S6 Spawns and state per unit** | SPAWNS section (v5), dormant state table, active entity pool, stable ids, mover state table. | Kill an enemy, leave, return: it stays dead. No enemy ever stands in a non-resident unit. Save and load restores the same set. |
| **S7 Combat scope and music** | Scope F, the state machine of 3.10, XA yield integration, engine gate moved to "F complete". | Scripted tapes: aggro, music starts only after F is complete; no data request while Audio owns the drive; sprint out through the leash; knockback to the frontier; fill-failure injection (no music, no hang). The existing music tests stay green. |
| **S8 Editor** | Overlays, preview probe, report, MCP tools, hints. | UI tests in the existing harness. The preview agrees with the host sim on pool peak. |
| **S9 Console stress world and sign-off** | The generated world at full size, built through the production path. | Tapes plus a free run on the console, no loading screen, zero Need misses, zero wall hits, slack above target, measured seek table inside the model band. Needs Manny's console and the hardware-test additions below. |

### 5.2 Console measurements needed

All go into the existing hardware-tests suite, in the single linear run, never a new probe disc (memory `extend-hardware-tests-not-new-probes`, `no-console-ab-discs`). Manny will not burn until the suite is complete.

1. **Seek table beyond 512 sectors:** 1k, 4k, 16k, 64k, 200k, 330k sectors, forward and backward, repeated for the 2x swing. Gives the far-seek and respawn cost. (Overlaps the deferred v2.8 "seek lengths" item.)
2. **Production-pattern reads:** unit-sized reads (7, 16 and 32 sectors) at the layout's contiguous, near and far gaps through `psx-cdstream`, with a GPU draw load and pad polling running, recording frame-time distribution, PIO cost per sector under load, handler peak, discarded sectors.
3. **XA after a data read:** decode the v2.6 step if it has been run, repeat on the current disc if not; add the first-sector and resume times after a 16-sector read.
4. **Repeat the 4.4 s read after Pause plus 15 s** (v2.7 open finding) with several idle gaps, because it decides whether option A's exit margin is safe.
5. **VRAM uploads during gameplay:** cost per 64 x 64 4bpp, 8bpp and 15bpp upload through the present-queue guard while a frame is drawing, and whether texture-cache state after an upload perturbs the next frame.
6. **FNV verify cost:** cycles per 2 KiB sector for byte-wise FNV-1a and for a word-wise variant on the R3000 (needed for T_inst; today only the emulator could answer).
7. **SPU RAM occupancy** at runtime and an SPU ADPCM loop that survives a data read (option B feasibility), inside the existing audio steps with the end-of-run silence rule.
8. **Compaction cost:** time to slide a page run of 16 sectors in RAM (the moving of 32 KB, needed for the pool's compaction budget).

### 5.3 Risks

1. **The open-field frontier.** rho is bytes per cell crossed; an outdoor frontier 100 cells wide at 125.8 B per cell is 12.6 KB per sector walked [D], over the 11.6 KB limit for 16-sector units [D]. The cure is visual (fog, draw distance) or smaller cells and needs approval. Question 1.
2. **Pool smaller than the window.** The resident window is 9 to 12 units at the 16-sector target [D] and the pool is unmeasured. If headroom is small the draw distance must shrink.
3. **PIO tax on a 30 fps frame.** About 20% of the frame while a read runs [D]. The grid frame is already near 920k mean cycles with a p90 over the slot (`level-format-and-runtime.md` section 12) [M]. Streaming reads will cost frames unless duty-cycled. S4 measures it.
4. **Far seeks.** Unmeasured beyond 512 sectors [?], on a disc up to 333,000 sectors [D].
5. **Seam pixels between units** (3.3 hypothesis).
6. **Collision cost at hundreds of units** (3.9), unmeasured.
7. **Music handoff variance:** 1 s typical, 4.4 s seen once [M]. Option A's margin assumes the typical case.
8. **Cook time.** The planner and the whole-world bake on a large world. The bake runs a line-of-sight per lit corner on the host, whole-world; for a world 100 times the Cathedral this needs a spatial index in the host collision, which `GridCollision` has per room only.
9. **Skeleton scaling** past about 2,000 units (3.6).
10. **Save flags** (832) may be too few (3.12).
11. **Two page pools and three CD transports in the tree** (section 6) invite the next change to fix the wrong copy.

### 5.4 Open questions for Manny, with recommendations

1. **How big, and how open?** Roughly how many sectors across and how many units or MB on the disc, and is it TR-style corridors and halls or large open outdoors? It decides unit size, whether the rho limit holds, and whether option A or B for music. *Recommendation:* design for the extent limit as built (819 x 819 sectors), indoor-first with open areas bounded by fog, and treat any outdoor area wider than about 90 cells at the frontier as a cook warning.
2. **What happens when the guarantee still fails?** *Recommendation:* the existing collision wall plus a red counter (already built, 3.9). No speed governor unless you ask.
3. **Hook targets in non-resident units: not selectable?** *Recommendation:* yes, not selectable, a small availability change that only appears when the Lead failed.
4. **Slow doors, lifts and corridors as optional gameplay.** They help the stream but would be disguised transitions if built to hide a load. *Recommendation:* allowed as ordinary design, never used by the cook gate.
5. **Music.** *Recommendation:* option A first (combat radius with XA yield), then measure the exit margin on the console (5.2 items 3 and 4), with option B (SPU-resident combat loop, 30 s at 11,025 Hz is 189 KB [D]) as the fallback and option C only if both fail.
6. **Model and animation compression:** did the pass you pulled forward on 10-08 land, and by how much? Per-archetype streaming (3.8) waits on it. *Recommendation:* keep archetypes resident in v1.
7. **Textures:** accept 255 world-wide materials and the 64 VRAM slots for now? *Recommendation:* yes, with a pinned zone-common texture set and per-zone sets streamed; revisit when a level needs more.
8. **Unit planner control:** automatic with hints only (`pin`, `force cut`, `no split`)? *Recommendation:* yes, matching your "the cooker splits automatically".
9. **hk-psx adoption:** hk-psx and cs-psx do not use `psx-cdstream` or `psx-residency` yet. *Recommendation:* do not block Cortex on it; move them in their own change per the "harmonise onto the SDK" rule.
10. **Console time:** the additions in 5.2 need a hardware-test version and a burn slot after the suite is otherwise complete. *Recommendation:* one batch, after S2 shows which numbers the cook gate actually depends on.

---

## 6. Noticed, not touched

* Two page pools in the tree: `psx-game-runtime/src/streaming/page_pool.rs` (366 lines) and `sdk/crates/psx-residency/src/pool.rs` (561). Same ancestry. Delete the engine copy when the streamer lands.
* Three CD transports in the ecosystem: `psx-cdstream`, hk-psx `cd_stream.rs` (599 lines), cs-psx `cd_irq.rs`, plus the engine ring on top of the first. hk-psx has not adopted the SDK crate.
* `POOL_PAGE_CAP = 512` is 1 MiB for a console with 2 MB, tied to no measurement (`cook/mod.rs:33-35`).
* `LevelHeader.pool_pages` is the whole level and `resident_hops` and `prefetch_hops` are written but never read.
* `MAX_WORLD_FACES = u16::MAX / 4` is a leftover of the `GRD1` single-pack limit (`limits.rs:44`, module comment `limits.rs:9-13` says it was to be replaced) and is now a whole-world limit with no basis.
* `level-format-and-runtime.md` section 5.3 says `locate` tries the hint, then the hint's portal destinations, then a scan. The code tries the hint then every room in index order (`collision/world.rs:317-360`).
* The seek table in `psx-residency-sim` clamps at 512 sectors and says so; the cook gate and the layout cost therefore underprice far seeks.
* `hk-psx/docs/STREAMING.md` is partly stale (it states scene-gate as production, the prefetch and code streaming landed later) and contains joined-word typos.
* `docs/grid/level-tools-design.md` section 13 lists T16 as "design-level"; this document is that spec, so T16 should point here.

## Decisions (2026-10-17)

- World shape (Manny): a mix of indoor and outdoor sections, like Dark Souls. Interiors stay cheap; outdoor areas need wide frontiers, so the frontier budget warning, fog and far-vista settings, and node LOD (level-tools T15) are on the streaming critical path rather than optional.
- Animation compression has landed: commit 949ad78a ("Pack animation clips into keyed tracks inside a vertex-space error budget", in origin/main and grid/main) took Graybox Reach resident clips from 487,872 B to 305,976 B. No separate mesh compression commit was found. Archetypes stay resident in v1.
- Taken on the recommendations above: automatic unit planner with hints; keep the collision wall plus a red counter when the guarantee fails, no speed governor; hook targets in non-resident units unselectable; slow doors and lifts allowed as ordinary design only; combat-radius music with XA yield first, SPU loop fallback; 255 world materials and 64 VRAM slots for now with a pinned common set; hk-psx and cs-psx moved to the SDK transport separately; console measurements batched after the first host-side planner phase.
- The CONFLICT items in section 4 are fixed as their own phase right after the natural-terrain N1 to N4 block, together with the format bump T10 needs.

## Stream-ready phase: result (2026-10-17, branch `grid/stream-ready`)

The CONFLICT items of section 4, fixed together with the format bump (version 4, documented in `level-format-and-runtime.md` section 9.4c; T10 extends it). No streaming code exists; rooms are still all loaded at boot, through the new resident-set API.

| Item | Resolution |
|---|---|
| 4 caps | `MAX_ROOMS` 256 became the format's room id limit (65,534); `ROOM_SET_WORDS` and `TooManyRooms` are gone from the guest; `MAX_WORLD_FACES` became the per-room `MAX_ROOM_FACES`; `POOL_PAGE_CAP` became `MAX_ROOM_PAGES` (255 per room, `max_room_pages` u8 in the core) plus an advisory above 512 total pages. The 255 world materials and the editor extent stay, knowingly; `Quad.material` now indexes the room's own `MATERIAL_USE`, so the table can become per-unit later. |
| 5 mover id | `Mover.stable` ([u16; 2], from `MoverId::from_name`), not `room << 8 | local`. |
| 6 stable ids | `psx_level::StableId` (FNV-1a of the authored node or name): spawns and logic records (`GAME_ENTITY_IDS`, `LOGIC_IDS`, collision is a cook error), checkpoints from their authored id, saves v3 (checkpoint stable id and a world pose, no indices; v2 loads). Entity slots carry a spawn id and a dormant table keeps kills across a room leaving. |
| 12, 13 collision and views | `ResidentRooms` (slot storage, add/remove, epoch); `GridCollision` is a few-word view built from it. The world lattice in the core replaces every loop over all rooms in `scan_column`, `point_blocked`, `locate`, `contents_at` and the ray walks. `GridCollision::new` is gone; nothing scans rooms at construction; flags are kept incrementally. Non-resident rooms are still solid boxes (tests kept, plus lattice-versus-all-rooms equivalence and load/evict tests). |
| 14 frustum | `FrustumPlanes::at_eye` plus `cull_world_aabb`: eye-relative boxes, no world `i16` clamp; invariant under a 100,000 unit translation (test). |
| 15 activation | Walk distance over the portal graph (`WalkSet`, `RoomActivity`) from the camera (draw set) and the player and camera (activation), radii in sectors from the core; `resident_hops` and `prefetch_hops` replaced by `draw_far`, `active_radius`, `lead_radius`. |
| 16 draw bound | Per-level `GridWorld.stream` (defaults 48 / 32 / 8 sectors, MCP `grid_set_stream`); the room-level bound is the walk radius. `far_world` is left off because it only fades colour. |
| 17 scratch | `GridRenderer::with_maxima` from the core; an oversize room is refused and counted. |
| 18 materials | Per-room binding and ready flag; no global latch or resolve; `MATERIAL_USE` is now what the room's quads index (so it cannot disagree with them). |
| 21 entities | Slot ownership with stable ids (`spawn_slot`, `release_slot`); the active set (`MAX_ENTITIES`) is a pool size, not a world limit. The playtest still spawns global record `i` into slot `i` at boot, and the entity queries keep their 64 bit masks (the pool size). |

Left for the streaming phase: the disc streamer and `GridHost`, the unit planner, per-unit spawn sections (`SPAWNS`) and the playtest reading entities from them, texture packs out of the EXE, material ids per unit, the combat scope and music gate, and a lattice paged beyond about 2,000 rooms.

## Sim-state gate: result (2026-10-17, branch `grid/meshes`)

The stream-ready phase could not prove frame identity on multi-room levels (matching frames: cave 10 of 15, dungeon 11 of 15, a Cathedral rebuild 6 of 15), blamed on frame pacing but unproven. Frames are the wrong evidence: `--stop-at-poll` ends at the first display flip after a poll, so the frame shows whichever render was in flight, and a faster or slower build lands on a different tick. The gate compares the simulation instead.

**The tool** (`tools/grid-simgate/`, reusable by any later phase). The guest feature `sim-trace` (which implies `camera-pose-log`) prints one `sim-state` line per fixed tick: tick, player position, yaw, vertical speed, grounded, room, camera eye and angles, an FNV hash over every entity slot's position and state, and the first three entity positions. `simgate.sh disc LABEL PROJECT` builds a disc with it, `run LABEL TAPE STOP_POLL` replays a poll-bound tape headlessly and keeps the lines, `cmp A B` compares two runs tick for tick from tick 1100 (the tapes issue no gameplay input before 1200; the spawn drop before that moves by a tick with load time and is reported, not judged). `mktape.rs` writes tapes (`L:from:to:x:y` left stick, `R:` right stick, `B:` buttons). To compare two commits, apply the `sim-trace` commits to each (`git cherry-pick`), build each tree's frontend and discs with the same script, and run both discs with one frontend (the emulator core does not differ).

**The finding.** On the four reference levels (Cathedral, Dungeon, Cave, starter, route tape: walk, then turn) and on a new three-room fixture (`editor/projects/grid-gate`: rooms joined by ghosted openings, the last with a one click step; tapes walk east through all three rooms, and east then back) the sim state of `d4f31d95` (grid/main before stream-ready) and `grid/stream-ready` is identical on every judged tick: 1,200, 1,197, 1,198, 1,198, 1,048 and 1,248 ticks, zero different. The player reaches x 1,428 of a 1,440 unit fixture, so the two portals and the step are crossed.

The first run on the fixture diverged, in the camera only: from tick 1,612 the eye differed by a tick's worth of pull-in while the player, entities, room and every other field matched. Cause: the third-person camera throttles its spring-arm solve (`collision_solve_interval`) and its idle lift check (`LIFT_IDLE_CHECK_PERIOD`) with phase counters that start at the camera's first update. That update is the first gameplay tick, which moves with load time (the new build loaded the fixture one tick slower: first tick 124 against 123), so the camera solves on different absolute ticks. The Cathedral, Dungeon, Cave and starter start on the same tick in both builds, which is why they matched from the start. Proof: with the phases offset by one in the new build, the fixture matched on every tick. It is not a stream-ready regression and the player never depends on it, but it makes camera poses (and so frames) depend on load time. Under `sim-trace` only, `ThirdPersonCameraState::pin_throttle_phase` pins both phases to the absolute tick so two builds agree whatever their load time; shipping builds keep the start-relative phases (a unit test covers the pin). Smell for a separate issue: the camera's cadence should be anchored to the tick, not to its own first update.

Frame mismatches between builds are therefore presentation phase (which render a flip shows, and the camera throttle on a differing start tick), not a simulation difference.
