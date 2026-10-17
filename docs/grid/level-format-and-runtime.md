# Grid level format and runtime (design, 2026-10-16)

Status (2026-10-17): phases 0 to 4 and the container half of phase 5 are built on `grid/format-runtime`. The format (`engine/crates/psx-grid/src/format.rs`), the cook (`editor/crates/psxed-project/src/grid/cook/`), the renderer (`psx-grid/src/render/`), the native collision (`psx-grid/src/collision/`) and `GridRuntime` (`engine/examples/editor-playtest/src/grid_runtime.rs`) exist and Play uses them for grid worlds with no PXBSP data at all. Differences from the text below, all deliberate:

* `Quad.d` is the plane constant of the normal dotted with a corner shifted right by 7, as designed. Quads, vertices and cells are exactly the designed records (40, 8, 8 bytes).
* Room chunks sit in one resident blob laid out like `WORLD.PAK` (each chunk on a 2048-byte boundary, `pack_sector` the sector offset inside the blob). The guest reads the blob in place; streaming the chunks off the disc and the page-pool latches are not built, so `pool_pages` is the whole level and the cook refuses a level above 512 pages.
* Portals are derived (side by side and stacked) and drive activation (`ACTIVE_HOPS` rings). The portal-clipper visibility traversal of 7.2 is not ported: rooms in the camera room's ring that touch the frustum are drawn and the per-cell cull does the rest.
* The distance LOD of 4.8 is built: the cook merges horizontal quads of 4x4 blocks, the renderer draws them under the material's texture window beyond `LOD_FAR_CELLS` sectors.
* Movers, liquids and free planes have their records in the format and no producer yet (the grid document has no door, liquid or plane objects).

Original proposal, kept as written. It replaces three things on `grid/main`: the prototype `GRD1` render pack, the thin-collision-brush derivation that feeds PXBSP (`editor/crates/psxed-project/src/brush_grid/collision.rs`), and the roughly 20 `BspRuntime` methods the playtest guest still calls for everything that is not drawing.

How to read the numbers. Every figure carries one of four tags.

* **[measured]** came out of a headless emulator run or a file on disk, with the source named.
* **[derived]** is arithmetic on measured inputs or on struct sizes in the code.
* **[estimate]** is my reasoning with no run behind it.
* **[hypothesis]** is a claim I expect to be true and have not tested; each one names the experiment that settles it.

Citations are `path:line` in this tree, or `sha:path` for history (read with `git -C /Users/ebonura/Desktop/repos/PSoXide-editor-wt/open-hall show <sha>:<path>`).

---

## 0. Summary

1. One new format, **PGL** ("PSX grid level"), in two parts. A small resident **level core** (room directory, portal graph, material table) and one **room chunk per room** stored in the existing `WORLD.PAK` (`PSOXWPAK`, 2048-byte sector aligned, `crates/psx-iso/src/lib.rs:76-82,290`). The core is `include_bytes!`'d by the manifest; chunks stream.
2. A room chunk is a single zero-copy blob: collision block first (so a room can be walked before it can be drawn), render block second. Every record is a fixed-size little-endian struct cast in place, as the prototype already does (`engine/crates/psx-bsp/src/render/grid.rs:166-190`). No strings, no pointers, no relocation.
3. Rendering keeps the prototype's algorithm (row-span cell cull, backface-before-projection, shared lattice vertices, GTE-batched projection, one packet per quad). The record shrinks 44 to 40 bytes, the pack header becomes typed, macro cells go away, rooms get their own coordinate frame, and a distance LOD for horizontal surfaces and a free-plane facility are added.
4. Collision becomes native and O(1) per sector: 20-byte dense sector records plus sparse side tables, behind the **existing** `psx_engine::CollisionTraceProvider` contract (`engine/crates/psx-engine/src/collision_query.rs:110`). The character motor, the third-person camera, melee, projectiles, hook traces and enemy movement already consume only that trait, so none of them change. The old grid backends (`22894367`, -4668 lines) are **not** restored.
5. BSP-provided services are replaced one by one (section 6): activation by portal-graph rooms, line of sight by a cell DDA, liquids by rectangle records, doors and destructibles by rigid vertex-set movers, sky by a per-cell flag.
6. Units: the runtime unit is the engine unit (editor unit / 16). The world scale must give an integer sector size. The default 21/16 gives 84 runtime units per sector.
7. Prototype Cathedral pack is 98 KB for rendering only **[measured: 1718 faces, 1883 shared vertices, 98 KB, memory `cortex-grid-engine-preference-2026-10-16`]**. The new render block for the same counts is 90.8 KB **[derived, section 7.4]** and the collision block adds 17.2 KB of dense sector records, plus sparse wall, ext and plane tables whose size depends on the level and which I could not measure **[derived for the dense part]**.

Decisions I need from Manny are collected in section 13. The two that change the numbers most are the default world scale (section 2.3) and the baked-light encoding (section 4.6).

---

## 1. What the prototype tells us

Inputs for every size and cost claim below.

| Fact | Value | Source |
|---|---|---|
| Cathedral world render cost | 237k to 280k cycles/frame | **[measured]**, memory `cortex-grid-engine-preference-2026-10-16` (grid bands 0,0 = 237k, grid final = 280k) |
| Per visible quad | about 800 cycles: emit about 420 per packet, project about 69 per vertex, select about 170 per quad, OT link about 150 per packet | **[measured]**, same note and task brief |
| Cathedral pack | 1718 faces, 1883 shared vertices, 98 KB | **[measured]**; recomputes to 98.1 KB from `grid.rs:119-122` sizes (8 B vertex, 8 B cell, 8 B macro, 44 B quad) **[derived]** |
| Cathedral shape | one room, 33x26 sectors, 6 sectors tall, 0 portals, nearly no ceiling | **[measured]**, memory `bonnie32-levels-ps1-budget-2026-10-16` |
| Frustum tiles per view | p50 397, p90 928, max 1467 | **[measured by analyzer, not by the engine]**, same note |
| Same with a 16-sector cut plus idealised 2x2 merge beyond 6 sectors | p50 147, p90 186, max 243 | analyzer **[estimate for the engine]**, same note |
| Player (Reach character) | about 470k cycles/frame | **[measured]** |
| Sky | about 58k cycles/frame | **[measured]**, memory note above |
| End-to-end | grid final 25.9 fps, mean 920k, p90 1.33M; grid bands 0,0 26.5 fps | **[measured]**, memory note above |
| 30 fps slot | 33,868,800 / 30 = 1,128,960 cycles | **[derived]** from the 33.8688 MHz CPU clock |
| Near-plane and depth-band split path | the hot spot: 8.8 to 25.4 split triangles per frame on four other maps, about 9.2k cycles each, 59% to 82% of room cost is the emit phase | **[measured counters, derived cost]**, section 4.7 |
| Sky and player in those four runs | sky 82k to 101k, player 469k to 660k cycles/frame | **[measured]**, `open-hall/build/oh-runs/grid/{cv,dg,ws}-grid-run.txt` |
| Old grid runtime for contrast | 1.6k to 9k cycles per considered surface, no shared vertices | **[measured]**, `docs/grid/old-grid-system-archaeology.md` sections 2.7 and 5; do not resurrect |
| Portal traversal, old system | 8,512 cycles/frame, 0.6% of render | **[measured]**, `docs/grid/old-grid-system-archaeology.md` section 2.4 (different era and cycle model; order of magnitude only) |

What the numbers say. At about 800 cycles per visible quad, Cathedral p90 (928 tiles) would cost about 742k cycles of world alone **[derived]**, which does not fit beside a 470k character in a 1.13M slot. The cut-plus-merge figures (p90 186, max 243) cost 149k and 194k **[derived]**. So distance LOD for horizontal surfaces (section 4.8) is not an optional extra for open halls. It is the difference between fitting and not.

The prototype's structure is sound and is kept: `select_cells` row spans (`grid.rs:466`), `select_quads` backface test with axis-plane shortcut (`:568`), `project_corners` through a per-frame stamp table (`:643`), `emit` (`:711`), `robust_quad` for near-plane and GPU-limit cases (`:931`). What changes is the data layout around them.

---

## 2. Units, scale and the editor-to-runtime mapping

### 2.1 The runtime unit

The runtime unit is the **engine unit**, the same unit as `RoomPoint`, `CharacterMotorConfig` radius and height, the camera, and every cooked entity record. The existing cook already divides editor lengths by 16 (`editor/crates/psxed-project/src/units.rs`, `WORLD_UNIT_DIVISOR = 16`; `grid/limits.rs:EDITOR_UNITS_PER_ENGINE_UNIT`). Keeping that unit means the motor constants (`STEP_UP_HEIGHT = 40`, `STEP_DOWN_HEIGHT = 40`, walkable normal 2867 in Q12: `character_motor.rs:27,38,44`) and every entity position stay valid with no second conversion.

### 2.2 The mapping

The grid document is integer grid units, 1024 per sector, with a click of 256 and a fine snap of 64 (`grid/units.rs:6-12`). The world scale is `num/den` (`GridScale`, `grid/units.rs:28`) and maps grid to **editor** units. Grid to runtime is therefore

```
rt(v) = round_half_away_from_zero( v * num / (16 * den) )     // cook only, i64 intermediate
S     = rt(1024) = 64 * num / den                              // runtime units per sector
```

Rules the cook enforces (hard errors, in `limits::validate`, one place per decision D6):

* `64 * num` must be divisible by `den`, so S is an integer. Editor sector units are then exactly `16 * S`, so editor and runtime agree on every sector boundary. 21/16 gives S = 84, 9/16 gives S = 36, 7/8 gives S = 56 (the 36 matches the bonnie importer note in `docs/grid/psoxide-editor-architecture.md` line 5).
* `2 <= S <= 255`, so a local offset inside a sector fits `u8` (section 2.4).
* `width * S` and `depth * S` fit `i16` (so rooms are at most about 128 sectors per side at S = 255, far more at S = 84). Room-local vertex coordinates are `i16`.
* Room origins are whole sectors, so every room sits on the one shared lattice (already true: `GridRoom::origin` in whole sectors, `grid/room.rs:55-60`).

One consequence to know about. With S = 84 a click is 21 runtime units and the fine snap of 64 grid units is 5.25. Heights are rounded to whole runtime units at cook, so fine-snapped heights lose at most half a unit, under 1% of the 64-unit character height **[derived: 0.5 / 64]**. Collision uses the rounded values, so what is drawn is what is stood on. Section 2.3 proposes a scale that makes fine snap exact.

### 2.3 Scale choice (decision for Manny)

D1 chose 21/16 so a 1024-editor-unit Cortex character (64 runtime units) stands 0.76 of a sector tall against Lara's 762/1024 = 0.74 (`docs/grid/plan.md` section 3, D1). That scale has two soft spots: S = 84 is not a power of two (needs the reciprocal in 2.4, a few multiplies per query), and the fine snap is not integral.

5/4 gives S = 80, click 20, fine 5: every snap lands on a whole runtime unit, and the character is 0.80 of a sector. 4/3 is not allowed (S = 85.33). I recommend 5/4 for new worlds and leaving 21/16 supported. This is a one-field change in `GridScale::DEFAULT`; it is flagged, not decided.

Related and larger: `STEP_UP_HEIGHT = 40` is a constant in the motor, not derived from the world. At S = 84 that is just under two clicks (42), so a one-click riser steps and a two-click riser blocks, which is the Tomb Raider feel. At S = 36 (scale 9/16) the same constant is 4.4 clicks. The step height should become a `CharacterMotorConfig` field defaulting to the current constant (quake-psx and hl-psx pin an editor revision in their `components.lock.json`, so a change on `grid/main` does not reach them until they bump).

### 2.4 Cell lookup without a divide

`x_local / S` happens on every collision sample. A divide is about 36 cycles on the R3000 (general knowledge, not measured here) and the engine already avoids Q12 divides in hot paths (the old `circle_overlaps_segment` in `e1e55509:engine/crates/psx-engine/src/character_motor.rs` rejects early "before the closest-point projection, whose Q12 divide is comparatively expensive"). The runtime derives a reciprocal once at level load (one divide, off the hot path) and then looks cells up with a multiply and a shift:

```
inv  = ceil(2^k / S)                                   // k chosen by the same function in cook and runtime
cell = (x_local as u32 * inv) >> k                     // x_local in 0 .. max_room_extent
```

A 16-bit reciprocal is **not** exact over a whole room. Counterexample: S = 84, `inv = 781`, `x = 32000`: `32000 * 781 >> 16` is 381 but `32000 / 84` is 380. The standard bound is that `ceil(2^k / S)` is exact for all `x` with `x * (S - 1) < 2^k`, so `k` must grow with the room extent, and `x * inv` must still fit `u32`. `k` is the smallest value in 16 to 24 that satisfies both for the largest room extent in the level (known from the room directory). The cook runs the exhaustive check (`0 ..= extent` against integer division) and refuses to write a level for which no `k` works. Nothing about `k` or `inv` is stored in the file; both sides compute them from `S` and the room extents, so they cannot drift.

### 2.5 One function for height, shared by editor and runtime

Today the height of a floor at a point is implemented twice: `GridWorld::floor_height_at` (`grid/world.rs:88-130`, i64 barycentric) and, in history, `floor_sample.rs::height_at_local` (`e1e55509:engine/crates/psx-engine/src/floor_sample.rs`). They agree by luck. The new `psx-grid` crate (section 11, phase 0) owns **one** `height_at_local(corners, split, lx, lz, S)`, `no_std`, and the editor, the cook's light bake and the runtime all call it.

Interpolation weight, exact at both ends so neighbouring cells agree on a shared edge: with `inv24 = ceil(2^24 / S)` and `amount` in `0 ..= S`, `w12 = min(4096, (amount * inv24) >> 12)`. Since `amount * inv24 <= 2^24 + S`, the product fits `u32` and `amount = S` gives exactly 4096. Each term is `(delta * w12) >> 12` with `delta` a corner difference (`i16` corners, so at most 65535 in magnitude, and the cook rejects cells whose corner deltas exceed 4096 units, about 48 sectors of height difference, never seen in practice). Then `delta * w12 < 2^13 * 2^12 = 2^25` fits `i32`. Error against exact rational arithmetic is at most one runtime unit for deltas up to 4096 (the Q12 weight quantises to 1/4096 of the delta). A test compares the function against an `i64` reference for every delta in [-4096, 4096] at a sampled set of amounts and every amount for small deltas, and asserts the one-unit bound and the exact endpoints.

---

## 3. Container and on-disc layout

### 3.1 Files

| File | Where it lives | Size class | Lifetime |
|---|---|---|---|
| Level core `grid_level.pgl` | `include_bytes!` in the cooked manifest, so in the EXE `.data` | a few KB [estimate] | always resident |
| Room chunk `room_NNN.pgr` | one chunk per room in `WORLD.PAK`, chunk id = room index | tens of KB to about 110 KB [derived for Cathedral] | streamed, pinned or evicted |
| Name sidecar `grid_level.pgn` | beside the generated files, **not** on the disc | small | editor and tooling only |

`WORLD.PAK` already exists and is already the streaming container: 28-byte header, 24-byte entry per chunk (id, sector offset, sector count, byte size, FNV-1a checksum), each chunk on a 2048-byte sector (`crates/psx-iso/src/lib.rs:285-318,412-435`). The embedded Play path already builds it from `generated/stream_chunks/room_NNN.*` files (`emu/crates/frontend/src/playtest_disc.rs:293-330`). The manifest already carries `LevelWorldPackEntryRecord` (room, sector_offset, sector_count, byte_size, checksum; `engine/crates/psx-level/src/lib.rs:668`). The grid format reuses all of that. Nothing on the disc-building side needs to be invented. `RoomRecord.pack_sector` duplicates the entry for convenience and the cook asserts the two agree.

Everything in the format is **string-free**: names, texture paths and node names are cook-time only. Debug names go to the sidecar, keyed by room index and cell.

### 3.2 General rules

* Little endian. `const _: () = assert!(cfg!(target_endian = "little"))` as in the prototype (`grid.rs:68`).
* Every table starts on a 4-byte boundary relative to the chunk start. Every struct record size is a multiple of 4 (so a `u32` field is aligned wherever the table is); the two plain index lists (`MOVER_VERTS` as `u16`, `MATERIAL_USE` as `u8`) only need their table start aligned. The page pool lands the chunk on a word boundary, so chunk-relative alignment is RAM alignment (`engine/crates/psx-game-runtime/src/streaming/page_pool.rs`, contiguous 2 KiB pages).
* Every record is `#[repr(C)]`, plain integers, no invalid bit patterns, with `const _: () = assert!(size_of::<T>() == N)` for each. A `layout_hash` in the level header is a hash of those sizes and field offsets, so a reader built against a different layout refuses the file instead of misreading it.
* Versioning: `version` increments on any layout change; `min_reader` is the oldest reader that can still understand the file. The runtime accepts `min_reader <= its version <= version`. A golden-file test pins the bytes of a small fixture level for each shipped version.
* Validation: the cook proves every index in range. The guest checks the header, section bounds and `layout_hash` in O(1) when a chunk lands, and the pack's FNV-1a checksum covers integrity. A `validate_full()` that walks every index exists for the host, the editor and debug guests (the prototype's `GridPack::parse` does this walk on every load, `grid.rs:233-262`; in the guest that costs a pass over all quads at room install, so I make it a debug and host check). This is a deliberate trade and is listed in section 13.

### 3.3 Level core (`PGL1`)

Offsets in bytes. Fixed order, tables follow the header back to back, each count in the header.

`LevelHeader`, 32 bytes:

| Off | Size | Field | Meaning |
|---|---|---|---|
| 0 | 4 | `magic` | `"PGL1"` |
| 4 | 2 | `version` u16 | 1 |
| 6 | 2 | `min_reader` u16 | 1 |
| 8 | 4 | `flags` u32 | bit 0 has sky, bit 1 has liquids, bit 2 has free planes, bit 3 has movers |
| 12 | 2 | `room_count` u16 | |
| 14 | 2 | `portal_count` u16 | |
| 16 | 2 | `material_count` u16 | |
| 18 | 2 | `sector_units` u16 | S, runtime units per sector |
| 20 | 2 | `pool_pages` u16 | worst-case resident 2 KiB pages for the declared radius (cook computes, hard error above the platform cap) |
| 22 | 1 | `resident_hops` u8 | rooms within this many portal hops of the player room stay pinned |
| 23 | 1 | `prefetch_hops` u8 | additional hops prefetched, evictable |
| 24 | 4 | reserved | zero |
| 28 | 4 | `layout_hash` u32 | see 3.2 |

`RoomRecord`, 40 bytes, `room_count` of them:

| Off | Size | Field |
|---|---|---|
| 0 | 4 | `pack_sector` u32, first sector of the chunk inside `WORLD.PAK` |
| 4 | 2 | `pack_sectors` u16 |
| 6 | 2 | `flags` u16: bit 0 outdoor (sky may show), bit 1 fog, bit 2 pinned (always resident), bit 3 has liquid |
| 8 | 12 | `origin` [i32; 3], world runtime-unit position of local cell (0,0) at the room's vertical offset |
| 20 | 1 | `width` u8, sectors along X |
| 21 | 1 | `depth` u8, sectors along Z |
| 22 | 2 | `portal_first` u16 |
| 24 | 2 | `portal_count` u16 |
| 26 | 2 | `min_y` i16, lowest geometry, room-local |
| 28 | 2 | `max_y` i16 |
| 30 | 2 | `fog_near` u16, runtime units |
| 32 | 2 | `fog_far` u16 |
| 34 | 3 | `fog_rgb` [u8; 3] |
| 37 | 3 | reserved |

World position of a room-local point is `origin + (x, y, z)`. The core holds only what is needed before a chunk is resident: where the room is, how big, how to fetch it, fog (so the fog setup never waits on a chunk), and its portals.

`Portal`, 36 bytes, directed (the cook emits both directions), `portal_count` of them, grouped per source room:

| Off | Size | Field |
|---|---|---|
| 0 | 2 | `dest` u16, destination room |
| 2 | 2 | `flags` u16: bit 0 vertical (floor or ceiling opening), bit 1 always open |
| 4 | 24 | `v` [[i16; 3]; 4], opening corners, **source-room local**, ring order |
| 28 | 6 | `normal` [i16; 3], Q12, pointing into `dest` |
| 34 | 2 | reserved |

`Material`, 8 bytes, `material_count` of them, global:

| Off | Size | Field |
|---|---|---|
| 0 | 2 | `texture_asset` u16, an `AssetId` in the manifest asset table |
| 2 | 2 | `flags` u16: bits 0 to 1 sidedness (front, back, both; matches `psx_level::material_flags::FACE_*`), bit 2 alpha cutout (palette index 0 transparent), bit 3 liquid surface |
| 4 | 1 | `blend` u8: 0 opaque, 1 average, 2 add, 3 subtract, 4 add-quarter (the PS1 semi-transparency modes plus opaque) |
| 5 | 1 | `uv_motion` u8: index into the manifest's existing `LevelMaterialUvMotion` table, 0 none |
| 6 | 2 | reserved |

Why a separate material table rather than the prototype's `u16` asset list (`grid.rs:230`): sidedness, blend and cutout are per material, and the renderer needs them without a second lookup through the asset table.

### 3.4 Room chunk (`PGRM`)

A chunk is `header | collision block | render block`, padded to 2048. The collision block comes first on purpose: when the streamer has landed the first `render_offset` bytes (a few sectors), the room is **collidable** and entities may enter it; when the rest lands it is **drawable**. A player who outruns a load stands on a floor that is not drawn yet instead of falling out of the world. (This is also the cheap answer to "room not resident yet": an unloaded room is treated as solid at its footprint, so nothing walks into it.)

`RoomHeader`, 152 bytes at offset 0. The section table at the end holds one `(offset u32, count u32)` per section in the order of 3.5; offsets are chunk-relative and 4-aligned.

| Off | Size | Field |
|---|---|---|
| 0 | 4 | `magic` `"PGRM"` |
| 4 | 2 | `version` u16 |
| 6 | 2 | `flags` u16: bit 0 has LOD, bit 1 has planes, bit 2 has liquids, bit 3 has movers |
| 8 | 4 | `byte_len` u32, unpadded; must equal the pack entry's `byte_size` |
| 12 | 4 | `render_offset` u32, byte offset of the render block (first byte after the collision block) |
| 16 | 2 | `room_id` u16 |
| 18 | 1 | `width` u8 |
| 19 | 1 | `depth` u8 |
| 20 | 2 | `min_y` i16 |
| 22 | 2 | `max_y` i16 |
| 24 | 1 | `lod_far_cells` u8, distance in sectors beyond which merged quads replace cell quads (0 = no LOD) |
| 25 | 1 | `lod_shift` u8, log2 of the LOD block edge in cells (2 = 4x4) |
| 26 | 2 | reserved |
| 28 | 120 | section table, 15 x `(offset u32, count u32)` |
| 148 | 4 | reserved |

The exact packed layout is one Rust struct in `psx-grid` with size and offset asserts, and the `layout_hash` covers it. This document is normative for field meaning, the struct is normative for bytes.

### 3.5 Sections

Order is the order in the file. "C" = collision block, "R" = render block.

| # | Block | Section | Record | Bytes | Count | Used by (zero-copy slice) |
|---|---|---|---|---|---|---|
| 0 | C | `SECTORS` | `Sector` | 20 | `width*depth`, dense, z-major | every collision query, indexed by `cz*width + cx` |
| 1 | C | `SECTOR_EXTS` | `SectorExt` | 8 | cells that have walls, planes or a split-B | reached from `Sector.ext` |
| 2 | C | `WALLS` | `ColWall` | 12 | solid wall pieces | reached from `SectorExt.wall_first/count` |
| 3 | C | `SPLITBS` | `SplitB` | 16 | cells with a split-level second triangle | reached from `SectorExt.splitb` |
| 4 | C | `COL_PLANES` | `ColPlane` | 28 | collidable free planes, copied per overlapped cell | reached from `SectorExt.plane_first/count` |
| 5 | C | `LIQUIDS` | `Liquid` | 12 | liquid volumes | scanned linearly (few) |
| 6 | C | `MOVERS` | `Mover` | 32 | doors, lifts, destructibles | mover tick, logic binding |
| 7 | C | `MOVER_VERTS` | `u16` | 2 | vertex indices owned by movers | mover tick |
| 8 | R | `MATERIAL_USE` | `u8` | 1 | distinct materials the room draws | VRAM upload requests when the room becomes drawable |
| 9 | R | `VERTICES` | `Vtx` | 8 | lattice plus plane corners | projection (GTE `LWC2`) |
| 10 | R | `QUADS` | `Quad` | 40 | cell quads, then plane quads | select, emit |
| 11 | R | `CELLS` | `RCell` | 8 | `width*depth`, dense, z-major | row-span cull |
| 12 | R | `LOD_BLOCKS` | `LodBlock` | 8 | `ceil(w/4) * ceil(d/4)` | LOD selection |
| 13 | R | `LOD_QUADS` | `Quad` | 40 | merged far quads | emit |
| 14 | R | `PLANE_REFS` | `PlaneRef` | 12 | free planes | per-plane frustum test |

#### Collision records

`Sector`, 20 bytes. Heights are room-local `i16` runtime units, corners `[NW, NE, SE, SW]` as in `GridSurface::heights` (`grid/surface.rs`).

| Off | Size | Field |
|---|---|---|
| 0 | 8 | `floor` [i16; 4] |
| 8 | 8 | `ceil` [i16; 4] |
| 16 | 2 | `flags` u16, below |
| 18 | 2 | `ext` u16, index into `SECTOR_EXTS` or `0xFFFF` |

`Sector.flags`:

| Bit | Name | Meaning |
|---|---|---|
| 0 | `HAS_FLOOR` | clear = pit or void (no floor; `floor[]` unused) |
| 1 | `HAS_CEIL` | clear = open above |
| 2 | `FLOOR_SPLIT_NESW` | floor diagonal is NE-SW, else NW-SE |
| 3 | `CEIL_SPLIT_NESW` | same for the ceiling |
| 4 | `FLOOR_WALK_A` | floor triangle A is walkable (slope within the walk limit and the document's `walkable` flag) |
| 5 | `FLOOR_WALK_B` | same for triangle B |
| 6 | `CEIL_SOLID` | the ceiling blocks the head (a document ceiling with `walkable` off is decoration) |
| 7 | `OPEN_SKY` | no ceiling and the room is outdoor: counts toward `sky_visible` |
| 8 | `LIQUID` | some liquid record covers this cell |
| 9 | `SPLIT_LEVEL` | second triangle has its own heights, see `SplitB` |
| 10 to 15 | reserved | |

The walk bits are computed at cook with the same rule the motor applies (`normal_y_q12 >= 2867`, `character_motor.rs:38`), so the guest never needs a square root to decide walkability. Normals are computed only at contact, from the corner heights.

`SectorExt`, 8 bytes:

| Off | Size | Field |
|---|---|---|
| 0 | 2 | `wall_first` u16 |
| 2 | 1 | `wall_count` u8 |
| 3 | 1 | `plane_count` u8 |
| 4 | 2 | `plane_first` u16 |
| 6 | 2 | `splitb` u16 or `0xFFFF` |

`ColWall`, 12 bytes. Geometry is implied by the owning cell and the edge id, so no coordinates are stored.

| Off | Size | Field |
|---|---|---|
| 0 | 1 | `edge` u8: 0 N, 1 E, 2 S, 3 W, 4 NW-SE diagonal, 5 NE-SW diagonal (same order as `WallEdge::ALL`, `grid/sector.rs`) |
| 1 | 1 | `flags` u8: bit 0 blocks from the front only, bit 1 mover-owned |
| 2 | 8 | `y` [i16; 4], `[bottom at a, bottom at b, top at b, top at a]` for the edge's walking direction `a -> b` (`GridWall::heights`, `grid/surface.rs`) |
| 10 | 2 | reserved |

Only walls with `solid` on (`GridWall::solid`) are cooked into this table. A drawn but non-solid wall exists only as render quads.

`SplitB`, 16 bytes: `floor_b` [i16; 4] and `ceil_b` [i16; 4], the corner heights of the second triangle when the document set `heights_b` (`GridSurface::heights_b`). Rare in practice (level_008 has 24 of its 332 floors **[measured]**, `docs/grid/other-maps-survey.md`), hence sparse.

`ColPlane`, 28 bytes (section 9):

| Off | Size | Field |
|---|---|---|
| 0 | 1 | `kind` u8: 1 platform, 2 wall, 3 blocker box |
| 1 | 1 | `flags` u8: bit 0 camera passes through, bit 1 one-sided |
| 2 | 1 | `id` u8, the plane's index in the room (dedupes the per-cell copies during a trace) |
| 3 | 1 | reserved |
| 4 | 16 | `xz` [[i16; 2]; 4], footprint corners (platform), segment a and b in the first two (wall), min and max in the first two (blocker) |
| 20 | 2 | `y0` i16, height at `xz[0]` (platform), bottom (wall, blocker) |
| 22 | 2 | `sx_q12` i16, slope dy/dx (platform), unused otherwise |
| 24 | 2 | `sz_q12` i16, slope dy/dz (platform), unused otherwise |
| 26 | 2 | `extent` i16, thickness below the surface (platform), height (wall, blocker) |

`Liquid`, 12 bytes: `x0, z0, x1, z1` u8 (inclusive cell rectangle), `kind` u8 (1 water, 2 slime, 3 lava, the same ordering the guest's `CONTENTS_*` constants use today), `flags` u8, `surface_y` i16, `bottom_y` i16, 2 reserved.

`Mover`, 32 bytes (section 6.6): `kind` u8 (door, lift, destructible), `flags` u8 (start open, enabled), `id` u8, 1 pad; `vert_first` u16 and `vert_count` u16 into `MOVER_VERTS`; `wall_first` u16 and `wall_count` u16 (a contiguous run in `WALLS`); `tail_cell` u16 and `tail_count` u8 (destructibles: the quads are the last `tail_count` of that cell's list); `offset` [i16; 3] (open travel); `travel_ticks` u16; `progress_ticks` u16 and `target_open` u8 (mutable state, zero at load); reserved.

#### Render records

`MATERIAL_USE` is a plain list of level-global material ids (`u8`), padded to 4 bytes. `Quad.material` is itself a **level-global** `u8` index, so the emit loop reads one level-wide `[GridMaterial; N]` array with no per-room remap. The cook errors if a level has more than 255 distinct materials (VRAM is the real limit long before that).

`Vtx`, 8 bytes: `x, y, z` i16 room-local, `pad` i16. The pad is zero. Whether the GTE ignores the upper half of the `VZ` word (so the pad could carry data) is a **[hypothesis]**; the prototype keeps it clean on purpose (`grid.rs:70`). Experiment: one headless test that loads a vertex with a non-zero pad via `LWC2` and compares `RTPT` output. If the upper half is ignored, the pad could hold 16 bits of per-vertex light and shrink quads (section 4.6, option C).

`Quad`, 40 bytes (prototype: 44, `grid.rs:104-122`):

| Off | Size | Field |
|---|---|---|
| 0 | 8 | `v` [u16; 4], room-local vertex indices, ring order |
| 8 | 16 | `color` [u32; 4], baked corner light, `r + (g << 8) + (b << 16)`, ready to OR with the GP0 command byte |
| 24 | 8 | `uv` [u16; 4], `u + (v << 8)` per corner, material-relative texels |
| 32 | 2 | `d` i16, plane constant in runtime units: front when `(n . camera) >> 7 > d` |
| 34 | 3 | `n` [i8; 3], Q7 normal |
| 37 | 1 | `material` u8, level-global material index |
| 38 | 1 | `flags` u8 |
| 39 | 1 | reserved |

The saving is `d` from `i32` to `i16`: `n` is Q7 so `n . p` can reach 127 x 32767 = 4.16M, which an `i16` cannot hold, so the compare shifts the dot product right by 7 (one extra ALU op per tested quad) and stores `d` in plain runtime units. Axis-aligned quads keep the multiply-free path and store the plane coordinate in `d` directly.

`Quad.flags`:

| Bit | Name | Meaning |
|---|---|---|
| 0 | `TRI` | three corners; `v[3] == v[2]` |
| 1 | `AXIS` | plane is axis aligned; `d` is the signed plane coordinate (as `GRID_QUAD_AXIS`, `grid.rs:64`) |
| 2 to 3 | `AXIS_ID` | 0 x, 1 y, 2 z |
| 4 | `AXIS_NEG` | negative-facing |
| 5 | `DOUBLE` | no back-face cull |
| 6 | `PLANE` | free plane (drawn through the plane list, not a cell list) |
| 7 | `LOD_COVERED` | a merged LOD quad stands in for this quad at far range |

`RCell`, 8 bytes: `first` u16 (index into `QUADS`), `count` u8, `flags` u8 (bit 0 `OPEN_SKY`, bit 1 `HAS_LOD`, bit 2 `NEAR_RISK` reserved), `min_y` i16, `max_y` i16. The prototype's `u16 count` becomes `u8` to make room for flags; the cook errors if a cell needs more than 255 quads. The validator's caps allow it in theory (16 walls per edge, each banded by sector height, `grid/limits.rs`); real cells hold a handful.

`LodBlock`, 8 bytes: `first` u16 (into `LOD_QUADS`), `count` u8, `flags` u8, `min_y` i16, `max_y` i16.

`PlaneRef`, 12 bytes: `quad` u16, `col` u16 (first `ColPlane` index or `0xFFFF`), cell rectangle `x0, z0, x1, z1` u8, `min_y` i16, `max_y` i16.

### 3.6 Alignment and zero-copy use

At install, `RoomView::new(bytes)` checks `magic`, `version`, `layout_hash`, `byte_len`, that every section's `offset + count * size <= byte_len`, and that offsets are 4-aligned. It then builds 15 `&[T]` slices with the prototype's `table::<T>` cast (`grid.rs:182`). No allocation, no copy, no parse. The page pool's generation-checked `PageHandle` guards against a stale view surviving a slot reuse (`streaming/page_pool.rs:30`).

Two mutation sites exist and both run outside the draw call: the mover tick (writes vertex `x/y/z`, `ColWall.y`, `Mover.progress`) and destructible break (writes one `RCell.count` and one `Mover.flags`). They go through a `RoomMut` handle holding the raw pointer; the guest is single threaded, so this is a convention plus a debug assertion, not a lock.

---

## 4. Render data and how the renderer uses it

### 4.1 What a room draw does

```
for each visible room (section 7.2), nearest first:
    set GTE translation to rotate(room.origin - eye)         // 3 rotated components, once per room
    translate the 5 frustum planes into room-local space     // 5 dot products, once per room
    bump the vertex-stamp epoch (u16, fill on wrap)
    select_cells   : row spans, queue cells          (grid.rs:466)
                     + LOD decision per cell (4.8)
    select_quads   : backface test, stamp corners    (grid.rs:568)
    plane pass     : frustum-test PLANE_REFS, queue their quads (9.3)
    project_corners: GTE RTPT three at a time        (grid.rs:643)
    emit           : one GP0 textured Gouraud quad per surviving quad (grid.rs:711)
```

The per-room GTE translation is the only structural addition to the prototype's single global grid. Vertices stay `i16` room-local, so the GTE transforms them with no per-vertex add (the same trick Tomb Raider used).

### 4.2 Shared vertex lattice per room

One `Vtx` per distinct snapped position per room. The cook snaps each corner to the runtime lattice (`rt()` of the grid coordinate), deduplicates by `(x, y, z)`, and numbers vertices in **first use order while walking cells z-major**. Consequences:

* A visible row of cells touches a compact vertex range, so the stamp table and screen cache are read and written nearly sequentially.
* Floors, ceilings and walls that meet at a grid point share one vertex, so a corner is projected once per frame however many quads use it. Prototype ratio: 1718 quads x 4 corners = 6872 corner uses over 1883 vertices, 3.65 uses per vertex **[derived]**.
* A vertex is keyed on position only. Baked light lives on the quad, not the vertex (4.6), which is what allows a floor and a wall to share a corner and still have different lighting.

### 4.3 Quads and triangles, ordering for the emit loop

Cell lists are contiguous in `QUADS`, cells are in z-major order, so scanning a row span walks `QUADS` in increasing address order. Inside a cell the order is fixed: floor (one quad, or two triangles if the surface is not planar), ceiling, then walls by edge in `WallEdge::ALL` order, then movers' destructible tail. Material is deliberately **not** a sort key: every textured packet carries its own tpage word, and draw order is decided by OT depth, so sorting by material buys nothing here.

The face derivation (planar-merge rule, wall banding at multiples of one sector so the world-aligned texture tiles, winding, 32 texels per sector UV) is already specified and tested in `grid/faces.rs:1-30`. The cook consumes that module and does not re-derive geometry.

### 4.4 Cell table and culling

`CELLS` is dense, `width * depth`, so a row `cz` is the slice `[cz*width .. (cz+1)*width]`. The prototype's row-span cull (`select_cells`, `grid.rs:466-566`) intersects the near, left and right planes with each row slab to get an `[lo, hi]` interval of columns, then queues the non-empty cells in it. It tests only the three horizontal planes and relies on per-corner outcodes for vertical rejection. That stays. Two changes: the room's `min_y/max_y` feeds the plane/slab test (as the pack's `min_y/max_y` does today), and the sky flag is read from `RCell.flags` while queueing, which replaces the macro-cell table entirely (the prototype's 4x4 macro blocks only existed to find `sky_visible` and the lid, `grid.rs:466-512`).

### 4.5 Materials, tpage and CLUT

`quad.material` (u8) is a level-global index into one runtime array `[GridMaterial; material_count]` (tpage, clut, uv origin, resident flag; `grid.rs:290`), resolved through the existing playtest VRAM owner (`resolve_material_binding`, `engine/examples/editor-playtest/src/bsp_runtime.rs:564-619`, which is renderer-independent and moves over unchanged). The level `Material` record supplies sidedness, blend and cutout, so the emit loop makes no second lookup. When a room becomes drawable the runtime walks its `MATERIAL_USE` list and queues uploads for any material not yet resident. A missing texture drops the quad and counts it (`material_missing`, `grid.rs:301`) exactly as today.

### 4.6 Baked per-corner light

The editor bakes lit colour per face corner with a Lambert term against the face normal (`grid/light.rs`). Today it has **no shadow occluders** ("There are no shadow occluders yet", `grid/light.rs:1-9`); the prototype got shadows from the derived collision slabs. The new cook must get shadows from somewhere that is not PXBSP brushes: it runs the **runtime's own `line_of_sight` / trace query** (section 5.5) from each lit corner toward each light, on the host, with the same `psx-grid` code the guest uses. One implementation, no drift, and it also means the bake and the collision agree about what is solid.

Encoding options for the four corner colours of a quad:

| Option | Bytes per quad | Decode cost in emit | Notes |
|---|---|---|---|
| A. `[u32; 4]`, GP0-ready (prototype) | 16 | none | measured baseline |
| B. `[u16; 4]` RGB555 | 8 | about 8 ALU ops per corner **[estimate]**, about 32 per quad, about 4% of 800 cycles | the PS1 framebuffer is 15 bit and dithers, so 5 bits per channel loses nothing the GPU would keep; quad shrinks to 32 bytes (-20%) |
| C. light in the vertex pad | 0 | once per projected vertex, amortised over 3.65 quads | needs the `VZ` pad **[hypothesis]**, and loses normal-dependent shading at shared corners |

Recommendation: ship A (it is the measured configuration), keep B behind a cook flag, and decide with one measurement in phase 3 (emit cycles per packet with B decoded). On the Cathedral B saves 13.7 KB **[derived: 1718 x 8]**; whether that matters depends on the pool size chosen in section 7.

### 4.7 Near-field data and the clip path

This is the biggest cost lever the logs expose, so it gets measured evidence first.

The open-hall bench logs (`/Users/ebonura/Desktop/repos/PSoXide-editor-wt/open-hall/build/oh-runs/grid/*-run.txt` and `*/stats_t.txt`, all written after commit `b9761a92`, the grid's own split profile) record, per frame, the world ("room") cycles, the packets written, the split triangles ("robust/split", the counter `room_surf_split_tris`) and the cycles of each phase. The file prefixes do not name the map; `cv`, `dg`, `ws`, `l8` match Cave, Dungeon, West and level_008 by the survey's abbreviations, which is my reading, not a recorded fact.

| Run | Room cycles/frame | Emit cycles | Emit share | Packets/frame | Split tris/frame | Emit per packet | Splits per packet |
|---|---|---|---|---|---|---|---|
| `cv-grid-run` | 330,385 | 272,340 | 82% | 88 | 25.4 | 3.09k | 0.29 |
| `dg-grid-run` | 335,592 | 235,650 | 70% | 149 | 20.6 | 1.58k | 0.14 |
| `ws-grid-run` | 141,613 | 105,661 | 75% | 27 | 10.3 | 3.91k | 0.38 |
| `l8-grid-run` (level_008) | 282,300 | 167,168 | 59% | 205 | 8.8 | 0.82k | 0.043 |

**[measured]** columns are room cycles, emit cycles, packets and split triangles; the shares and ratios are **[derived]**. A line through the two extreme runs (`l8` and `ws`) gives `emit per packet = 430 + 9.2k x (splits per packet)`; it reproduces `cv` (3.09k predicted, 3.09k measured) and lands within 8% on `dg` (1.71k against 1.58k). Two readings follow, both **[derived]**: the intercept of about 430 cycles matches the measured fast-path cost of about 420 per packet, and each split triangle costs about 9.2k cycles, roughly twenty fast-path quads. The Cathedral comparison in the memory notes agrees in direction: grid with the depth-band split off ("bands 0,0") is 237k against 280k with it, so splitting is about 43k of that frame, reading "bands 0,0" as both split distances set to zero **[measured, memory `cortex-grid-engine-preference-2026-10-16`]**.

Two causes feed the counter. A quad with a corner behind the near plane goes down the clip path. And any quad nearer than 170 depth units (about two sectors at S = 84, `GRID_RENDER_PROFILE`, `grid.rs:48-52`) is subdivided once to bound affine texture warp, whatever its size; in a corridor or a small cave most visible quads are inside that radius.

What this means for the format:

* No near-field **table** is needed. The expensive thing is the generic path (`submit_surface_batch_near` through `robust_quad`, `grid.rs:931`), which handles arbitrary polygons. Grid quads are special: most are axis-aligned rectangles on the lattice (the `AXIS` flag marks them), and a split of such a quad is a fixed 2x2 cut. A dedicated path for "quad nearer than the split depth, no corner behind the near plane" can project five extra points (four edge midpoints and the centre, each an average of two or four existing `i16` vertices, so three or four adds with no GTE load) and write four ordinary packets. By cost per piece, that is roughly five projections at about 69 plus four packets at about 420, near 2k cycles **[estimate, unmeasured]**, against the fitted 9.2k. If it holds, level_008 saves about 8.8 x 7k = 62k cycles of 282k and Cave about 25.4 x 7k = 178k of 330k **[estimates built on that estimate]**. This is the highest-value experiment in the document and it needs no format change.
* The few quads that really cross the near plane keep the generic clip path.
* Midpoints rounded to the lattice can open a sub-pixel crack against an unsplit neighbour's straight edge (a T-junction). Round every midpoint the same way (`(a + b) >> 1`), keep it on the edge's line where the edge is axis aligned, and check by eye on the bench corridors before trusting it.
* Camera clearance still matters: the camera's spring-arm point trace stops short of walls by the authored margin (`bsp_runtime.rs:1911-1946`); keep that margin at least the near plane plus half a sector so walls do not routinely cross the lens.
* Reserved: `RCell.flags` bit 2 `NEAR_RISK`, and a reserved section id for an optional pre-split fine-cell table, in case the dedicated path does not get close enough. Not specified, because the measurement above may make it unnecessary.

### 4.8 Distance LOD for open floors

Open halls cost the same in any representation (memory `bonnie32-levels-ps1-budget-2026-10-16`); the lever is fewer, bigger far quads. The format supports it for **horizontal surfaces only**, which dominate open rooms and merge cleanly.

* The cook finds maximal axis-aligned rectangles of cells whose floor (or ceiling) is planar, same material, same split behaviour, contiguous UV, within a 4x4 block (`lod_shift = 2`), and emits one merged `Quad` per rectangle into `LOD_QUADS`, with corners that already exist in `VERTICES` (a merged rectangle's corners are cell corners). Light is baked at the merged corners from the same light set.
* Every cell quad covered by a merged quad gets `LOD_COVERED`.
* At runtime, `select_cells` computes the squared distance from the camera to each queued cell; beyond `lod_far_cells * S` it sets a per-block stamp instead of queueing covered quads; `select_quads` skips `LOD_COVERED` quads when the block stamp is set (one flag test per quad), and emits each block's `LOD_QUADS` once. Walls and non-merged surfaces keep their cell quads at every distance.
* The merged quad's UV span must stay inside the u8 packet range after the cook subtracts whole texture periods; the prototype already counts overflowing spans (`uv_overflow`, `editor/crates/psxed-project/src/brush_grid/mod.rs:106-130`) and the cook reuses that check.

Expected effect, **[derived from analyzer counts, an estimate for the engine]**: Cathedral frustum p50/p90/max of 397/928/1467 tiles becomes 147/186/243 with a 16-sector cut and 2x2 merge beyond 6 sectors. At about 800 cycles per quad that is 118k/149k/194k cycles of world instead of 318k/742k/1.17M. The analyzer's tile model is not the cook's quad model (wall banding differs), so treat this as a target, and gate on the measured result in phase 6.

Memory cost: LOD quads are extra records. Cathedral floor is about 858 cells at most, merged 4x4 into at most `ceil(33/4) * ceil(26/4) = 9 * 7 = 63` blocks **[derived]**. One merged quad per block is 63 x 40 B = 2.5 KB; partial merges add some; call it a few KB **[estimate]**.

---

## 5. Collision

### 5.1 Contract and why not the old backends

The live motor and camera take a `CollisionTraceProvider` (`collision_query.rs:110`; `character_motor.rs:752`; `third_person_camera.rs:494`). Everything gameplay asks of the world (floor under feet, body sweep, ledge, lift, step, camera arm, melee segment, projectile segment, hook landing, enemy step) is a `CollisionTraceQuery` of shape `Point` or `Body { radius, height }`. `CharacterBlockerTraceProvider` already layers dynamic cylinders and prop AABBs on top (`character_motor.rs:136`). So the grid world needs exactly one thing: a `GridTraceProvider` implementing that trait. All existing motor, camera and enemy tests keep their meaning.

Salvage verdicts:

| Candidate | Verdict |
|---|---|
| `e1e55509:engine/crates/psx-engine/src/floor_sample.rs` (68 lines) | **Keep the math**, reimplemented in `psx-grid` with the reciprocal and the overflow proof of 2.5. Same corner order and split convention (`SPLIT_NE_SW`, `v <= u` test). Its header comment states the right invariant: motor and camera must sample the same floor. |
| Grid backend in `character_motor.rs` (`stand_height`, `floor_probe`, `body_hits_solid_wall`, `wall_blocks_body`, `circle_overlaps_segment`; `e1e55509:...character_motor.rs:2486-2705`; about 1960 lines removed by `22894367`) | **Do not restore.** The live motor no longer has a backend trait; it speaks traces. **Keep the rules and the helpers as the test oracle** for the new provider: five-point footprint sampling, a wall blocks only if it overlaps the body's vertical span and its top is more than `STEP_UP_HEIGHT` above the feet, circle-vs-segment overlap. |
| Grid camera backend in `third_person_camera.rs` (`GridCameraCollision`, sector-stepping rays at `e1e55509:...third_person_camera.rs:1794-1950`) | **Do not restore.** The live camera already uses point traces (`TraceCameraCollision`, `third_person_camera.rs:1534`). Read the sector-stepping ray as the reference for the DDA in 5.3 (item 5). |
| `.psxw` v5 (`crates/psxed-format/src/world.rs`, `sdk/crates/psx-asset/src/lib.rs` `World`, `WorldSectorFloorCollision`) | **Do not keep as the format.** 60 B sector with `i32` heights, 32 B wall, no render data ("render path WIP" in its own roadmap), no vertex lattice. The roadmap's compact sketch (28 B sector, 12 B wall; `docs/world-format-roadmap.md`) is superseded by section 3.5 (20 B sector plus 8 B ext, 12 B wall). Both crates live in the materialized SDK repo, so on this branch they are left alone and scheduled for removal upstream. |
| `RuntimeRoom` / `RoomCollision` facades (`e1e55509:engine/crates/psx-engine/src/world.rs`, 1539 lines) | Do not restore. `RoomView` (3.6) replaces them with far less API. |

### 5.2 Static-world model

The world is a height field with layers:

* **Floor** of a cell: two triangles over four corner heights, diagonal per `FLOOR_SPLIT_NESW`, optional `SplitB` override. No `HAS_FLOOR` means void.
* **Ceiling** likewise, blocks the head when `CEIL_SOLID`.
* **Implicit risers**: the vertical face between two adjacent cells whose floors differ. There is no record for it; it falls out of sampling the floor at the body's footprint. A body moving into a cell whose floor is more than `STEP_UP_HEIGHT` above its feet is blocked with a horizontal contact normal along the crossed edge.
* **Wall pieces** (`ColWall`): explicit vertical quads on the six edges, tested as 2D segments with a vertical range.
* **Plane colliders** (`ColPlane`): platforms, free-standing walls and box blockers (section 9).
* **Liquids** are not obstacles; they are queried (6.5).

### 5.3 Query primitives (all O(1) per cell)

1. `locate(hint_room, p) -> Option<(room, cx, cz)>`: test the hint room's footprint and vertical fit first; then the hint's portal destinations; then a linear scan of the room directory (small) as the slow path. Stacked rooms (the Dungeon has 45 plan cells held by two rooms **[measured]**, `docs/grid/other-maps-survey.md`) are separated by comparing `y` with the cell's floor and ceiling. Entities carry a room hint (`u16`) updated by this function; coordinates stay world space (do not repeat "room-local entity coordinates", archaeology section 5 item 13).
2. `floor_at(room, cx, cz, lx, lz) -> Option<i32>` and `ceil_at(...)`: one `Sector` load, the split test, `height_at_local` (2.5), then the `SplitB` and plane layers when flagged. Platforms contribute `max(h)` over those at or below `feet + STEP_UP_HEIGHT`.
3. `wall_hits(room, cx, cz, circle, y_span) -> Option<contact>`: loops the cell's `ColWall` run (`SectorExt`), segment endpoints from the edge id and `S`, `circle_overlaps_segment` logic.
4. `contents_at(room, p) -> Contents`: scans `LIQUIDS` (a handful) for cell membership and `bottom_y <= y <= surface_y`.
5. `line_of_sight(a, b) -> bool`: a DDA over cells along the segment; per cell it tests floor and ceiling against the ray height at entry and exit, and the cell's `ColWall` pieces by 2D segment intersection plus vertical range at the hit.

### 5.4 The trace provider

`GridTraceProvider::trace_into(query, out)` classifies the query by its geometry (no new API):

1. **Vertical probes** (the motor's `trace_supporting_floor`, `character_motor.rs:1522`, a long near probe then a 32,767-unit far probe): evaluate floor height over the footprint, the five-point set (centre plus four compass points at `radius`), take the highest walkable floor at or below the start, return fraction, end, and the floor normal Q12 (from the triangle's corner heights, one integer square root at contact via `psx_math::int32::isqrt_i32`). Rejection of non-walkable slopes uses the `FLOOR_WALK_*` bits.
2. **Short horizontal body moves** (`trace_stand_position` direct, lift, across, settle; `:1566`): march the segment in steps of at most `min(radius, S/4)`. At each step test footprint floor (step rule), ceiling, walls, planes. On the first blocked step, bisect the last interval to Q12 resolution and return the last clear fraction (conservative: never penetrates), with `start_solid` if the start itself is blocked.
3. **Long point segments** (camera arm, melee, projectile, LOS): the cell DDA of 5.3, with analytic per-cell tests, not stepping.

Constraint: body radius must be at most `S/2` so a footprint touches at most 2x2 cells; checked when the controller config is loaded.

Contact normals: floors give the triangle normal; risers give the horizontal normal of the crossed edge; wall pieces give the segment normal (flipped toward the mover); ceilings give `(0, -4096, 0)`.

I do not give cycle costs for these traces. The motor issues up to about five traces per moving body per tick (direct, near and far floor probe, lift, across, settle), so the provider's per-trace cost matters, and I have no measurement. Phase 3 produces it (section 11) with a hard gate: the provider must not exceed the BSP provider's per-tick collision cost on the Cathedral route, measured with the same tape.

### 5.5 What collision shares with the bake

`line_of_sight` and `locate` are the same code the light bake calls on the host (4.6), the same code the editor's walk test calls, and the same code enemy sight uses. That is the point of putting the world logic in a `no_std` crate with a `std` feature.

### 5.6 Step and climb rules

Taken from the live motor and unchanged: step up to `STEP_UP_HEIGHT` (40 units), snap down within `STEP_DOWN_HEIGHT` (40), floor needs normal y >= 2867/4096 (about 0.7, 45.6 degrees), body falls past a deeper drop. What changes is where those numbers come from (a motor config field, 2.3), not what they do. Ledge-grab or climb is not a motor feature today and is out of scope.

---

## 6. Everything else `BspRuntime` provides

`BspRuntime` is `engine/examples/editor-playtest/src/bsp_runtime.rs` (2965 lines); the playtest's callers are in `game_logic_runtime.rs`, `playtest_update.rs`, `playtest_scene.rs`, `playtest_runtime.rs`, `main.rs`, `duel.rs`, `hook_runtime.rs`, `player_ranged.rs`, `marker_runtime.rs`, `opening_sequence.rs` (counted with a grep for each method name). The new `GridRuntime` keeps the same call shapes where callers should not have to change.

| BspRuntime member (line) | Used by | Grid replacement |
|---|---|---|
| `load_manifest` (:670), `refresh_materials` (:1345), `invalidate_materials` (:1378), `materials_ready` (:1387), `world_stream_ready` (:1369) | `main.rs`, `playtest_update.rs`, `playtest_scene.rs` | `GridRuntime::load` builds the core view; `refresh_materials` is the same latch over the level `Material` table. `world_stream_ready` becomes "pinned rooms drawable". `resolve_material_binding` (:564) is reused unchanged. |
| `draw` (:1976), `last_world_packet_words` (:1961), `fits_before_fence` (:1968) | `playtest_scene.rs` | `GridRuntime::draw`: section 4.1 per visible room, summing packet words so the fence prediction keeps working. |
| `visible_points_mask` (:918), `visible_bounds_mask` (:949) | `playtest_update.rs` (entity, model-instance and logic activation) | Same signatures, 64-bit masks. A point or bounds is active when its room is in the active room set (below) and within the room's draw distance. No leaf PVS. |
| `visible_world_objects` (:1006), `typed_world_object_directly_visible` (:1130) | `playtest_scene.rs`, `playtest_runtime.rs` | Room set for the coarse test, `line_of_sight` for objects flagged strict occlusion. The PXBSP-specific cooked leaf links, solid-cell witnesses and occlusion caches (`bsp_runtime.rs:368-540`, `playtest/world_object_leaf_links.rs`) have no equivalent and are dropped. |
| `player_contents` (:1309) | `playtest_update.rs`, returns `LiquidContentsSample` | `contents_at` sampled at feet, torso, head (same three samples). The sample type and `CONTENTS_*` constants move out of `psx_bsp::collision` into `psx-grid` and keep their values. |
| `tick_doors` (:1404), `set_door_open` (:1680), `nearest_door` (:1687) | `playtest_update.rs`, `game_logic_runtime.rs` | `Mover` tick (6.6). |
| `damage_brush_destructibles_with_capsule` (:1412), `destructible_effect_origin` (:1488), `spawn_destructible_fragments` (:1509), `advance_destructible_fragments` (:1520), `draw_destructible_fragments` (:1535) | `game_logic_runtime.rs`, `playtest_update.rs`, `playtest_scene.rs` | Destructible `Mover` (6.6); fragments reuse `psx-game-runtime/src/particles/fracture.rs` with the destroyed quads' corners from `VERTICES`. |
| `update_motor` (:1737), `trace_hook_body` (:1769), `commit_body_step` (:1805), `commit_body_direction` (:1838), `melee_segment_clear` (:1874), `trace_point_segment` (:1889) | `playtest_update.rs`, `hook_runtime.rs`, `game_logic_runtime.rs`, `duel.rs`, `player_ranged.rs`, `marker_runtime.rs`, `opening_sequence.rs` | Same bodies with `GridTraceProvider` in place of `PxbspCollisionProvider`; `CharacterBlockerTraceProvider` composition unchanged. The `select_body_hull` / `PXBSP_BODY_HULLS` machinery disappears (a grid has no per-radius hull). |
| `update_camera` (:1911), `camera_traces` (:1956) | `playtest_runtime.rs` | Same body, point traces through `GridTraceProvider`. |
| `collision_models` / `CollisionModels` (:111, :2213) | internal | Deleted (BSP submodels). Movers are not collision models; they patch records (6.6). |
| Sky (`sky_runtime.rs` uses `psx_bsp::sky` and `load_pxbsp_view_rotation`) | `sky_runtime.rs` | Sky is drawn at the back of the OT independent of world geometry (`SKY_OT_SLOT = 2047`, `psx-bsp/src/sky.rs`); only the "is any sky visible" gate is world-owned. It becomes the per-cell `OPEN_SKY` flag (6.4). The sky module itself must move to a world-neutral crate (deletion review, REPLACE). |

### 6.1 Activation set

Replaces the PVS leaf bitmap. Each frame (or when the camera room changes):

1. `cam_room = locate(prev_cam_room, camera)`; `player_room` likewise.
2. `active = room_graph_ring(player_room, ACTIVATION_HOPS)` (breadth first over portals, nearest first, capped), unioned with the portal-visible set of the camera (section 7.2).
3. A point or bounds query becomes: `room_of(point)` (hint + `locate`), membership in `active`, then the room's `draw_distance` as the fine filter.

`room_graph_ring` is salvaged verbatim from `e1e55509:engine/crates/psx-game-runtime/src/room_streaming.rs:1531-1600` (breadth first, distance ordered, capped, skips invalid indices). Starting hop counts: draw depth 3 and prefetch 2 (the old `RESIDENT_DRAW_DEPTH` and `RESIDENT_PREFETCH_HOPS`; the archaeology doc records 3 and 2 from memory notes, I did not find them in a file I could open, so treat them as starting values to tune).

Honest limit: a single big room such as the Cathedral is one activation unit. Everything in it is active whenever the player is in it. The BSP PVS was finer, but it was also close to "everything" in an open hall, and the radius filter keeps far enemies asleep.

### 6.2 Triggers, perches, logic

Logic records and hook points are positions in world space (`scene_types.rs` `Logic`, `HookPoint`); they need `room_of(point)` for activation and nothing else. `trace_hook_body` only needs the trace provider.

### 6.3 Liquids

`Liquid` records (3.5). `contents_at` is a rectangle test plus two height compares. The visible surface is a free plane with a translucent material (section 9) so water, slime and lava reuse plane rendering, UV motion (`uv_motion`) and blend.

### 6.4 Sky

`RCell.flags & OPEN_SKY` is set by the cook for cells with no ceiling in an outdoor room. `select_cells` ORs it into `sky_visible` while queueing. The guest draws the sky layer iff any drawn room reported it. This removes the prototype's 4x4 macro table and the synthesized sky lid brush (`brush_grid/collision.rs` "sky lid"), which existed only to close the BSP hull for the leak check.

### 6.5 Fog

`RoomRecord.fog_*` (core), so fog setup does not wait on a chunk. Per-room linear depth cue is bonnie's model (`GridFog`, `grid/room.rs:8-33`) and maps onto the existing `LevelRoomRecord` fog fields.

### 6.6 Movers: doors, lifts, destructibles

BSP does these as brush submodels with a transform (`psx-bsp/src/mover.rs`, `destructible.rs`, 521 and 220 lines). Rotating a quad soup at draw time would put a branch in the hottest loop. The grid design avoids it:

* A **mover** is a rigid translation of a private vertex set. The cook gives mover geometry its own unwelded vertices (listed in `MOVER_VERTS`) and its own `ColWall` run, and widens the owning cells' `min_y/max_y` to cover the full travel.
* Per tick, for each active mover with `progress` changing, the runtime writes `vertex += offset * progress / travel_ticks` (a fixed-point lerp, integer progress like `BrushDoor`, so reversing never accumulates error: `mover.rs:19-25`) for each listed vertex, and the same translation into the `ColWall.y` run. O(k) in the mover's size, zero cost in the render loop.
* Destructibles reuse the `Mover` record with `kind = destructible`: the cook sorts the destructible's quads to the **end** of their cell lists, and breaking one subtracts `tail_count` from `RCell.count` and clears its walls from collision by zeroing `wall_count` in the same `SectorExt` run. A level-state bitset of broken ids is re-applied when a room is re-installed after eviction.
* Not supported: rotating movers (swinging doors). A swinging door is a prop, as it can be today (`BoxProp`, `ArchProp` with AABB blockers).
* Caps: 16 doors and `psx_level::MAX_DESTRUCTIBLES` destructibles, the current limits (`bsp_runtime.rs:63-65`).

Binding: logic nodes name movers by name today (`target` strings, `scene_types.rs`); the cook resolves names to a `u16` mover id (`room << 8 | local`) and the manifest's `LevelLogicRecord` carries the id.

---

## 7. Rooms, portals, streaming and RAM

### 7.1 What a room is

A room is a rectangle of sectors on the shared lattice with one vertical offset, one chunk and one ambient/fog setting (`grid/room.rs`). Rooms may overlap in plan; that is how floors stack. The editor already models this.

### 7.2 Portals and visibility

Portals are **derived by the cook, never placed by hand** (the hand-placed portal convention was a documented failure, archaeology section 5 item 4). Rule, following bonnie-32's own `Level::recalculate_portals` (`docs/grid/bonnie32-editor-inventory.md` line 288): for each cell edge on a room's boundary, if the neighbouring lattice cell belongs to another room and the vertical intervals of the two cells' open spans intersect and there is no full-height solid wall across the gap, the open interval is a portal span; spans that are contiguous along an edge and lead to the same room merge into one portal quad. Vertical portals are derived the same way from cells with no floor (or no ceiling) over a cell of another room. Dungeon, the one multi-room sample with portals, has 90 in bonnie's own data (`docs/grid/other-maps-survey.md`), so expect many small portals if merging is weak.

Traversal is the camera-cone portal clipper from `e1e55509:engine/crates/psx-level/src/portal_visibility.rs` (2204 lines, 27 tests): heap-free, it clips each directed portal against the current screen-space window and recurses with the clipped window, so a room is accepted because a projected portal reaches it. Port it with its tests, adapting the input to the 36-byte `Portal` record and the new room directory. Measured on the old system at 8,512 cycles/frame, 0.6% of render **[measured, different era]**.

### 7.3 Streaming

Reuse, do not rebuild:

* Transport: `psx-cdstream` through `RegionRead` (`engine/crates/psx-game-runtime/src/cd_stream/region.rs`), PIO reads. Console CD DMA (channel 3 chopping burst) is unreliable on Manny's console and the DMA-based paths are suspect (memory `console-cd-dma-unreliable`), so the grid streamer must not introduce a new DMA read.
* Placement: `PagePool<PAGES, SLOTS>` (`streaming/page_pool.rs:58`): fixed pool of 2 KiB pages, sectors land at their final RAM address (no staging copy), generation-checked handles, compaction.
* Policy: `FailureBackoff`, `PinFlags`, `WindowCounters` (`streaming/scheduler_policy.rs`), already salvaged by `358728f2`.
* Prefetch order: `room_graph_ring` (6.1).
* Install: the region installer's per-sector FNV and relocation (`region_stream.rs` header) is for PXBSP; the grid needs **no relocation** (offsets are chunk relative), only the checksum, accumulated per landed sector so the work per pump stays bounded by the sector budget (byte-wise FNV-1a over 2048 bytes is not free, and I have not measured it on the R3000), and, when the render block is complete, texture upload requests through the VRAM owner and a drawable latch.
* Music: CD-DA dies on any data read and a music/data switch costs about a second each way on silicon **[measured, memory `silicon-v128-cdstream`: T_pause 123 ms, first data sector after audio about 945 ms, resume about 1.0 s]**. The scheduler must expose "streaming suspended" so the combat-radius music rule (memory `cortex-streaming-requirements`) can hold it, which also means prefetch has to finish before combat begins. This is a scheduler input, not a format feature.
* One cap for cook and runtime: `LevelHeader.pool_pages` is computed by the cook as the worst-case pinned-plus-prefetch ring and is a hard error above the platform cap. That is the lesson of the old streamer, whose cook allowed chunks the runtime silently refused (archaeology section 5 item 5).

Read time for the Cathedral chunk, **[derived]**: 53 sectors (7.4) at the measured 151.4 sectors/s at 2x (memory `silicon-v128-cdstream`) is about 0.35 s of transfer, plus seek, which I did not find measured for this pattern. The Cathedral is one room and would simply be pinned.

### 7.4 Sizes: Cathedral against the measured pack

Inputs **[measured]**: 1718 quads, 1883 vertices, 33 x 26 = 858 cells (dense upper bound). Record sizes from section 3.5.

| Part | Prototype `GRD1` | New PGRM | Source of the new number |
|---|---|---|---|
| Header | 64 B | 152 B | |
| Vertices | 15,064 B (1883 x 8) | 15,064 B | same record |
| Quads | 75,592 B (1718 x 44) | 68,720 B (1718 x 40) | **[derived]** |
| Render cells | 6,864 B (858 x 8) | 6,864 B | same |
| Macro cells | about 504 B (63 x 8) | 0 | removed |
| **Render total** | **about 98.1 KB** (matches the measured 98 KB) | **90,800 B = 88.7 KiB (-7.4%)** | |
| Sectors | n/a (collision came from PXBSP) | 17,160 B (858 x 20) | **[derived]** |
| Sector exts | n/a | up to 6,864 B (858 x 8), fewer in practice | upper bound |
| Walls | n/a | 12 B per solid wall piece; count not measured | |
| Material use list | n/a | 1 B per distinct material | |
| **Known total** | | **107,960 B = 105.4 KiB, 53 sectors** before ext and walls | **[derived]** |

With the RGB555 option (4.6, option B) the quad table is 54,976 B and the known total 94,216 B.

For scale against the BSP: the generated directory in this worktree currently holds a `brush_world.pxbsp` of 221,168 B written by the same cook, at the same minute, as a `grid_world.bin` of 30,392 B (`engine/examples/editor-playtest/generated/`, 21:14 on 16 Oct; another session's output, project unknown) **[measured]**. The PXBSP carries render surfaces, hull clipnodes and visibility together, so it is not a like-for-like of render data, but it shows the resident BSP map is several times the grid render pack for the same world. Adding about 20% of the render pack for dense collision records still leaves the grid well below the BSP **[estimate]**. Phase 1 reports both numbers for the Cathedral.

Per-frame scratch is **not** per room and does not stream: the prototype's `GridRenderer` holds, per vertex, a 2-byte stamp, an 8-byte screen entry and a 2-byte work slot, plus 2 bytes per cell and per quad for the queues (`grid.rs:351-380`, `working_bytes`). For the Cathedral that is 22,596 + 1,716 + 3,436 = 27,748 B **[derived]**. It is sized for the **largest room in the level** and reused across rooms (bump a stamp epoch between rooms), so the scratch cost does not scale with the resident set.

Resident set: `pool_pages * 2048` for chunks (cook-computed), plus the level core, plus scratch for the largest room. Cortex RAM is nearly full (memory `cortex-guest-heap-headroom`, 2026-09-04: heap about 20 KB on the shipping feature set), though Manny dropped the fixed 16 KB free-RAM floor on 2026-10-10 (memory `ram-floor-dropped`) and only a real stack/heap safety check remains.

---

## 8. Integer math

* No floats, no 64-bit in `psx-grid` runtime code (`tools/psoxide-dev` `runtime-numeric-guard`, `Makefile:277-291`, enforces this for the engine crates; add `engine/crates/psx-grid` to its root list explicitly, the BSP crate is not covered by it as I read the guard list). The cook and the editor run on the host and may use `i64` and `f64`; their output is integers.
* Sector lookup: reciprocal multiply (2.4). Height interpolation: weight multiply (2.5). No divides in the runtime query path.
* Normals: Q7 `i8` in quads, Q12 `i16` in contacts. Plane test uses the 32-bit dot product shifted right 7 (4.3). The one square root per contact is `isqrt_i32`.
* Platform height: `y = y0 + ((sx_q12 * dx + sz_q12 * dz) >> 12)`, with `dx, dz` bounded by the footprint (at most 255 units), so each product is at most 2^28.
* Mover lerp: `offset * progress / travel_ticks` with `progress` and `travel_ticks` as `u16`, computed as `(offset * progress) / travel` once per tick per mover (a divide, but per mover, not per vertex; per-vertex cost is one add).
* Camera-relative translation per room: three rotated components in the GTE, no software multiply.

---

## 9. Free planes ("flat planes you can place and stretch everywhere")

### 9.1 What exists today

`ImageProp` is already a flat textured quad with a bottom-centre anchor, pitch/yaw/roll, width and height, tint, baked corner light, optional cylindrical billboard and an optional collision AABB (`engine/crates/psx-level/src/lib.rs:1653-1695` `LevelImagePropRecord`; rendering and blockers in `engine/crates/psx-game-runtime/src/image_props.rs`). It is a scene node drawn through a separate prop path. That is the right tool for sprites, signs and billboards and it stays. What is missing is world geometry that behaves like the sectors: baked, culled, streamed with its room, lit by the same bake, drawn by the same renderer, and collidable as part of the world.

### 9.2 Document

A room owns a list of planes, integers only:

| Field | Type | Notes |
|---|---|---|
| `center` | `[i32; 3]` grid units | room-local, free (not on the sector lattice) |
| `size` | `[i32; 2]` grid units | width and height; stretch handles edit these |
| `rotation` | `[i32; 3]` degrees | yaw, pitch, roll; editor snaps to 15 |
| `material` | `Option<ResourceId>` | blend and cutout come from the material |
| `uv` | `FaceUv` | the existing offset/rotation/scale (`crate::brush::FaceUv`, used by `GridTri`) |
| `sided` | front or both | two-sided doubles nothing at runtime (one `DOUBLE` quad) |
| `collision` | none, platform, wall, blocker | 9.4 |
| `light` | baked or fullbright | fullbright writes `128` corners |
| `tag` | `u8` | mover/destructible group binding |

Likely additional needs, which I recommend deciding now because they change the record: **alpha** (foliage, grates, banners: `cutout` is a material flag, `blend` is a material mode, both already in `Material`, so no plane field is needed, but translucent planes need a separate draw list, 9.3), **UV scroll** (`uv_motion` on the material, free), **camera pass-through** (a banner or foliage the camera should not collide with: `ColPlane.flags` bit 0), **one-sided collision** (jump-through platforms: bit 1), **tint or per-corner colour** (a painted gradient on a banner: reuse the corner `color` the bake writes, with an authoring override), **group tag** (so a logic door can own planes), and **billboard** (stays with `ImageProp`, not here).

### 9.3 Cook and render

The cook computes the four rotated, stretched corners in `f64`, rounds each to the runtime lattice (`rt()`), appends them to `VERTICES` (never welded to the grid lattice, so a plane never tears a floor vertex), emits one `Quad` flagged `PLANE` (plus `DOUBLE` if two-sided), and a `PlaneRef` with its cell rectangle and vertical bounds. Light is baked at the four corners like any quad.

At runtime, planes are **not** in cell lists. Each frame the renderer frustum-tests every `PlaneRef` with the existing AABB test (`FrustumPlanes::cull_aabb`) and queues survivors' quads into the same select/project/emit pipeline. Planes are few, so a linear test is cheaper than index machinery; the per-room cap is 64 **[design choice, not measured; chosen so the linear test stays a small constant, to be revisited with a count from real levels]**. Opaque planes ride the normal path. Translucent planes (material `blend != 0`) are emitted with the semi-transparency bit and the blend mode in the packet, at their OT depth, after the opaque pass, so they never write ahead of geometry behind them.

### 9.4 Collision from planes

By cook-time classification of the plane normal (`ny` = normal y component), with the same 0.7 threshold the motor uses (2867/4096):

* **Platform** when `|ny| >= 0.7`: a walkable surface at any height over its footprint (bridges, balconies, lifts' decks, stairs made of tilted planes). Stored as footprint corners plus origin height and two Q12 slopes (`y = y0 + sx*dx + sz*dz`). It joins the cell's floor candidates (5.3 item 2): highest candidate at or below `feet + STEP_UP_HEIGHT` wins; its underside, at `extent` below, acts as a ceiling for bodies beneath it unless one-sided.
* **Wall** when `|ny| <= 0.35` (near vertical): a free-standing vertical segment with a bottom and a height, any angle. Tested like a `ColWall` but with arbitrary endpoints. This is what a diagonal fence or a bridge rail needs and the sector edges cannot do.
* **Blocker** otherwise: the plane's conservative AABB, handed to the existing `CharacterCollisionAabb` layer. It over-blocks and says so; the cook warns once per such plane.

A plane overlapping several cells is copied into each cell's run (28 B each; a 3x3 platform is 252 B) and carries an `id` so a trace that crosses two cells counts it once.

### 9.5 Editor

A Plane tool in the grid strip: place on a surface under the pointer (snap to cell centre, face, or free), drag handles to stretch width and height, rotation ring with 15 degree snap, gizmo reuse from `psxed-ui` `gizmo.rs`. Selection, undo, copy/paste go through the same grid edit layer (`grid/edit/`); planes need a `refs`-style selection entry. The 3D preview draws planes from the same cook output.

---

## 10. Salvage summary

| Item | Source | Verdict |
|---|---|---|
| Row-span cull, backface-before-projection, stamp table, GTE-batched projection, packet writer, `robust_quad` | `engine/crates/psx-bsp/src/render/grid.rs` (1107 lines) | **Keep**, move into `psx-grid`; helpers it borrows from `psx-bsp/src/render.rs` (`FrustumPlanes`, `clip_polygon_plane`, `load_gte_clip_planes`, `open_hall_bands`, `PXBSP_CLIP_ALL_PLANES`, `TEXTURED_GOURAUD_COMMAND`) must be extracted first (grid.rs imports them from its parent, `grid.rs:39-42`) |
| `floor_sample.rs` | `e1e55509` | Keep the math (2.5) |
| Motor grid backend, camera grid backend | `e1e55509`, `22894367` | Do not restore; keep as oracle (5.1) |
| `portal_visibility.rs` and 27 tests | `e1e55509:engine/crates/psx-level` | Port (7.2) |
| `room_graph_ring` | `e1e55509:...room_streaming.rs:1531` | Port (6.1) |
| `PagePool`, `FailureBackoff`, `PinFlags` | already on HEAD | Reuse |
| `RegionRead` / psx-cdstream | already on HEAD | Reuse as transport |
| `WORLD.PAK`, `LevelWorldPackEntryRecord`, `playtest_disc.rs` chunk plumbing | HEAD | Reuse as container |
| `.psxw` v5 and its parser | SDK repo | Not the format; leave alone here |
| `RoomResidencyRecord` | `e1e55509:engine/crates/psx-level/src/lib.rs:1424` | Cooked and never consumed in the old system (07-25 audit); replaced by `pool_pages` and the ring |
| `grid/faces.rs`, `grid/light.rs`, `grid/limits.rs`, `grid/edit/*` | HEAD | Keep as the document-side half; `light.rs` gains the shared LOS occluder |
| `brush_grid/collision.rs` (thin slabs, sky lid) | HEAD | Delete once `GridTraceProvider` lands |
| `brush_grid/mod.rs` (GRD1 writer) | HEAD | Replaced by the PGL cook; its `uv_overflow` check and corner-snap code move over |

---

## 11. Implementation order, with tests

Each phase leaves the editor and Play building and green. Gates are the pass criteria; a gate that needs a number says which tape and which counter.

**Phase 0: crate and extractions (no behaviour change).**
Create `engine/crates/psx-grid` (`no_std`, `std` feature for the writer and cook helpers; psxed-project already path-depends on engine crates, so the editor can use it: `editor/crates/psxed-project/Cargo.toml` lists `psx-bsp`, `psx-engine`, `psx-level`). Move `render/grid.rs` and the render helpers it needs out of `psx-bsp/src/render.rs`; move `psx_bsp::sky` to a world-neutral crate (`sky_runtime.rs` and `playtest/budget.rs` import it). Add `psx-grid` to the numeric guard's root list.
Tests: existing grid.rs unit tests (`pack_bytes`, `a_well_formed_pack_parses`, `grid.rs:1028` onward) pass unchanged in the new home; prototype `GRD1` still drives Play.

**Phase 1: format and cook.**
`psx-grid` format structs with size asserts and `layout_hash`; reader (`RoomView`, `validate_full`); writer. `psxed-project/src/grid/cook/` builds core and chunks from `GridWorld`: snapping with `rt()`, lattice, cell bucketing, collision tables, portal derivation, `pool_pages`, name sidecar.
Tests: round-trip every section on synthetic worlds; determinism (cook twice, identical bytes); golden bytes for a tiny fixture per version; malformed-input suite (truncated, misaligned, offset past end, count overflow, bad magic, layout hash mismatch) all refused without panic; cap errors (`S` not integral, room too wide, cell over 255 quads, no reciprocal `k` exact for the room extent); Cathedral report prints the section sizes of 7.4 and the current resident PXBSP size for the same project.

**Phase 2: renderer on PGRM, still on PXBSP collision.**
Port the renderer to read `RoomView`; per-room translation and plane translation; stamp epoch; single room first.
Gate: render equivalence. Cook the Cathedral both ways and replay the poll-bound route tape (`/Users/ebonura/Desktop/repos/PSoXide-editor-wt/open-hall/build/oh-runs/grid/route.pxtape`, 2400 polls; the memory note says old `--press` routes drift per build, so use the tape); the GPU frame census should match the prototype frame for frame, or differ only by a reviewed list (quad order inside a cell can change OT insertion order at equal depth). Then cycles: world cost within 5% of the prototype's 237k to 280k **[the 5% is my proposed bar]**.

**Phase 3: native collision.**
`height_at_local`, `locate`, `floor_at`, `ceil_at`, `wall_hits`, `contents_at`, `line_of_sight`, `GridTraceProvider`.
Tests: exhaustive height-function test against the i64 reference (2.5); editor `floor_height_at` now calls the shared function and a test asserts equality over every cell of the Cathedral; oracle tests port the old `circle_overlaps_segment`, `wall_blocks_body`, footprint sampling from `e1e55509` as expected-behaviour tables; property tests (a trace never ends inside solid; fraction is monotonic in segment length; `start_solid` agrees with the discrete overlap test); the motor's and camera's own provider-based tests (`character_motor.rs` tests at `:3133-3560`, camera tests) run against a flat grid provider.
Gate: Cathedral route tape walk with the Reach character through `GridTraceProvider` reaches the same waypoints as the BSP run, and the per-tick collision cycles are reported next to BSP's.

**Phase 4: `GridRuntime` replaces `BspRuntime` services.**
Section 6 in this order: materials and draw, activation, liquids, line of sight, movers and destructibles, sky gate. Keep the PXBSP path compiling behind the world-kind switch until parity, then remove it (deletion review, order of removal).
Gates: Makefile acceptance targets rebuilt for grid projects: the equivalents of `editor-blank-playtest-check`, `editor-bsp-liquid-check` (lava damage count and respawn frame) and `editor-souls-bsp-check` (checkpoint, door, kill, lava, respawn). A 90-tick walk into a lava pool must reproduce the 40 hits and one respawn the liquid gate pins today (`Makefile:1016-1045`).

**Phase 5: rooms, portals, streaming.**
Portal traversal port with its 27 tests; `locate` across rooms and stacked rooms; `room_graph_ring`; pool and scheduler wiring through `WORLD.PAK`; drawable and collidable latches.
Tests: Dungeon (2 stacked rooms, 90 bonnie portals) cooks, derives portals, and walks across both rooms; a pool-exhaustion test (cook refuses); a prefetch counter test (the old streamer's prefetch counter was dead, archaeology item 7); a forced-failure test (`PSOXIDE_WEDGE_*` knobs exist for DMA and GPU, memory `console-cd-dma-unreliable`) showing backoff and no hang.
Gate: a silicon run before any claim about streaming on hardware (memory `no-console-ab-discs` says measurements go into the hardware-tests suite, not bespoke discs).

**Phase 6: LOD and the clip path.**
Merge pass in the cook, runtime selection, near-field measurement (4.7).
Gate: Cathedral frustum quads at p50/p90 on the route tape against the 147/186/243 target of 4.8; world cycles p90 against the measured 280k.

**Phase 7: free planes.**
Document type, tool, cook, render list, collision classes, editor preview.
Tests: plane corner exactness (rotation and stretch produce the expected lattice vertices), classification thresholds, a bridge walk (platform over a pit), a fence block (free wall), foliage cutout and a translucent banner in the OT order test.

**Phase 8: editor parity.**
The editor's 3D preview calls the same `psx-grid` renderer on the host as its truth source (the preview already links `psx_bsp::sky`, so linking engine crates is established: `emu/crates/frontend/src/editor_preview.rs:71-74`), and MCP screenshots show what the guest draws.

---

## 12. Perf expectations

| Quantity | Value | Tag |
|---|---|---|
| World render, Cathedral, prototype | 237k to 280k cycles/frame | measured |
| Per-quad cost | about 800 cycles averaged over a frame on the Cathedral; fast-path packet about 430, split triangle about 9.2k | measured / derived (4.7) |
| Same renderer on PGRM, no LOD | within a few percent of the prototype; the change is smaller records and fewer cache misses on the queue, not fewer instructions | estimate |
| 40 B vs 44 B quad | 4 B less read per tested quad; effect unmeasured | estimate |
| RGB555 option | about 4% more emit cycles for 20% less quad memory | estimate (4.6) |
| With LOD, Cathedral p90 | about 149k cycles of world instead of 742k | derived from analyzer counts |
| Dedicated 2x2 near split instead of the generic path | about 2k cycles each against 9.2k; about 62k saved on level_008 and about 178k on Cave | estimate built on an estimate (4.7), the first thing to measure |
| Portal traversal | single-digit thousands of cycles per frame | measured in the old system (8,512), different era |
| Collision trace cost | unknown | gate in phase 3, no number given |
| Frame budget at 30 fps | 1,128,960 cycles | derived |
| Today's end-to-end | 25.9 to 26.5 fps, mean 920k, p90 1.33M | measured |

The honest reading: world rendering is not what keeps Cortex under 30 fps in the Cathedral. The player (about 470k) plus sky (about 58k) plus the world (about 280k) is about 808k before everything else, and the measured mean is 920k with p90 above the slot. In the four corridor and cave runs the picture is worse for the world: the world is 140k to 335k, and most of it is the split path. The format and runtime in this document make the world cheaper and shrink its tail (LOD), and they must not regress collision cost. They do not by themselves reach 30 fps; the character cost does that work (plan.md section 6, third risk).

---

## 13. Decisions and open questions for Manny

1. **Default scale.** 21/16 (S = 84, fine snap inexact) as decided in D1, or 5/4 (S = 80, every snap integral, character 0.80 of a sector). Recommendation: 5/4 for new worlds, keep 21/16 supported.
2. **Step height.** Make `STEP_UP_HEIGHT` a motor config field derived from the world click (default: current 40). Touches shared `psx-engine`; harmless to quake-psx and hl-psx because they pin an editor revision.
3. **Light encoding.** Ship `[u32; 4]` (measured) or move to RGB555 (smaller, about 4% more emit work, an estimate). Recommendation: ship `[u32; 4]`, decide by a phase 3 measurement.
4. **Validation policy.** Guest checks header and checksum only; full index validation on host and debug guests. Alternative: validate in the guest at install, one pass over quads per room, a cost I did not measure.
5. **Movers.** Translation only, via vertex patching. Is a swinging door needed in the first Cortex levels? If yes it is a prop, not world geometry.
6. **Free-plane cap and collision classes.** 64 per room and the three collision classes of 9.4 are my choices. The foliage and banner cases need Manny's call on whether translucent planes may overlap the player's silhouette (OT ordering against model packets).
7. **Rooms larger than a streamable chunk.** The cook currently errors above the pool; auto-splitting a big room into row bands is possible later without a format change (a chunk per band, same room record) but is not designed here. The Cathedral at 105 KiB is the largest sample.
8. **`psx-bsp` crate fate.** `psx-grid` takes over the renderer, `sky` moves out, but the PXBSP crate cannot simply vanish while quake-psx pins and builds it and while the materialized emulator frontend lists `psx-bsp` as an optional workspace dependency (`emu/crates/frontend/Cargo.toml:17,39`). See the deletion review.

---

## 14. Risks

* **Collision parity.** The motor's feel (steps, ledges, slopes) is tuned against hull traces. The grid provider uses footprint sampling plus segment tests. The old grid backend lived under the motor's test suite with a similar rule set (89 tests, mixed grid and policy, archaeology section 2.5), which is evidence the rules are attainable but not that the new trace layering matches. Mitigation: phase 3 oracle tests and the route-tape walk.
* **Trace cost.** Up to about five traces per moving body per tick, each possibly several cell steps. Unmeasured. Mitigation: pattern fast paths (5.4) and a hard gate.
* **Near-plane path.** Unquantified hot spot; the plan is measure first, then specialise (4.7).
* **Single big room.** The Cathedral is one activation unit and one chunk; streaming and portals do nothing for it. The only levers are LOD and fog/far cull.
* **Stacked rooms and `locate`.** Stacking makes cell lookup ambiguous in plan; correctness rests on the hint room and vertical comparison. Needs the Dungeon as a test.
* **Scale rounding.** Fine-snapped heights round at cook (2.2). Low risk, but the editor must show the rounded value or authors will be confused.
* **Shared-crate coupling.** `psx-engine` is consumed by hl-psx and quake-psx via a pinned revision; any change to it on `grid/main` stays safe only while they keep pinning. Do not merge `grid/main` into `main` wholesale (deletion review).
* **Materialized upstreams.** The SDK repo owns `.psxw` and `mkisopsx`; the emulator repo owns the frontend files that call editor symbols. Changes there need a revision bump in `components.lock.json`.
