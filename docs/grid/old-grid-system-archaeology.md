# Old grid-room system: archaeology for the bonnie-32 editor port

Read-only recovery, 2026-10-16. Repo for every sha below: `/Users/ebonura/Desktop/repos/PSoXide-editor-wt/open-hall` (shares history with `PSoXide-editor`). Nothing was checked out, built or run. Read any file with `git show <sha>:<path>`.

Method: `git log --all -S/-G/--diff-filter=D`, `git ls-tree`, `git show --stat`, the docs that were in the tree at the last grid commits, and the memory notes. Every sha below was confirmed to be an ancestor of `main` (`git merge-base --is-ancestor`). I did not build any snapshot, so "last commit where it existed" means "last commit where the file was in the tree", not "last commit where the whole pipeline was verified to compile". Where a claim comes from memory notes only, it says so.

One oddity: the 10-08 deletion series exists twice, once authored as "Emanuele Bonura" (reachable from `main`, use these) and once as "ebonura-fastly" (same subjects, different shas, on no branch, e.g. `e96ba97f` = `637cb4a4`, `4234e059` = `c81f1235`). Ignore the ebonura-fastly copies.

## 0. Which sha to read what at (the short version)

| What you want | Read at | Why |
|---|---|---|
| Editor with the grid paint tools still on the toolbar, plus cook, plus preview | `fc3129dc` (2026-08-22) | Parent of `d4d1e10c` "Simplify BSP editor authoring controls", which made the paint tools dead code ("Retired LegacyGrid tools remain decodable") |
| Whole grid pipeline in one coherent snapshot (editor model, cook, preview, runtime, tests) | `cf154ab4` (2026-08-25) | Parent of `174263f8` "consolidate BSP authoring", which deleted `world_cook*`, `cook_world.rs`, `cook_visibility.rs`, `prefab.rs`, `sector_inspector.rs`, editor_preview room geometry, the cortex_v1 sample and the grid starter fixture. Tool enum still present but unreachable from the toolbar |
| Runtime, project model, motor, camera, streamer, docs | `e1e55509` (2026-10-08 11:01) | Parent of the first 10-08 deletion. Runtime files are byte-near-identical to `cf154ab4` (diff: motor +923, camera +2392 lines of unrelated work, portal_rooms +2 lines) |
| SDK-side format and reader | SDK repo `/Users/ebonura/Desktop/repos/PSoXide` `main` (`84a220abc`) | `.psxw` writer types `crates/psxed-format/src/world.rs`, reader `sdk/crates/psx-asset/src/lib.rs` `World<'a>` (:2453), `WorldSectorFloorCollision` (:3007), 102 mentions of "world" in `sdk/crates/psx-asset/src/tests.rs`. These were never deleted |

Mixing is safe for the runtime half. For the editor half, take the toolbar and painting code from `fc3129dc` and the cook/preview from `cf154ab4`.

## 1. Timeline

Dates are author dates. Subjects are verbatim.

### Birth (April to May)

| Date | sha | Subject |
|---|---|---|
| 2026-04-26 | `3867e25e` | editor: World/Room hierarchy + paint tools, sector inspector, undo, cook |
| 2026-04-27 | `c56614fe` | editor: Phase 1, render the active Room through the host GTE |
| 2026-04-27 | `789b6b30` | editor: Sims-style 3D tool dispatch, paint by click on cells |
| 2026-04-27 | `ba2732d9` | editor: TR-pass P1-P4, paint replace bug, edge-aware dedupe, i16 vertex safety, diagonal cook reject |
| 2026-04-27 | `ce1c3310` | editor: TR-pass P5-P9+P11, room budget UI, .psxw v2 records, RuntimeRoom wrapper |
| 2026-04-27 | `544a7103` | editor: TR-pass P10+P12+P13, render/collision facades, tests, design notes |
| 2026-04-27 | `b39d5c5b` | engine: showcase-room, render a cooked .psxw on PSX |
| 2026-04-27 | `62736f01`, `ede939af`, `728f9382` | playtest pipeline, cook-playtest CLI, Cook & Play menu |
| 2026-04-28 | `b53dfced` | editor: vertex-height quantum (32 units), snap on drag, reject at cook |
| 2026-04-28 | `cdf92a40` | editor: vertex / edge / face selection with universal coincidence |
| 2026-04-28 | `b2af17f5` | engine: psx-level, master asset table + per-room residency contract |
| 2026-04-29 | `dbb414a1` | editor: add playable character movement (motor + third-person camera) |
| 2026-05-03 | `0aa1d4e0` | Embed baked room lighting in psxw |
| 2026-05-08..13 | `621cb879`, `dac8c4d3`, `085b3cfe`, `31734205` | cached-room renderer passes (GTE NCLIP culling `31734205`) |
| 2026-05-12 | `e2441bee`, `e34ba3e4` | Add disc streaming validation path; Stream compact room collision chunks |

### Portals, streaming, vertical rooms (May to July)

| Date | sha | Subject |
|---|---|---|
| 2026-05-21 | `541928a7` | Implement portal room streaming for demo7 |
| 2026-05-21 | `fdb40f10` | Expose portal map editing UI |
| 2026-05-22 | `e8fe9ec1` | Fix Tomb-style portal visibility |
| 2026-05-24 | `1c6b4385` | Import TR rooms and flatten world hierarchy |
| 2026-05-30 | `ea62a115` | feat(playtest): node-graph BFS-ring room streaming |
| 2026-05-30 | `e648c4f2` | refactor(engine): delete the dead 1024-hardcoded GridRoom/GridWorld path |
| 2026-06-01 | `9adc53a2`, `da438b98`, `cf44f188`, `89067205`, `cbb4bc0f`, `406a8714` | vertical-room floors: model, UI, Sims-style floor view, runtime elevation, stair-stepping, stacked-floor collision |
| 2026-06-05 | `b0be1880` | refactor(psxed-ui): split 45k-line lib.rs into 24 focused modules |
| 2026-06-11 | `20ca6286` | engine: portal-anchored PVS for far rooms, all-cells fallback that draws |
| 2026-06-12 | `e0ffdf5d`, `8dfa019f` | docs: streaming system audit, phase 1; streaming hardening |
| 2026-07-11 | `b3b809f6`, `fba15f0f`, `4ccb8664` | psx-game-runtime: game layer carved out (cd_stream, room visibility + cache, room window + stream scheduler) |
| 2026-07-15 | `7d901a19` (file add), `45fd3659`, `d295376b` | gen-stress-map harness; Enforce the cook/runtime streamed-room size contract; vis-draw-pruned |
| 2026-07-16 | `304cdd51`, `cdf893fe` | Redesign seamless room streaming cache; Stream persistent model and animation assets |
| 2026-07-25 | `625a8d04` | Audit room streaming against the seamless-world goal |
| 2026-07-26 | (docs) | `docs/cortex-v3-render-architecture-2026-07-26.md`, `docs/cortex-30fps-experiments-2026-07-26.md` |
| 2026-08-01 | `04b57e3d` | Active-room window: per-tick reconciler replaces event-triggered rebuilds |
| 2026-08-03 | `92d2fe21`, `65b30cd8` | prefab kit, prefab workflow |

### Pivot and retirement (August to October)

| Date | sha | Subject |
|---|---|---|
| 2026-08-10 | `5daf3b80` | docs: quake BSP world migration plan (the decision, with a "Why this wins (measured)" section) |
| 2026-08-10 | `c309f37a` | docs: add legacy grid boundary decision artifact |
| 2026-08-11 | `e4031c39`, `466b4c37`, `202cf176` | fail closed when a BSP project touches grid spatial state; remove synthetic BSP grid ownership; record the enforced boundary |
| 2026-08-13 | `1fe20688` | editor: archive legacy grid projects |
| 2026-08-16 | `9560d91d` | projects: only default + tech-demo stay; fixtures and legacy grid move to editor/archive |
| 2026-08-22 | `d4d1e10c` (parent `fc3129dc`) | Simplify BSP editor authoring controls (grid tools dead) |
| 2026-08-25 | `174263f8` (parent `cf154ab4`) | feat(editor): consolidate BSP authoring and scene skies (grid cooker, preview, sector inspector, prefabs, cortex_v1 sample removed; 1085 files, -60714 lines) |
| 2026-09-05 | `b8506c54` | Prune grid-only guest paths from cooked BSP builds |
| 2026-09-26 | (memory only) | desloppify: "nuke the grid world everywhere incl. the editor option"; engine WP2 "grid retire" |
| 2026-10-04 | (memory only) | Manny: delete grid rooms entirely, queued as D14 |
| 2026-10-08 | `e1e55509` | Add the world streaming survey and design (LAST commit with the full runtime + project model) |
| 2026-10-08 11:20 | `358728f2` | Salvage the page pool and scheduler policy from the grid room streamer |
| 2026-10-08 11:31 | `637cb4a4` | Delete the grid room window, visibility and streaming runtime (-11020 lines) |
| 2026-10-08 12:07 | `c81f1235` | Drop the grid tables from the cooker, manifest and psx-level (-15130; takes `world_render*`, `portal_visibility.rs`) |
| 2026-10-08 12:14 | `76272822` | Remove the grid world from the project model (-6195; takes `world_types.rs` body, `scene_grid_types.rs`, `portal_rooms.rs`, `room_connections.rs`, `floor_view.rs`, `gen_stress_map.rs`) |
| 2026-10-08 12:20 | `2091fb6e` | Express the enemy leash and logic gating without grid rooms |
| 2026-10-08 12:26 | `26f62b39` | Delete the engine's cached-room renderer and its dead projection paths |
| 2026-10-08 12:29 | `bbf6fbb9` | Mark the grid docs superseded and drop the removed visibility features from the Makefile |
| 2026-10-08 12:37 | `22894367` | Delete the grid collision backends from the character motor and third-person camera (-4668; takes `world.rs`, `floor_sample.rs`) |
| 2026-10-08 12:40 | `5b683996`, `fad384fc` | Rewrite project crate tests for a BSP-only world and add legacy-grid load tests; strip removed keys from tracked projects |
| 2026-10-08 12:55 | `57dd4303` | Remove grid authoring from the editor UI and frontend (-11114; takes `psxed-ui/src/geometry.rs`) |
| 2026-10-08 14:07 | `5c67b01b` | Remove the dead grid runtime render knobs from ProjectDocument |
| 2026-10-09 | `a54e8d77` | Stream brush worlds by region from the CD (the BSP replacement for the room streamer) |
| 2026-10-16 | `0e64a92b`, `5d93e571`, `64faaa1f`, `29704ca5` | bonnie-32 importer as BSP brushes; WIP grid world renderer prototype (`brush_grid.rs`, `psx-bsp/src/render/grid.rs`); Bake grid corner light; telemetry for both world renderers |

## 2. Architecture of the old system

### 2.1 Data model (editor, `psxed-project`)

Last read at `e1e55509`, with `cf154ab4` for the cook.

- `WorldGrid` (`editor/crates/psxed-project/src/world_types.rs:867`): `width`, `depth`, `sector_size` (engine units, default 1024, Cortex used a larger value, memory says 1664), flat `sectors: Vec<Option<GridSector>>` indexed `[x * depth + z]`, `origin: [i32;2]` (cell units, lets the grid grow into negative X/Z), `elevation` (room Y), ambient/fog/atmosphere settings, and `floors_above: Vec<WorldGrid>` for stacked floors.
- `GridSector` (`scene_grid_types.rs:2830`): optional `floor` and `ceiling` (`GridHorizontalFace`: 4 corner heights `[NW,NE,SE,SW]`, a diagonal `split`, material, UV transform, `walkable`, per-triangle overrides, `dropped_corner` to make a triangle), `walls` per direction (`GridVerticalFace`: 4 heights, material, `solid`; N/E/S/W stacks plus two diagonals), and `floor_above` / `floor_below: Option<GridFloorLink>` for vertical traversal.
- Caps (`scene_grid_types.rs:3050`): `MAX_ROOM_WIDTH/DEPTH = 32`, `MAX_WALL_STACK = 4`, `MAX_ROOM_TRIANGLES = 2048`, `MAX_ROOM_BYTES = 64 KiB`. 32 x 1024 = 32768 is the i16 cliff the GTE imposes on room-local vertices (`max_room_cells_for_sector_size`). Heights quantised to 32 units (`b53dfced`).
- Authoring rule: one contiguous `WorldGrid` per Room node. Runtime "portal rooms" exist only where the author placed `Portal` scene nodes on grid edges (`portal_rooms.rs` header: "the planner must not invent chunk boundaries for size, walls, or streaming"). `plan_portal_rooms` then splits and enforces the caps.
- Wall ownership: the wall between `(x,z)` and `(x+1,z)` is `East(x,z)` and `West(x+1,z)`; the cooker rejects a grid that authors both (`DuplicatePhysicalWall`). Diagonal walls are rejected at cook.
- Extras on the same model: water volumes (`WaterVolumeSettings`, cell lists), far-vista and sky settings, atmosphere particles, box/cylinder/arch/image props, lights, Place-able entities as child nodes.

### 2.2 Editor UX (`psxed-ui`, snapshot `fc3129dc`)

- `ViewTool` (`psxed-ui/src/lib.rs:2400` at `fc3129dc`): `Select`, `PaintFloor`, `PaintWall`, `PaintCeiling`, `PaintMaterial`, `Water`, `Erase`, `Place`, `Brush`.
- Paint by click in the 3D viewport via face pick (`71d044e9`), green ghost previews (`19ddfcf7`), paint outside the grid auto-grows the room including negative side (`93776009`, `e89b51be`), Sims-style 3D tool dispatch (`789b6b30`), select faces/edges/vertices with welded coincidence and drag-to-raise (`cdf92a40`, `5180bb9f`), delete primitives, `R` rotate / `F` flip / PageUp/PageDown elevation during stamp and duplicate (input.rs ~746-766), 2D top view (`viewport2d.rs`), sector inspector (`sector_inspector.rs`, 580 lines) with slope and stack controls, room budget panel that tints over-cap metrics red (`WorldGridBudget`).
- Floors: `floor_view.rs` (single floor-resolution source of truth, `2bd4dbe5`), active-floor up/down buttons, ghost of floor N-1, auto-stack above the lower ceiling, hole-gated vertical portals. Design docs: `docs/floors-plan.md`, `docs/floors-editor-architecture.md` (`e1e55509`).
- Portals: portal seam placement across connected seams, full-plane outlines, portal debug snapshot, room map as streaming dashboard (`fdb40f10`, `8f53a58f`, `19962525`, `0dd00e99`).
- Prefabs: `prefab.rs` (609 lines, `cf154ab4`) = the geometry clipboard with serde, saved to `editor/prefabs/<name>.ron` (20 files at `cf154ab4`: arena_5x5, boss_arena_9x9, stair_*, connector_*, ...). Stamping reuses the Duplicate preview loop. Decisions in memory `prefab-stamping-decisions`: incoming wall loses at a seam (`WorldGrid::strip_seam_walls`, `world_types.rs:1018`), portals are NOT auto-emitted per stamp (cortex_v1 has 7 portals for 1581 cells, so per-piece portals would shred a level).
- TR level importer: `1c6b4385` "Import TR rooms and flatten world hierarchy".
- Preview: `emu/crates/frontend/src/editor_preview/room_geometry.rs` (1139 lines), `portals.rs`, `visible_rooms.rs`, `tests.rs` (68 tests) at `cf154ab4`.

### 2.3 Cook and format

- Editor to engine: `WorldGrid` -> `CookedWorldGrid` (validated, material slots resolved, empty sectors stripped) -> `.psxw` blob (`AssetHeader` magic `PSXW`, `WorldHeader`, `SectorRecord[]`, `WallRecord[]`, horizontal overrides, per-surface baked lights). Doc: `docs/world-grid-architecture.md` (`e1e55509`), roadmap for a compact TR-style i16 room-relative format: `docs/world-format-roadmap.md`.
- Files at `cf154ab4`: `editor/crates/psxed-project/src/world_cook.rs` (2151, 30 tests) + `world_cook/{coords,encode,materials,validation}.rs`; `playtest/cook_world.rs` (1352); `playtest/cook_visibility.rs` (909, per-cell PVS rows, `append_room_visibility`); `playtest/manifest.rs` (room chunks, WORLD.PAK TOC, `validate_streamed_room_chunks`).
- Single editor/runtime coordinate boundary: editor North is +Z, runtime `.psxw` North is -Z; `world_cook` flips directions, corners and split ids in one place (`docs/editor-runtime-coordinates.md`).
- psx-level record families (`engine/crates/psx-level/src/lib.rs` at `e1e55509`): `LevelRoomRecord`, `LevelRoomPortalRecord` (full 3D quad), `LevelChunkRecord`, `LevelWorldPackEntryRecord`, `LevelRoomVisibilityRecord`, `LevelVisibilityPvsRecord`, `RoomResidencyRecord` (required/warm RAM and VRAM sets per room, cooked and never consumed per the 07-25 audit), compact collision header.
- Disc side kept alive: `PSOXWPAK` world pack (`psx-iso build_world_pack`), shared by hl-psx, oot, alttp, wipeout, so it was not grid-only.

### 2.4 Runtime render

- Pipeline: `psx_asset::World` (zero-copy) -> `psx_engine::RuntimeRoom` (`engine/crates/psx-engine/src/world.rs`, 1539 lines, `Copy`, `.render()` and `.collision()` facades) -> `world_render.rs` (3322 lines at `e1e55509`; module header says it walks a `RoomRender` and emits floors/ceilings/walls) -> cached-room path `world_render/indexed_cache.rs` (3096), `cache_build.rs`, `room_draw.rs`, `render3d/world_pass_gouraud.rs` (2364, partly kept).
- Culling and visibility: Tomb-style portal traversal `engine/crates/psx-level/src/portal_visibility.rs` (2204 lines, 27 tests; clips each directed portal to the screen-space window, recurses with the clipped window); per-cell PVS rows from the cook, `world_cells.rs` (`VisibleCellSelector`, 1096), `room_visibility.rs`, `world_visibility.rs` (cell-accurate room containment for L-shaped portal rooms), GTE NCLIP culling.
- Surface quality tricks: TR subdivision lattice (feature `tr-subdivision-lattice`, adaptive subdivision "wins 4:1" per memory `ps1-texture-warping-measured`), prebuilt quad packets (`PrebuiltRoomQuads`, `room_cache.rs`), texture-window room tiling, baked vertex light embedded in `.psxw`, fog depth cue.
- Window/residency: `room_window.rs` (`RoomWindow`, 623) and `editor-playtest/src/active_rooms.rs` (832) keep an "active room window" resident and draw order by visibility.

### 2.5 Collision

- Sector addressing is O(1): world X/Z divided by sector size gives the cell, floor height is interpolated from 4 corners + diagonal split (`floor_sample.rs`, 68 lines, shared by the motor and camera so they agree exactly), walls are a short list per cell. API: `RoomCollision` (`sector_probe`, `sector_floor_collision`, `sector_probe_wall`), compact collision chunks streamed from CD (`e34ba3e4`).
- Consumers: `character_motor.rs` (6084 lines, 89 tests at `e1e55509`; the grid backend was ~1960 lines removed by `22894367`; policy layer kept), `third_person_camera.rs` (4728, 63 tests), enemy mover through `GameEntityMover` trait.
- Vertical traversal: `floor_above_room` / `floor_below_room` per sector consumed in `editor-playtest` room switching; `docs/floors-plan.md` records the round-6 bugs (switch condition wrong for stairs, Y not rebased into the new room frame).

### 2.6 Streaming

- `RoomStreamScheduler<N,M>` + `StreamedRoomPages<PAGES,SLOTS>`: `engine/crates/psx-game-runtime/src/room_streaming.rs` (1690 lines, 4 tests) at `e1e55509`. Fixed pool of 2 KiB CD-sector pages with generation-checked handles, slots LRU among unpinned, pinned = desired window, failure backoff 16 to 512 windows (never abandoned), protected prefix so prefetch cannot evict, `room_graph_ring` (portal-hop BFS ring, `RESIDENT_DRAW_DEPTH` and `RESIDENT_PREFETCH_HOPS` in `editor-playtest/src/active_rooms.rs`, values 3 and 2 per memory), one load job in flight, non-blocking pump.
- Transport: `cd_stream.rs` `WorldRoomSlotsReadJob` (polled, ch3 DMA), still alive on `main` for UI/model assets.
- Audit verdict 2026-07-25 (`docs/streaming-audit-2026-07-25.md`): "The room-geometry streamer is well built and is not the problem. It is non-blocking, budgeted, LRU-evicted, and it degrades by not drawing rather than by stalling."

### 2.7 Perf numbers on record

All emulated cycles unless noted. Cycle model changed over time (BIAS, I-cache), so compare within an era only.

- 2026-06-11 (`docs/perf-30fps.md`): Cortex render vblank 1,126k (199%), ~20 fps with correct visuals. Visibility fix: 333 tris vs 341 draw-everything.
- 2026-07-15 stress-map findings (memory `stress-map-harness-and-findings`): room band about **1.6k cycles per considered surface** (cortex 86k/52 surfaces, corridor 822k/540). Straight open sightline through 14 full-width portals: nothing culls, render avg 1,708k per vblank (151% of the whole 30 fps slot), room band alone 1,254k. Engine had no distance cut at that time.
- 2026-07-26 (`docs/cortex-v3-render-architecture-2026-07-26.md`): cost fit 5,608 to 7,020 cycles of fixed decision tax per surface (cortex_v3, 88.9 surfaces per frame = 499k to 624k of 804.6k room draw), inner loop `draw_indexed_cached_room_vertex_lit_visible_cells` compiled to 35,488 bytes vs a 4 KB I-cache. cortex_v1 gameplay 28.80 fps on an older build and different route. Also: cortex_v3 14.01 fps.
- Plan doc 2026-08-10 (`docs/quake-bsp-migration-plan.md`): cortex_ignition_v1 render vblank about 843k, 30 fps slot about 102% full, open sightlines up to 1.7M cycles per vblank.
- Cortex room data: 8 rooms, whole world 54,220 bytes, largest room 9 sectors (07-25 audit); cortex_v1 51 x 31 cells = 1581, 7 portals.
- For comparison (memory, BSP era): render about 734k + 1,856 x visible faces, 30 fps about 150 faces, 20 fps about 420 (`engine-stress-limits-2026-10-03`). 10-16 Cathedral race with the Graybox Reach character: BSP 23.8 / 25.2 fps, grid prototype 25.1 to 26.0 fps, grid world about 250k to 300k cycles vs BSP 377k to 460k, character about 460k per frame is now the bottleneck (memory `cortex-grid-engine-preference-2026-10-16`).

Uncertainty to flag: the 1.6k per surface figure (07-15) and the 5.6k to 9k figures (07-26) differ by 3x to 5x. They are different projects (v1 vs v3), builds and cycle models, and I did not reconcile them. The 1.6k figure is the one Manny's note cites.

## 3. Why it was deprecated (quoted, not inferred)

1. The written decision, `docs/quake-bsp-migration-plan.md` (`5daf3b80`, read at `e1e55509`): "Replace the sector-grid ("Tomb Raider style") world with a brush/BSP world across the whole stack, modelled directly on quake-psx". Section "Why this wins (measured, not vibes)": cortex_ignition_v1 render vblank about 843k cycles, "open sightlines up to 1.7M cycles/vblank because the grid world has no PVS and no distance cut"; quake-psx Episode 1 averaged 747k with more complex geometry. Stated cause: "The difference is architecture: PVS bounds the visible set no matter the map, faces are precompiled render-ready, lighting is baked at cook, and the loop free-runs with variable dt so overload degrades smoothly instead of missing fixed deadlines."
2. Diagnosis in `docs/cortex-v3-render-architecture-2026-07-26.md`: "The renderer's cost is not proportional to what it draws. It is proportional to how many cached surface records it walks past." A surface was "a record to be interpreted at draw time" instead of a precompiled draw command. Portal traversal itself was cheap (8,512 cycles per frame, 0.6% of render).
3. Entity coupling, plan doc: entities lived in "ROOM-LOCAL coordinates with `RoomIndex` everywhere", which the BSP world wanted as world space with leaf/PVS activation. Owner locked a world-space move.
4. Boundary audit `docs/legacy-grid-boundary.md` section 2.8: the BSP cook "does not bypass the grid pipeline; it feeds it a stub" (a synthetic 1x1 grid, zero-byte `room_000.psxw`, one synthetic `PlaytestRoom`). "This is the main "BSP silently rides on grid" discovery of the audit." Fixed by `466b4c37`.
5. Owner decisions (memory notes, not in the repo): 2026-08-10 "existing projects frozen on grid (no port, next game starts fresh)" (`quake-bsp-world-migration`); 2026-09-26 "nuke the grid world everywhere incl. the editor option" (`desloppify-campaign`); 2026-10-04 grid rooms deprecated and "must be removed entirely", prompted by Manny "correcting a Discord draft that pitched grid-room CD streaming as a capability" (`grid-rooms-deprecated`); 2026-10-08 "confirmed the ENTIRE legacy grid world can be removed now ... salvage reusable streaming pieces first" (`cortex-streaming-requirements`).
6. I did not find a recorded personal reason from Manny for 10-04 beyond the Discord correction. The D14 queue file `work/QUEUE-2026-10-04.md` is not in either repo checkout I could read, so its text is unread.

Reversal for Cortex only, 2026-10-16 (memory `cortex-grid-engine-preference-2026-10-16`): Manny, after the bonnie-32 Cathedral ran at 29.5 fps on the BSP runtime: "I'd rather move to a pure grid engine like tombraider, if the grid is as small as the cathedral I believe we have enough flexibility." quake-psx and hl-psx stay on psx-bsp. Memory `bonnie32-levels-ps1-budget-2026-10-16` argues open areas cost the same in any world representation, so the grid-vs-BSP choice should not be argued on open-area grounds.

## 4. What to salvage, with exact sha:path

Convention: `sha:path` readable with `git show`. "Already on HEAD" means a generalised copy exists in the current tree and you should read that instead.

### 4.1 Streaming (the item Manny asked about)

| Piece | sha:path | Notes |
|---|---|---|
| Page pool with generation handles | Already on HEAD: `engine/crates/psx-game-runtime/src/streaming/page_pool.rs` (added by `358728f2`). Original: `e1e55509:engine/crates/psx-game-runtime/src/room_streaming.rs` lines 1096 to 1480 (`StreamedRoomPages`, `ResidentRoomHandle`, `prepare_slot`, `release_slot`, compaction) | Contiguous 2 KiB runs, sectors land at final RAM address, all-zero valid so statics stay in `.bss` |
| Scheduler policy (backoff, epochs, pin flags, counters) | Already on HEAD: `engine/crates/psx-game-runtime/src/streaming/scheduler_policy.rs` (`358728f2`). Original `e1e55509:.../room_streaming.rs` lines 87 to 98 and 270 to 322 | Backoff 16 to 512 windows, protected prefix |
| Full original scheduler incl. `reconcile_residency`, `pump`, `commit_ready_job_entries` | `e1e55509:engine/crates/psx-game-runtime/src/room_streaming.rs` (1690 lines) | 4 tests: `page_pool_charges_exact_sector_count`, `released_or_reused_slot_invalidates_old_handle`, `fragmented_pool_compacts_and_preserves_live_bytes`, `writes_and_reads_across_page_boundary_without_staging` |
| Portal-hop prefetch ring | `e1e55509:.../room_streaming.rs` line 1541 `room_graph_ring` | Not salvaged by the 10-08 commit; the design doc kept it "only as a test oracle". A TR grid needs exactly this (rooms + portals), so this one is worth porting back |
| Active window state machine | `e1e55509:engine/crates/psx-game-runtime/src/room_window.rs`, `e1e55509:engine/examples/editor-playtest/src/active_rooms.rs` | Read for the incremental rebuild job and request anchor; per-tick reconciler `04b57e3d` replaced event-triggered rebuilds |
| Required/warm residency sets | `e1e55509:engine/crates/psx-level/src/lib.rs` `RoomResidencyRecord` (:1424), cooked by `manifest.rs` | Cooked but never consumed (07-25 audit finding 1) |
| BSP replacement now on HEAD | `a54e8d77`: `engine/crates/psx-game-runtime/src/region_stream.rs`, `cd_stream/region.rs`, `engine/crates/psx-bsp/src/pxbsp_stream.rs`, `editor/crates/psxed-project/src/stream_world.rs` | Opt-in `PSXED_STREAM_WORLD` + `world-stream` feature. A grid world would map rooms to regions here instead of reviving `RoomStreamScheduler` |
| Streaming docs | `e1e55509:docs/streaming-audit-2026-07-25.md`, `docs/streaming-audit-2026-06-12.md`, `docs/level-residency.md`, `docs/streaming-design-2026-10-08.md` section 10, `docs/streaming-survey-2026-10-08.md` | Section 10 of the design lists the salvage verdict per file; section "Gaps" of the survey lists what never existed |

### 4.2 Collision

| Piece | sha:path |
|---|---|
| Sector floor sampling shared by motor and camera | `e1e55509:engine/crates/psx-engine/src/floor_sample.rs` (68 lines) |
| Grid collision backend inside the motor | `e1e55509:engine/crates/psx-engine/src/character_motor.rs` (helpers around lines 2370 to 2700: `sector_floor_collision`, `sector_probe_wall`, `wall_segment_xz`, sparse-world test builders near 4776); 89 tests in file mix grid and policy |
| Grid probes in the camera | `e1e55509:engine/crates/psx-engine/src/third_person_camera.rs` (63 tests, uses the same sampling) |
| Zero-copy runtime facade | `e1e55509:engine/crates/psx-engine/src/world.rs` (`RuntimeRoom`, `RoomCollision`, `RoomRender`, 1539 lines) |
| Compact collision sector payload | `e1e55509:engine/crates/psx-level/src/lib.rs` `compact_collision_header`; cooked in `manifest.rs` ~:3117; `.psxw` reader in the SDK repo `main:sdk/crates/psx-asset/src/lib.rs` `World`, `WorldSectorFloorCollision` (:3007) |
| Pre-delete motor with policy and grid together | `22894367^` = `bbf6fbb9`:`engine/crates/psx-engine/src/character_motor.rs` |

### 4.3 Editor tools, model, cook, preview

| Piece | sha:path |
|---|---|
| Data model | `e1e55509:editor/crates/psxed-project/src/world_types.rs`, `scene_grid_types.rs` (3356, 8 tests), `spatial.rs` |
| Portal room planner (split, caps) | `e1e55509:editor/crates/psxed-project/src/portal_rooms.rs` (1368, 7 tests); `room_connections.rs` (367); `floor_view.rs` (166) |
| Paint/edit tools and picking | `fc3129dc:editor/crates/psxed-ui/src/workspace/painting.rs`, `workspace/geometry.rs`, `workspace/selection.rs`, `workspace/input.rs`, `workspace/toolbars.rs`, `psxed-ui/src/geometry.rs` (`e1e55509`, 2075 lines), `viewport2d.rs` |
| Sector inspector | `cf154ab4:editor/crates/psxed-ui/src/sector_inspector.rs` (580) |
| Prefabs | `cf154ab4:editor/crates/psxed-project/src/prefab.rs` (609); `cf154ab4:editor/crates/psxed-project/src/bin/gen_prefab_kit.rs` (913), `gen_prefab_gallery.rs`, `prefab_sheet.rs`; `cf154ab4:editor/prefabs/*.ron` (20 prefabs) |
| Cook | `cf154ab4:editor/crates/psxed-project/src/world_cook.rs` (+ `world_cook/{encode,validation,coords,materials}.rs`), `playtest/cook_world.rs`, `playtest/cook_visibility.rs` |
| Editor preview of rooms | `cf154ab4:emu/crates/frontend/src/editor_preview/room_geometry.rs`, `portals.rs`, `visible_rooms.rs` |
| Play overlay and room map dashboard | `cf154ab4:editor/crates/psxed-ui/src/play_overlay.rs` (1294) |
| Sample projects | `cf154ab4:editor/samples/cortex_v1/project.ron` (6642 lines, full grid game); `editor/archive/fixtures/legacy-grid-starter/` (cf154ab4); `editor/archive/legacy-grid/README.md` has the archive map (cortex_v1..v5, demo_02..demo_11, vis_corridor; payload is local and gitignored, so only the README is in git) |

### 4.4 Tests worth porting

| Test set | sha:path | Count |
|---|---|---|
| Editor paint/placement interaction tests | `cf154ab4:editor/crates/psxed-ui/src/tests/placement_painting.rs` | 35 |
| World editing tests | `cf154ab4:editor/crates/psxed-ui/src/tests/world_editing.rs` | 49 |
| Cook tests | `cf154ab4:editor/crates/psxed-project/src/world_cook.rs`, `playtest/tests/rooms_visibility.rs` (14), `playtest/tests/water.rs` (2) | 30 + 14 + 2 |
| Project model tests | `cf154ab4:editor/crates/psxed-project/src/tests.rs` | 151 (mixed) |
| Preview tests | `cf154ab4:emu/crates/frontend/src/editor_preview/tests.rs` | 68 |
| Portal traversal | `e1e55509:engine/crates/psx-level/src/portal_visibility.rs` | 27 |
| Renderer tests | `e1e55509:engine/crates/psx-engine/src/world_render/tests.rs` (36), `render3d/tests.rs` (59, partly kept) | |
| Motor/camera | `e1e55509:.../character_motor.rs` (89), `third_person_camera.rs` (63) | mixed grid and policy |
| Stress harness | `e1e55509:editor/crates/psxed-project/src/bin/gen_stress_map.rs` (corridor and field generators, portal markers at cell centre +0.05) | |
| .psxw reader/writer round trips | SDK `main:sdk/crates/psx-asset/src/tests.rs` (still present) | |

HEAD still has the headless interaction rig `editor/crates/psxed-ui/src/tests/interaction_rig.rs` (ViewportHarness), usable for a new grid tool test suite.

### 4.5 Ideas, not code

- Seamless TR-style streaming needs the room graph ring plus the page pool; both are small.
- Prefab seam rule (incoming wall loses) and the `strip_seam_walls` check are cheap and correct.
- Fail-closed cook budget style (hard error at cook, same caps shown in the editor budget panel).
- Single coordinate-flip boundary in the cook (`docs/editor-runtime-coordinates.md`).
- The Sims-style active floor + ghost of the floor below is the UX the user liked (memory says floors were playtest-verified 2026-06-21).

## 5. What NOT to repeat

Each item has its evidence.

1. Per-surface interpretation at draw time with independent 4-corner surfaces and no shared vertices. Cost 1.6k (07-15) to 5.6k..9k (07-26) cycles per considered surface; the retained packet pool / prebuilt quad family "was measured to destruction" in quake-psx (memory `cortex-room-is-psx-bsp`) and is dead code. Do not resurrect `PrebuiltRoomQuads`, `room_cache.rs` or `indexed_cache.rs`. Current prototype direction (shared vertex lattice per room, whole-cell culling, no runtime subdivision) is the right reaction.
2. Giant inlined draw loops. 35,488-byte function vs a 4 KB I-cache (`cortex-v3-render-architecture`).
3. No distance cut and no PVS. 1.7M cycles per vblank on an open sightline (07-15). Portal traversal alone cannot save a genuinely open hall (memory: Cathedral p50 397 tiles, perfect occlusion still p50 252).
4. Hand-placed portals as the only room boundary, with the planner refusing to invent boundaries. Result: authors had to place `Portal` nodes at exact seam positions; the marker convention bit people ("portal marker convention that blocked the seam", `785ddcff`); portals must sit at cell centre +0.05 toward the target edge or an on-boundary coordinate tie-breaks west.
5. Silent oversize failure. Cook declared chunks up to the pack limit while the runtime clamped slots to 32 KB; oversize rooms failed every load with exponential backoff and rendered empty sky at a perfect 30 fps (stress-map memory, fixed by `45fd3659`). Any grid-room port must share one constant and hard-error at cook.
6. Cook/runtime cap drift: `MAX_ROOM_WIDTH 32` was only right for sector size 1024 (i16 cliff), too loose elsewhere. Derive caps from sector size.
7. Prefetch that never fires because the request list equals the active window (counter was dead, 07-15). Test the prefetch ring with a counter.
8. Camera-dependent window membership. `vis-draw-pruned` as active-window membership caused constant teardown/rebuild churn; the ring's camera independence is what makes reuse work. Filter per frame, keep membership stable.
9. One load job in flight, a pump budget "derived" from nothing (4.0 sectors per vblank requested vs 2.5 deliverable), seek missing from the scheduler model (one seek = 8 sectors at 2x). Audit findings 2 to 4.
10. Whole-level asset residency and a bump allocator that cannot free (`PersistentAssetStorage`, 07-25). Not grid-specific, but the grid port will hit it when worlds grow. Manny said not to touch model/animation residency in the streaming work (memory 10-08).
11. Streaming over polled ch3 DMA. Console DMA is unreliable (memory `console-cd-dma-unreliable`); use PIO / seek-first and the IRQ transport work in the current tree. Do not copy `cd_stream/hw.rs` (`start_cd_read_at_lba` is bare SetLoc+ReadN).
12. Stacked floors as `node.floor` checked in six separate render walks plus pick, bounds and cook. The 06-xx bug stream ("entity shows on the wrong floor / selection hits the floor below") came from that; `floors-editor-architecture.md` proposed three fixes, none implemented. Gap left open: upper-floor entity handling. If vertical space is wanted, make "floor" a first-class container the tools, picking, preview and cook all resolve through one function.
13. Room-local entity coordinates with `RoomIndex` everywhere (spawn records, AI gating, leash tests like `input.player_room != record.room` in `entities.rs`/`tactics.rs`). That coupling is why the BSP migration needed a synthetic grid scaffold and why `2091fb6e` had to rewrite the leash. In the port, store world-space positions and derive room membership.
14. The BSP cook riding on a stub grid, or a grid cook that needs `Portal` nodes to exist. Keep one world-format switch (`ProjectWorldFormat`, `document_types.rs`) and fail closed on mixed state, as `e4031c39` did.
15. Editor monoliths. psxed-ui `lib.rs` was 45k lines before `b0be1880`; `world_types.rs` 2201, `scene_grid_types.rs` 3356, `NodeKind::Room { grid }` matched at ~168 sites (floors-plan). Keep the grid model behind a small API.
16. Two coordinate conventions (editor North +Z, runtime North -Z). It is fine only because `world_cook` is the single flip point; bonnie-32 and the current BSP world have their own handedness history (`b1e7da78` "Brush worlds were rendered mirrored").
17. Wall ownership ambiguity (East of A vs West of B) and rejected diagonal walls. Pick one canonical edge representation up front.
18. Judging performance on an old cook. The 07-15 "-39% regression" was a stale-baseline artifact. Re-cook and re-baseline in the same session.
19. Pitching the old streamer as a capability. Memory `grid-rooms-deprecated`: never describe the old grid CD streaming to outsiders as a current feature.

## 6. Current state on this branch (for the planner)

- HEAD of this worktree is on `exp/grid-engine-2026-10-16`, with the grid prototype: `editor/crates/psxed-project/src/brush_grid.rs`, `engine/crates/psx-bsp/src/render/grid.rs`, `engine/examples/editor-playtest/src/bsp_runtime/grid_world.rs` (`5d93e571`, message says "unverified, may not build"), plus `64faaa1f` and `29704ca5`. The bonnie-32 importer is `0e64a92b` (`import_bonnie32_level.rs`, converts each floor/ceiling/wall to a thin brush, scaled 9/16).
- Survey of other bonnie levels: `build/oh-runs/grid/other-maps-survey.md` (Cave, Dungeon, Sewers, West, level_008 all fit with a 16-cut and 2x2 merge; Cathedral frustum p50 397 / p90 928 / max 1467 tiles).
- The BSP streaming path that replaced the room streamer is `a54e8d77` and is on this branch.

## 7. How to read things quickly

```
git show cf154ab4:editor/crates/psxed-project/src/world_cook.rs | less
git show e1e55509:engine/crates/psx-game-runtime/src/room_streaming.rs | sed -n 1090,1500p
git show e1e55509:docs/streaming-design-2026-10-08.md | sed -n 501,524p
git diff --stat cf154ab4 e1e55509 -- engine/crates/psx-engine/src/world.rs
```

To materialise a snapshot without touching any checkout, use `git archive <sha> <path> | tar -x -C <scratch>`.
