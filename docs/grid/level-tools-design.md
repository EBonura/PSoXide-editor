# Grid level tools: the design for an easy level-building editor (2026-10-17)

Status: final design, ready to implement. Branch base: `grid/main` 9dc6430e. Written read-only against that tree and against the in-progress free-plane work in the `grid-runtime` worktree (branch `grid/planes`, commit bd53898a plus uncommitted editor files). Implementation starts when free planes merge; section 12 says which files collide.

Manny's direction, as approved, is the contract:

1. The room is a **quadtree**, big cells by default, subdivided on demand down to 1/4 sector, merged back.
2. Interaction is **region, then verb, then drag an amount** snapped to clicks. Verbs only write ordinary grid data that stays hand-tweakable.
3. Walls are **derived** from height differences. Authored walls remain only for what heights do not imply.
4. The diagonal split is **chosen automatically**; manual flip stays.
5. Verbs: Raise/Lower, Sculpt, Ramp, Stairs, Room, Corridor, Opening, Ceiling follow, Stamps, smart paint rules, per-face override.
6. Overlays: walkability and a performance heatmap.
7. **Horizontal wall bands** are a first-class concept (section 6).
8. **Decoupling**: render cost must not depend on authoring granularity (section 9).
9. Latest overrides from Manny: make it work first, optimise after (perf is measured and reported per phase, never a gate to building); and a precise, easy **texture tiling control** with one UV model shared by faces, derived walls, bands and planes (section 5).

How to read the numbers. **[measured]** came from a headless run or a file and the source is named. **[derived]** is arithmetic on measured values or on struct sizes. **[estimate]** is reasoning with no run behind it. **[proposal]** is a chosen constant that no measurement backs, to be revisited with data. Nothing in this document is a benchmark I ran.

Path shorthand. `E/` is `editor/crates/psxed-project/src/grid/`, `U/` is `editor/crates/psxed-ui/src/grid_tools/`, `P/` is `engine/crates/psx-grid/src/`. Line numbers refer to `grid/main` 9dc6430e unless a path starts with `grid-runtime:` (the planes worktree; uncommitted files have no stable line numbers).

---

## 0. Summary

* **Document**: `GridRoom` keeps its rectangle (origin, width, depth in sectors) but its cells become a **linear quadtree of leaves** keyed by Morton-ordered `(level, x, z)` in click units. Level 2 is exactly today's sector, so all six reference levels and the starter are "all leaves at level 2" and load unchanged. A leaf is a `GridSector` (floor, ceiling, authored walls) of any of five sizes: 4 sectors, 2, 1, 1/2, 1/4. Neighbouring leaves differ by at most one level (2:1 balance), so a coarse edge has at most one hanging vertex.
* **Runtime**: pointerless, breadth-first node arrays per room with u16 ids, plus a **per-sector index straight to the leaf** (or to a 4x4 sub-block) so the hot point lookup stays two dependent loads and no loop. The tree serves rays, culling, LOD and streaming in the later optimise phases.
* **Walls**: per-room `wall_mode`. New rooms are `Derived`: a wall exists wherever neighbouring leaves disagree in height, with material from a fixed resolution chain. Legacy rooms stay `Authored` until an explicit, look-preserving **Convert** command.
* **Bands**: ordered horizontal cut lines with a material and a UV rule per band, stored as **band sets** (room default plus per-run overrides), applied to derived and authored walls alike.
* **UV**: one `UvMap` (mode FIT or WORLD, offset, quarter turns, flip) shared by floors, ceilings, walls, bands and planes. FIT is "N x M tiles across this face"; WORLD is tiles per sector, world-aligned.
* **Cook**: stitches T-junctions, splits and merges quads under the texture window (E2) with the exact UV rules of section 9, caps quad size near camera-reachable space, and writes PGL v3 (window repeat) then v4 (leaf collision).
* **Order** (section 13): derived walls, UV model, window repeat, bands, auto split with Raise/Lower, Ramp with walkability, Stairs with the perf heatmap, then the quadtree (storage, operations, cook and collision), Sculpt, Room and Corridor, Opening and Stamps, and last the optimise phases (cook merge, then tree culling, node LOD, ray early-outs, streaming).

---

## 1. Terms

| Term | Meaning |
| --- | --- |
| grid unit (gu) | Document unit. Sector = 1024 gu, click = 256 gu, fine snap = 64 gu (`E/units.rs:6-13`). |
| click | 256 gu, one quarter sector. The finest leaf is one click wide. |
| sector | 1024 gu. Level-2 leaf. Runtime size S ru (80 at the default scale 5/4: `E/units.rs:36`). |
| runtime unit (ru) | Engine unit, editor unit / 16. |
| leaf | A quadtree cell that holds data. Level 0 = 4 sectors (16 clicks), 1 = 2 sectors, 2 = 1 sector, 3 = 1/2, 4 = 1/4. |
| derived wall | A vertical face the editor computes from a height difference. Not stored. |
| authored wall | `GridWall` records on an edge stack, as today (`E/surface.rs:175`). |
| run | A maximal chain of collinear wall segments on one side with the same style. The editing unit for walls. |
| band | A horizontal strip of a wall between two cut lines, with its own material and UV rule. Not to be confused with the existing **course** split at 1024 gu multiples that `faces.rs:317` does so the world-aligned texture tiles. Courses are an implementation detail and go away (section 9.4). |
| region | The set of cells or corners the next verb applies to. |
| tile | One repeat of a 64 by 64 texture (the cook's assumption today: `E/cook/render.rs:25`). |

---

## 2. What exists now (the facts this design builds on)

Document. `GridRoom { name, origin, y_offset, width, depth, ambient, fog, sectors: Vec<Option<GridSector>> }` (`E/room.rs:55-77`), z-major, grown and trimmed by whole sectors without moving geometry (`E/room.rs:157`). `GridSector` holds `floor`, `ceiling` and six wall stacks (`E/sector.rs:108`). `GridSurface` has four corner heights `[NW,NE,SE,SW]`, a split, optional second-triangle heights, per-triangle `GridTri { material, uv: FaceUv }`, and `walkable` (`E/surface.rs:74-94`). `GridWall` has four heights, material, `FaceUv`, `facing`, `solid` (`E/surface.rs:175`). Walls are explicit: a wall tool fills the gap between floor, ceiling and existing walls on one edge, following slopes (`E/edit/walls.rs:84`, the bonnie-32 behaviour). Corner heights are welded by an edit-time index only (`E/edit/heights.rs:16`); stored data has no shared vertices.

Faces. `cell_faces(world, room, x, z)` is the single geometry derivation the editor preview, the light bake and the cook share (`E/faces.rs:346`, used at `emu/crates/frontend/src/editor_preview/grid.rs:134`). A planar surface with equal triangles is one quad; otherwise two triangles (`E/faces.rs:226`). Rectangular walls are cut into courses at absolute multiples of one sector (`E/faces.rs:317`). Default UV is world aligned at 32 texels per sector (`E/units.rs:13`). `FaceUv` is `{ offset_texels, rotation_deg, scale_q8 }` (`editor/crates/psxed-project/src/face_uv.rs:8`).

Cook and runtime. The cook turns each face into one packet quad, welds vertices, bakes per-corner light, and rebases UVs into one 64 texel repeat (`E/cook/render.rs:115`); it refuses faces of more than four corners (`E/cook/render.rs:227`). The existing far LOD merges at most 2x2 cells (`E/cook/lod.rs:29`) and clamps the far UV edge to 63 (`E/cook/lod.rs:311`). Every fast packet already carries a texture-window word, currently always the reset (`P/render/emit.rs:235,273`); the near path passes `texture_window_word: 0` (`P/render/emit.rs:365`). The VRAM owner already places textures with `TextureWindowAtlas::allocate` (power-of-two, aligned, up to 128 texels: `sdk/crates/psx-vram/src/lib.rs:357`) and the grid runtime reads the placement (`engine/examples/editor-playtest/src/grid_runtime.rs:143`) but adds its origin to the packet UVs instead of using the window.

Collision. Dense `Sector` records, 20 bytes, indexed `cz*width+cx`, cell found by an exact reciprocal (`P/collision/world.rs:68,99`, `P/format.rs:266`). Walls exist as `ColWall` only for `solid` authored walls (`E/cook/collision.rs` header). Risers are implicit: the floor sampler plus the step rule block a body (`docs/grid/level-format-and-runtime.md` section 5.2). Motor constants: step-up 40 ru, step-down 40, body height 48, walkable normal 2867/4096 (`engine/crates/psx-engine/src/character_motor.rs:20,27,38`); the step-up is becoming config.

Editor tools. Eight tools, keys V F C W H P D O (`U/mod.rs:48-120`), `?`, `.`, L, R (wall direction and paste turn), Delete, PgUp/PgDn and `[` `]` as height nudges (`U/keys.rs:25-113`). One `Gesture` enum holds the gesture in flight (`U/mod.rs:223`), height drags snapshot the document for undo at the first change (`U/mod.rs:212`).

Measured context given for this design (headless emulator, supplied with the brief): about 800 cycles per visible quad, with emit about 420 per packet and projection about 69 per vertex; a near-plane split about 9 to 11k cycles per near tile; world p99 about 716k in the open Cathedral against a 1,128,960 cycle budget at 30 fps; the collision family about 19.6k cycles per sim tick on the Cathedral; the Cathedral collision block 22,748 bytes; the earlier flat LOD checker re-registered across 10 to 17 percent of the screen, popped by up to 12 percent of the screen in one frame, left 1 px seams, and saved about 16 percent in the worst open-nave view (714k to 597k).

### 2.1 Compatibility with free planes (grid/planes)

The planes branch adds, all additive and defaulted: `GridRoom.planes: Vec<GridPlane>` (`grid-runtime:E/room.rs`), `GridFaceKind::Plane`, `GridFace.unlit`, `MAX_PLANES_PER_ROOM = 64`, `edit/planes.rs`, `cook/plane_collision.rs`, and PGL version 2 (`PlaneRef` now `{ quad, count, flags, x0..z1, min_y, max_y }`, `ColPlane.flags` reserved; `P/format.rs` at bd53898a). It also refactors the face-to-quad code of `cook/render.rs` into `RoomBuild::add_face`. Rules this design follows:

* Planes stay outside the leaf tree. They are per-room, free, never welded to the lattice, and keep their own `QUAD_PLANE` run. No phase here changes `PlaneRef`, `ColPlane` or `plane_faces.rs`, except the UV unification (section 5.7), which is a small, separate change.
* `GridPlane.uv_mode` (Tile or Stretch) maps onto the shared model: Tile is WORLD anchored at the plane's own top-left corner, Stretch is FIT 1x1. The plane's `FaceUv` field remains the fine adjustment.
* Collision for planes copies into the sector extension of each overlapped sector. With big leaves the copy attaches to the leaf (section 10.2).
* Version numbers: planes took VERSION 2. This design adds v3 (window repeat) and v4 (leaf collision). Each bump needs a golden fixture.

---

## 3. Data model

### 3.1 Units and snapping

* Everything stays integer grid units. The finest leaf is 256 gu, so every leaf corner lies on a click. Heights are stored in gu and snapped to the fine step (64 gu) by the tools, as today. Verb amounts snap to a click (256), Alt snaps to 64.
* The cook rounds every position once through `RuntimeUnits::rt` (`E/cook/units.rs`), so what is drawn is what is stood on. At scale 5/4 a click is 20 ru and a fine step 5 ru [derived from `E/units.rs:108-113` test]. A scale whose sector is not divisible by 4 cannot hold quarter leaves: `limits::validate` gains an error `SectorNotQuartered { s }` for sub-sector leaves when `S % 4 != 0` (S = 84, 80, 36 and 56 all pass; those are the scales 21/16, 5/4, 9/16 and 7/8 in use).
* Position keys use **clicks** (`i32`). A leaf of level `l` has edge `n = 16 >> l` clicks and its key `(l, x, z)` has `x % n == 0 && z % n == 0` in **world** click coordinates. World anchoring means growing or trimming a room never moves a leaf (the same guarantee `grow_to_include` gives sectors today), and a new room's origin snaps to a multiple of 4 sectors so level-0 leaves fit.

### 3.2 The leaf tree

```rust
pub const LEVELS: u8 = 5;                       // 0 = 4 sectors ... 4 = 1/4 sector
pub const SECTOR_LEVEL: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeafKey { pub level: u8, pub x: i32, pub z: i32 }   // world clicks, aligned

pub struct GridRoom {
    // existing: name, origin, y_offset, width, depth, ambient, fog, planes
    pub leaves: LeafMap,            // BTreeMap<LeafKey, GridSector>, replaces `sectors`
    pub wall_mode: WallMode,        // Authored (legacy default) | Derived
    pub defaults: RoomDefaults,     // wall material, bands, paint rules
    pub band_sets: Vec<BandSet>,    // section 6
    pub edge_overrides: BTreeMap<EdgeKey, EdgeOverride>,   // section 4.4
    pub openings: Vec<Opening>,     // section 4.5
}
```

Why a leaf map in the document and arrays at runtime. Editing wants cheap insertion, deletion and ordered iteration with stable keys that survive undo; the cook wants breadth-first arrays. `LeafMap` is the linear quadtree: leaves are disjoint and aligned, interior nodes are implicit (a node exists while some leaf lies below it), an uncovered area is void. Iteration order is Morton order of `(x/n, z/n)` with the level as tiebreak, so serialisation is deterministic. The cook walks the map once to build the breadth-first node arrays (section 10).

`GridRoom::sector(x, z)` (`E/room.rs:104`) is kept as a compatibility accessor that returns the leaf **only if it is a level-2 leaf**; code that must handle any size uses `leaf_at(world_click)` or `leaves_in(rect)`. While a room has only level-2 leaves (every room until the quadtree phase lands, and every legacy room forever unless edited) behaviour is unchanged.

**Leaf data.** A leaf is the existing `GridSector` (floor, ceiling, wall stacks). Constraints, enforced by `normalize` (3.5) and `validate`:

1. Leaves above level 2 must be **planar** per surface (`GridSurface::is_planar`, `E/surface.rs:149`), have no split-level `heights_b`, and no diagonal walls. A non-planar big leaf is subdivided automatically.
2. Leaves below level 2 carry no diagonal walls and no `heights_b`.
3. A leaf lies wholly inside the room rectangle.
4. 2:1 balance: leaves sharing an edge differ by at most one level.

**Hanging corners.** Where a coarse leaf C meets two finer leaves along an edge, the finer leaves share a corner M at the midpoint of C's edge. If the two end corners of C's edge are welded to the finer side (equal position and height at both ends), M is a **constrained** corner: its height is derived as `(a + b) div 2` with `a`, `b` the heights of C's two corners at that edge, and `normalize` rewrites it. If the ends differ (a cliff), M is free and the wall between them is derived per finer segment. The constraint is what makes continuous surfaces crack-free in the document; the stitching of 9.3 makes them crack-free on the GPU.

### 3.3 Why a quadtree, and the rejected alternatives

Chosen: quadtree, because the same structure serves authoring (big cells, region tools), collision memory (a flat 4-sector leaf is one 20 byte record, not 16), rays (skip empty and flat nodes), culling and LOD (reject or replace a subtree), and streaming (a root block is a natural unit). Balanced and capped at five levels it is shallow.

Rejected or deferred alternatives, kept for the record:

* **Fixed 4x4 sub-block under the uniform sector grid** (one subdivision level, `SECTOR_INDEX` to a 16 entry block). This was the first recommendation after performance concern about tree descent in traces that run many times per sim tick (collision ~19.6k cycles per tick on the Cathedral [measured]). The runtime format chosen below contains this model as its sub-sector half: sub-sector leaves are reached through exactly such a 4x4 table, and the point query never descends a tree. If the phase T10 measurement of collision and camera trace cost shows a cost against the uniform grid, falling back is a document-side restriction (cap the leaf level at 2, drop levels 0 and 1) with no format change.
* **General tree with pointer/variable-size stepping in every query.** Rejected for the hot path: the per-sector index (10.2) gives O(1) point lookup, the tree is used only by rays and culling where skipping pays.
* **Dense replication of big leaves in the collision block.** The simplest and what a "flat-only" design would do: one 20 byte record per sector. It costs RAM in large flat areas (the Cathedral block is 22,748 bytes [measured]). It remains the cook's fallback layout for rooms that use no big leaves (section 10.2).

### 3.4 Region, selection and references

`GridSelection` (`E/edit/selection.rs`) addresses elements by room and **world sector**; with leaves it addresses `FaceRef { room, leaf: LeafKey, target }` (the `cell: [i32;2]` field becomes a `LeafKey`; for level-2 leaves the conversion is trivial). `CornerRef` gains the same key. `WeldIndex` (`E/edit/heights.rs:16`) already buckets corners by grid position, which works unchanged on the click lattice.

A **Region** is new editor state, not saved:

```rust
pub enum RegionKind { Cells, Corners, EdgeRun }
pub struct Region { pub room: usize, pub kind: RegionKind,
                    pub leaves: BTreeSet<LeafKey>,        // cells region: leaves fully or by centre
                    pub corners: BTreeSet<CornerKey>,     // click lattice points, welded groups
                    pub run: Vec<EdgeKey> }
```

It persists after a verb until Esc or a new pick, so verbs chain ("pick region, Raise, then Ramp the same cells"). Region sources: marquee (cells whose centre is inside), paint brush (radius with `[` `]`), and path (waypoints). A region over big leaves selects the whole leaf; a marquee that covers only part of a big leaf offers "split to fit" (the verb first subdivides the leaf, which is lossless).

### 3.5 `normalize` and the invariants

One pure function, `grid::tree::normalize(room, dirty: &LeafSet)`, restores the invariants after every edit and is called by every verb before it returns:

1. Subdivide any big leaf that became non-planar or whose neighbours force 2:1 balance (repeat to fixpoint; only ever adds leaves, so it terminates).
2. Rewrite constrained hanging corners (3.2).
3. Re-evaluate `auto_split` surfaces (section 4.6).
4. Merge four sibling leaves into their parent when the parent represents them **exactly** (same materials, same UV maps, no authored walls or overrides on interior edges, and every sibling corner and edge midpoint equals the parent's triangulated surface for either diagonal) and the merge keeps 2:1 balance. Merging is lossless; the lossy "Simplify" command takes a tolerance.
5. Split or collapse `edge_overrides` and `openings` keyed to edges whose leaf subdivided or merged (an override on a coarse edge becomes the same override on both halves; four equal halves collapse back).

`normalize` touches only dirty leaves and their neighbours. Undo stores the whole document snapshot as it does today (`U/mod.rs:212`), so `normalize` never needs its own inverse.

### 3.6 Serde, versioning and migration

* Every new field is `#[serde(default, skip_serializing_if = ...)]` and its default **reproduces today's behaviour**: `wall_mode` defaults to `Authored`, `band_sets`/`edge_overrides`/`openings` to empty, `auto_split` to `false` (a legacy split is deliberate: level_008 has 46 NE-SW floors [measured, `docs/grid/other-maps-survey.md`]), UV `map` to `None`.
* **Leaf form negotiation.** On disk a room is written as the legacy dense `sectors` list while every leaf is level 2, and as the sorted `leaves` list otherwise. Load accepts both. So an unrefined project round-trips without change and the six tracked reference projects never need rewriting (note the existing caveat that a RON round trip is not always byte clean; the golden test compares documents, then re-serialised documents, not raw bytes of the tracked files).
* No `schema` number is needed: absence of a field means legacy. A `GridWorld.tools: u32` counter is added only when stamps land (3.8), defaulting to 0.
* **Convert room to derived walls** is the only migration, explicit and look-preserving by default (4.7).
* New rooms (File > New Grid Project, the Room verb, the Rooms panel `+`) are written `Derived`. The starter project gets `Derived` and its explicit walls converted at creation.

### 3.7 Limits (`E/limits.rs`)

Existing caps stay (`MAX_ROOM_SIDE` 128, `MAX_WALLS_PER_EDGE` 16: `E/limits.rs:23-26`). New constants, each labelled:

| Constant | Value | Basis |
| --- | --- | --- |
| `LEVELS` | 5 | Manny's decision (4 sectors down to 1/4). |
| `MAX_LEAVES_PER_ROOM` | 16383 | [derived] 14 bit id in `SECTOR_INDEX` (10.2) with `0x3FFF` reserved for "no leaf". A 128 by 128 room of only level-2 leaves would need 16384, one over; such a room uses big leaves, or is two rooms. |
| `MAX_SUB_BLOCKS_PER_ROOM` | 1024 | [proposal] each block is 32 bytes of index plus its leaves; revisit with the phase T10 size report. |
| `MAX_BAND_SETS_PER_ROOM` | 32 | [proposal] |
| `MAX_BAND_LINES` | 6 per set (7 bands) | [proposal] |
| `MAX_OPENINGS_PER_EDGE` | 4 | [proposal] |
| `MAX_STAMPS` / `MAX_STAMP_CELLS` | 64 / 1024 | [proposal] |
| `MAX_FIT_REPEAT` | 16 tiles per axis (256 in 1/16 steps) | [proposal]; larger values are accepted by the cook and split, but the inspector asks for confirmation. |

`MAX_WORLD_FACES` (`E/limits.rs:37`) stays as the authoring warning (orange from 85 percent in the status bar) but stops being the hard cap: the hard errors move to the cook's exact checks (65535 vertices and quads per room, 255 quads per render cell: `E/cook/render.rs:227` and the existing `u8` count check). The budget readout adds the cooked quad count once the cook has run (section 11.3).

### 3.8 Stamps

```rust
pub struct Stamp { pub name: String, pub size_clicks: [i32;2],
                   pub leaves: Vec<(LeafKey /*relative*/, GridSector)>,
                   pub edge_overrides: Vec<(EdgeKey, EdgeOverride)>,
                   pub openings: Vec<Opening>,
                   pub band_sets: Vec<BandSet> }            // referenced sets, re-indexed on place
// GridWorld.stamps: Vec<Stamp>, serde default empty
```

Heights are stored relative to the stamp's anchor floor height; placement adds the floor height under the pointer. Rotation and flips reuse `Transform` (`E/edit/clipboard.rs:20`), extended with the click lattice, edge overrides, openings and the UV `turns` and `flip` fields. Placement snaps the stamp anchor to the alignment of its coarsest leaf; if the pointer cell is less aligned, the placement offers "split to fit" which subdivides the **target** leaves, never the stamp. Nodes (lights, spawns) are not part of a stamp in v1. Ctrl+C / Ctrl+V remain the ephemeral clipboard; a stamp is a named, persisted clipboard.

---

## 4. Derived walls, openings and the auto split

### 4.1 The rule

A wall is a function of the two leaves on either side of an edge. For one edge segment, take each side's **floor profile** `f(u)` and **ceiling profile** `c(u)` along the segment (linear between the two edge corners of the leaf's surface; for a split surface the triangle that contains the edge is used, exactly as `surface_edge_heights` does, `E/edit/walls.rs:41`; `u` is the position along the segment). Then:

| Case | Faces made | Owner | Faces |
| --- | --- | --- | --- |
| R1 riser | both sides have floors and `fA != fB` | the **higher**-floor leaf | the strip between the two floor profiles, visible from the lower side |
| R2 drop | both sides have ceilings and `cA != cB` | the **lower**-ceiling leaf | the strip between the two ceiling profiles, visible from the higher side |
| R3 closure | the neighbour position holds no leaf | the leaf itself | its open interval `[f, c]` if it has a floor and a ceiling; `[f, f + room.open_wall]` if it has only a floor and the room sets `open_wall`; nothing otherwise |
| none | every other combination: open archway to a floorless or ceilingless neighbour, equal profiles, pit edges | | |

Notes.

* Profiles that cross inside a segment (a bow tie) are cut at the crossing into two triangles with opposite owners. Equal profiles give no face, which is the whole point: continuous floors never grow a wall.
* Faces are made per **segment of the finest common subdivision of the edge**: if one side is a coarse leaf and the other two finer leaves, there are two segments and the coarse side's profile is evaluated at the segment ends. With 2:1 balance that is at most two segments per edge (3.2).
* Derivation is **within one room**. A room's border is R3 against empty space. Rooms that sit side by side get a doorway with the Opening verb, which cuts both closure walls (the verb detects the neighbouring room across the border). Cross-room derivation would need the stacked-room disambiguation the dungeon needs for `locate`; it is out of scope (open question 6).
* Derived faces are produced in `faces.rs` next to `cell_faces`, so the preview, the light bake and the cook see the same polygons (the property `E/faces.rs:1-12` insists on). `cell_faces(world, room, x, z)` keeps its signature and returns the faces **owned** by that sector, clipped to the sector, which keeps the preview's per-sector bounds culling working. Cell bounds for culling are widened by the derived extent: `GridSector::height_range` (`E/sector.rs`) gains a companion `derived_range(room, key)` the preview uses (`emu/crates/frontend/src/editor_preview/grid.rs:26-47`).
* Face count: a derived room needs no authored walls at all, and adding one is the cost of one riser per height step, so the budget readout stays honest (`face_count`, `E/faces.rs:375`).

### 4.2 Authored walls in a derived room

Authored walls remain for what heights cannot say: a freestanding wall on a flat floor, a parapet on a ledge, a diagonal wall, an extra panel stacked on a derived face. In `Derived` mode they are **additive**. The Wall tool's gap logic (`next_wall_heights`, `E/edit/walls.rs:84`) now treats the derived faces on that edge as already-existing walls when it looks for gaps, so on an edge fully closed by a derived face it answers "fully covered" instead of drawing a coincident wall. `limits::validate` adds a warning `AuthoredDuplicatesDerived` for any authored wall whose span lies inside a derived face on the same edge and side (it would z-fight).

### 4.3 Material chain for a derived face

First hit wins:

1. a band material from the `EdgeOverride`'s band set (4.4), when the face has bands;
2. the **owner** leaf's `wall_material` (new optional `GridSector` field: "what derived faces owned by me look like");
3. the **facing** leaf's `wall_material` (the neighbour that looks at it; this is Manny's "material from the neighbour");
4. the room's `defaults.wall_material`;
5. the owner's own surface material: its floor triangle material for R1 and R3, its ceiling material for R2.

Verbs write step 2 explicitly when the palette has a material chosen (so a platform raised with brick selected gets brick sides), and leave it unset otherwise so the chain falls through. Paint on a derived face writes the `EdgeOverride`, never an authored wall.

### 4.4 `EdgeOverride`

```rust
pub struct EdgeKey { pub x: i32, pub z: i32, pub axis: Axis, pub len: u8 }  // world clicks of the segment start, direction, length in clicks
pub struct EdgeOverride {
    pub hidden: bool,                  // no derived face here (look only; risers still block by the step rule)
    pub band_set: Option<u8>,          // index into room.band_sets; None = room default
    pub material: Option<ResourceId>,  // replaces the whole chain when the face has no bands
    pub uv: Option<FaceUv>,            // per-run UV placement, includes UvMode (section 5)
    pub solid: Option<bool>,           // R3 closure only; false = ghost wall, no collision
}
```

Keys are geometry-addressed, never indices, so they survive edits to neighbours and undo. `normalize` (3.5 step 5) splits and collapses them. An override whose edge no longer produces a face is kept (the user may be mid-edit) and listed by a `validate` info line; "Clean up" in the Rooms panel removes them.

### 4.5 Openings

```rust
pub struct Opening { pub edge: EdgeKey, pub from: u8, pub to: u8,       // clicks along the segment, from < to <= edge.len
                     pub bottom: HeightRef, pub top: HeightRef, pub frame: Option<ResourceId> }
pub enum HeightRef { Floor(i32), Ceiling(i32), Abs(i32) }               // gu offset from the wall's floor or ceiling edge profile, or room-relative absolute
```

An opening removes a rectangle from the wall (derived or authored) on that edge and leaves a **sill** below `bottom` and a **lintel** above `top`. Rectangles are applied before bands (6.3). `bottom = Floor(0)` makes a doorway. Openings apply to R1 to R3 faces and to authored walls (the authored wall record is split into sill and lintel records by the verb at creation, because authored walls carry no opening list). Collision: a derived riser or drop never has a collision record, so an opening in one is visual, and a body passes only when the floors allow it (step rule). An opening in a closure wall (R3) or an authored wall changes the `ColWall` set: the cook emits `ColWall`s for the sill and lintel pieces only, with a partial span (10.1, `ColWall.reserved`). Portal derivation (`E/cook/portals.rs:26`) reads open spans from the document's floors and ceilings, so it needs one change: a closure wall without an opening blocks the portal, one with an opening yields the open interval of the opening.

### 4.6 Auto split

Per surface, `GridSurface` gains `auto_split: bool` (serde default `false` = legacy locked; verbs and new floors write `true`; the Split tool D writes `false`, which is "manual flip stays").

When a surface with `auto_split` has its heights changed (any verb, the Height tool, Shift-drag, `normalize`), the split is chosen by **fit to the bilinear surface of the four corners**: for both diagonals compute, on the 5 by 5 lattice of quarter points, the integer sum of absolute errors between the triangulated surface (`psx_grid::height`) and the bilinear surface `((4-i)(4-j)NW + i(4-j)NE + i j SE + (4-i) j SW) / 16` (compare scaled by 16, all integer). The smaller sum wins; on a tie keep the current split (so a flat or symmetric cell never flips and a stroke does not flicker). Planar surfaces are untouched.

Why this rule: at the centre both diagonals deviate from the bilinear surface by the same amount (`|NW+SE-NE-SW|/4`), so the centre cannot decide; the whole-lattice error can, and it picks the diagonal that follows the crease. It is deterministic, integer, and the same code runs for sub-sector leaves. Split-level surfaces (`heights_b` set) are not auto split.

### 4.7 Convert a legacy room (the migration)

`edit::convert_to_derived(world, room, mode)` with `mode` = `PreserveLook` (default) or `FillNewWalls`:

1. Compute the derived faces the room would have in `Derived` mode.
2. For each authored wall that **coincides** with a derived face (same edge and side, its span covers the derived span within one fine step on both ends): remove it and copy its material, `FaceUv` and `solid` into an `EdgeOverride` for those segments.
3. Authored walls that match nothing stay authored.
4. Derived faces with no authored counterpart (a legacy cliff that was left see-through): `PreserveLook` writes `EdgeOverride.hidden = true` there; `FillNewWalls` leaves them.
5. Switch `wall_mode` to `Derived` and report: n converted, n kept authored, n hidden.

Gate (acceptance of the phase): for each of the six reference levels and the starter, `all_faces` after `PreserveLook` equals `all_faces` before as a multiset of rings, materials and texels (ordering and wall course splits excluded), and the cook output has the same section contents as before except the room flags. Without the command nothing changes at all.

### 4.8 Collision of derived walls

Risers and drops are implicit in the floor sampler and ceiling slabs, as today (`docs/grid/level-format-and-runtime.md` section 5.2). Closure walls (R3) become `ColWall` records owned by the leaf, with `solid` true unless the override says otherwise. This is the one place the cook reads derived geometry for collision, and it reads the same function `faces.rs` uses (`derive::closure_walls`).

---

## 5. One UV model

### 5.1 The model

`FaceUv` (`editor/crates/psxed-project/src/face_uv.rs:8`: `offset_texels`, `rotation_deg`, `scale_q8`) is the per-face placement used by floors, ceilings, authored walls and planes. It is extended, not replaced, so every consumer gains the new controls and every existing file keeps its meaning:

```rust
pub enum UvMode {
    Inherit,                                   // default: use the material's grid default, else WORLD 32
    World { texels_per_sector: [u16; 2] },     // world aligned; 32 = today's behaviour
    Fit   { repeats_q4: [u16; 2] },            // tiles across the whole face, 1/16 tile steps; 16 = 1x
}
pub struct FaceUv {                            // + new fields, all serde-defaulted
    pub mode: UvMode,                          // Inherit
    pub turns: u8,                             // quarter turns 0..=3, applied in frame space
    pub flip: [bool; 2],                       // mirror U, V in frame space
    // existing, unchanged: offset_texels, rotation_deg, scale_q8 (the "fine" free adjustment)
}
```

`FaceUv::is_identity` returns true only when every field is default, so `skip_serializing_if` (used at `E/surface.rs:48`) keeps old files unchanged.

Resolution order: the face's `mode`, else its material's `grid_uv` default (new optional field on the Material resource, edited in the Material tab; the Inspector shows "from material" when it is used), else `World { [32, 32] }`. Applying a material to an `Inherit` face therefore takes the material's tiling, which is the "per-material default, per-face override" Manny asked for.

Mapping, in order: (1) the **frame** coordinates `(s, t)` of each corner, from the mode (5.2 and 5.3); (2) `turns` and `flip` in frame space; (3) texel scaling; (4) the existing `FaceUv::apply` (rotation about the face centre and `scale_q8`, as `place` does at `E/faces.rs:107`); (5) `+ offset_texels`.

### 5.2 WORLD

Texel = world position times density, so neighbouring faces line up and resizing a face tiles more instead of stretching. Axes: floors and ceilings use `(x, z)`; walls use `(distance along the run, -y)`, the same convention as authored walls today (`E/faces.rs:250-300`). `texels_per_sector` 32 on a 64 texel texture is 0.5 tiles per sector. The editor shows tiles per sector (`texels / texture size`), the file stores integer texels per sector so the legacy value is exactly 32 whatever the texture size (the cook's tile is a fixed 64, `E/cook/render.rs:25`).

### 5.3 FIT

Texel = fractional position inside the face's **frame** times `repeats * texture size`. "2x2 on this face" is `repeats_q4 = [32, 32]`, "3x1" is `[48, 16]`, non-integer repeats are allowed (1.5x) and simply start and end mid-tile. One-click presets: 1x1, 2x2, 4x4; free numeric fields per axis. Odd `turns` swap which frame axis the two repeat numbers refer to, so the inspector always shows repeats in screen terms.

The frame is the **logical face**, which is what the user selects:

| Face | Frame |
| --- | --- |
| floor or ceiling | the leaf square (both triangles of a split surface share it, so repeats continue across the diagonal) |
| derived wall | the **run** (4.1 table) bounding rectangle, in `(u, y)` between the run's lowest floor profile point and highest top |
| authored wall | the wall record, its bounding rectangle |
| band | the band's strip within the run |
| plane | the plane rectangle |

Because a run is a frame, "8x1" across a long derived wall repeats eight times along its whole length, not eight times per cell. The frame bounds are recomputed when the run changes length, which is the intended behaviour of FIT (repeat count fixed, tile size follows the face).

### 5.4 Storage and defaults

All integer. `repeats_q4` and `texels_per_sector` are `u16`, offsets stay `i16` texels, `turns` is `u8`. The default `FaceUv` is `Inherit`, no turn, no flip, identity fine: byte-identical to today for every existing face. `UvMode::World` is what `GridPlane.uv_mode = Tile` means (anchored at the plane's top-left, `grid-runtime:E/plane_faces.rs`), `Fit {16,16}` is `Stretch`.

### 5.5 PS1 constraints and the automatic split

Packet UVs are `u8` (`Quad.uv`, `P/format.rs:451`). One primitive can therefore span at most `255 - phase` texels per axis, where `phase = start texel mod P` (P the texture size on that axis, 64 for current materials). With phase 0 a 64 texel texture repeats at most **4 times** (the far edge clamps to 255, one texel short of 256, a [derived] 0.4 percent shift at the far end); a non-zero phase (an offset, a half-tile start) reduces the worst case to **3** [derived]. Repetition inside a primitive needs the GPU texture window (GP0 E2), section 9.6.

The cook therefore splits a face whose span exceeds the limit into the fewest equal pieces, cutting at tile boundaries where it can. Until the window path lands (phase T3) the limit is **1 tile** per primitive, which is also today's behaviour (`rebase_texels` clamps and counts `uv_overflow`, `E/cook/render.rs:115`); T3 raises it to 4. The split is on faces after stitching, so pieces share vertices and the T-junction repair of 9.3 covers their seams.

**Reporting.** `uv::report(face) -> UvReport { effective_repeats: [f32;2], pieces: [u8;2], quads_added: u32, phase_ok: bool }` is a pure function shared by the Inspector and the cook; the cook sums `quads_added` into `CookReport.uv_split_quads`. The Inspector always prints the effective result:

```
Tiling   (o) World   ( ) Fit
  World: 32 texels/sector   = 0.5 tile/sector     effective on this face: 1.0 x 0.5 tiles
  Fit  : [ 2 ] x [ 2 ]   presets [1x1][2x2][4x4]  effective: 2 x 2 tiles on this face
  Offset [  0 ][  0 ]  Turn [<][>]  Flip [H][V]  Fine: rotation 0, scale 100%
  from material: no        !  cook splits this face into 4 quads (+3)   (repeat 8x1 exceeds 4x per primitive)
```

### 5.6 Merging, near cap and continuity

The cook merge (9.2) joins two quads only when one affine texel map reproduces **both** quads' corner texels (the linearity test already in `E/cook/lod.rs`, generalised). WORLD faces satisfy it across neighbours, FIT faces from different logical faces do not (their frames restart), so they never merge across a frame boundary, which is correct. The near cap (9.5) splits large quads near camera-reachable space; pieces keep the continuous map, so splitting changes cost, never the picture.

### 5.7 Unifying with planes

Today planes compute their own raw texels (`grid-runtime:E/plane_faces.rs`, `raw_texels`) for Tile and Stretch. After this change `plane_faces` calls the shared `uv::frame_texels(frame, &FaceUv, material_default)` with the plane rectangle as frame and the plane's top-left as the WORLD anchor. `GridPlane.uv_mode` stays as the legacy way to say Tile or Stretch and is used only while `uv.mode == Inherit`; a non-Inherit `uv.mode` wins. The change is small and separate (phase T2 touches `plane_faces.rs` only after planes merge), so the planes agent is not blocked.

---

## 6. Horizontal wall bands

A band is a horizontal strip of a wall with its own material and UV rule. Bands are the answer to "trim at the base, main wall, frieze at the top" and apply identically to derived faces (R1, R2, R3) and authored walls.

### 6.1 Data

```rust
pub enum Anchor { Bottom(i32), Top(i32), Abs(i32) }   // gu: above the wall's bottom edge, below its top edge, or room-relative height
pub enum BandV   { World, FromBottom, FromTop }       // where V = 0 sits for the band's texture
pub struct Band  { pub material: Option<ResourceId>, pub uv: FaceUv, pub v: BandV }
pub struct BandSet { pub name: String, pub lines: Vec<Anchor>, pub bands: Vec<Band> }   // bands.len() == lines.len() + 1
```

Band 0 runs from the wall's bottom to line 0, the last band from the last line to the wall's top. `Band.uv.mode` is the shared UV model, so a frieze is `Fit { [16*n, 16] }` (n tiles along, exactly one texture high between its lines), a base trim `FromBottom` with `World` along u, and the main body `World` as today.

### 6.2 Where bands are stored and resolved

Three levels, most specific wins, nothing is baked into faces in the document:

1. **Room default**: `room.defaults.bands: Option<u8>` points at a set in `room.band_sets`. Applies to every derived face and every authored wall in the room that has no other assignment.
2. **Run override**: `EdgeOverride.band_set` (4.4) for derived segments, and for authored walls a new `GridWall.band_set: Option<u8>`. The Bands tool writes the same index on every segment of the selected run.
3. **None**: `band_set = Some(255)` explicitly means "no bands on this run" (a plain wall in a room that has defaults).

Editing is copy-on-write: changing lines or a band's material while the scope is "Run" clones the set, appends it, and points just those segments at the clone; scope "Room" edits the set in place. Unreferenced sets are dropped by `normalize`. The set count is capped by `MAX_BAND_SETS_PER_ROOM`.

### 6.3 Geometry pipeline

Per segment of a wall run:

1. **Profile**: the face region between `bottom(u)` and `top(u)` (derived R1/R2/R3 from 4.1; for an authored wall its own four heights). Trapezoids and triangles are normal.
2. **Anchors**: `Bottom(o)` means `bottom(u) + o`, so a trim follows a sloped floor; `Top(o)` means `top(u) - o`, so a frieze hangs from a sloped ceiling or a ledge; `Abs(y)` is horizontal, a datum line (a cornice at a fixed height across terrain that rises and falls). For stacked authored segments on one edge the anchors refer to the **edge**: Bottom is the lowest segment's bottom, Top is the highest segment's top, so one trim runs at the base of the stack, not at the base of every segment.
3. **Openings** (4.5) cut rectangles out of the region first. A lintel above a door therefore keeps the bands that run through it (the same lines, clipped), which is what makes a door in a banded wall look continuous.
4. **Lines**: line `i` is the function `y_i(u)`; the effective cut is `e_i(u) = clamp(y_i(u), e_{i-1}(u), top(u))` so lines never cross and a band whose strip collapses simply has zero height at that `u`. Band `i` is the region between `e_{i-1}` and `e_i`. Each of `e_i` is piecewise linear with at most one break inside a segment (where an `Abs` line meets a sloped edge), so a band polygon has at most six corners.
5. **Output**: faces of three or four corners. A polygon with more is fan-split **in `faces.rs`** into quads and triangles; the cook keeps its four corner limit (`E/cook/render.rs:227`) and needs no change for bands.

### 6.4 Slopes, openings, stacks, cracks

* **Sloped floors and ceilings**: handled by the anchors; the trim is a parallelogram-like trapezoid that follows the slope, a datum `Abs` line cuts it into a triangle and a trapezoid at the crossing.
* **Stacked authored segments** and **openings**: as in 6.3 steps 2 and 3.
* **Vertical edges between neighbouring segments**: bands cut on the same lines at both sides produce identical vertices on the shared vertical edge (both sides evaluate `y_i(u)` at the same `u` from the same edge heights). Where two neighbours use different band sets the cut heights differ along that edge, which is a T-junction; the cook's generic repair (9.3) inserts the neighbour's vertices into the longer side. This is also the only case where band count inflates quad count beyond `segments x (bands)`.
* **Courses disappear**: the 1024 gu splits (`E/faces.rs:317`) existed so a world-aligned texture fits a u8 span. With the UV splitter of 5.5 a tall plain wall is one face. In phase T4, `faces.rs` keeps emitting courses for rooms with `wall_mode = Authored` and no band sets (byte-identical legacy output) and stops emitting them for everything else.

### 6.5 Editing bands (the Bands verb, E)

Pick a run (click a wall; Shift extends along the run; Ctrl+A selects the room's walls), then in the Inspector or in the viewport:

* **Cut**: hover the wall, a horizontal rule previews at the pointer height snapped to clicks; click inserts a line. A toggle chooses the anchor kind (nearest edge decides by default: within the lower quarter of the wall is Bottom, upper quarter Top, otherwise Abs).
* **Move**: drag a line vertically; numeric field in clicks.
* **Remove**: click a line then Delete, or the minus button.
* **Material**: click a band in the viewport, click a material in the palette; the Paint tool also targets bands when the pointer is over a band of a banded wall.
* **Scope**: Run (the selected segments) or Room (the default set). A one-line preview reads "applies to 12 wall segments".

Undo: one step per insert, per drag, per material change.

---

## 7. Verbs

### 7.1 The interaction model every verb shares

```
 1. REGION   marquee | paint brush | path          (or reuse the current Select/Region)
 2. VERB     press its key or click its button    (Region persists, verbs chain)
 3. AMOUNT   drag vertically anywhere, or type it  (snaps to clicks; Alt = fine 64; Shift = one sector)
 4. COMMIT   release the mouse (one undo step); Esc cancels and restores exactly
```

Implementation shape, so one agent can build all verbs the same way:

* Pure functions in `E/edit/verbs/*.rs`: `begin(world, &Region, &Params) -> VerbDrag` captures the touched leaves plus the one-ring of neighbours; `apply(world, &mut VerbDrag, amount) -> Applied` restores that snapshot and recomputes from absolute values, then calls `tree::normalize` on the dirty set. This is the contract `HeightDrag` already has ("every update then sets absolute heights from the start values, so a drag can be repeated, reversed or cancelled exactly", `E/edit/heights.rs:1-9`). The preview is the live document plus an overlay, as the Height tool does now, so derived walls, stitching and the auto split show live because they are derived.
* Gestures in `U/verbs/*.rs` add variants to `Gesture` (`U/mod.rs:223`): `Region(RegionGesture)` and `Verb(VerbGesture)`. Undo snapshots the document at the first change (the `before` field of `HeightGesture`, `U/mod.rs:212`).
* The amount maps screen pixels to clicks with the same `pixels_per_unit` machinery as the Height tool (`U/mod.rs:212`); in the Top view a vertical mouse delta is used. The option row shows the live value in clicks, gu and ru (for example "+2 clicks, 512 gu, 40 ru").
* Every verb takes `follow_ceiling` and `material` parameters and writes ordinary fields; nothing is stored about which verb made it.
* Every verb reports refusals through the status line (cells skipped, room growth refused at `MAX_ROOM_SIDE`, `MAX_LEAVES_PER_ROOM`), never silently.

### 7.2 Smart paint rules and per-face override

`room.defaults.rules: PaintRules` (editable in the Rooms panel, optional):

* `floor_by_class: [Option<ResourceId>; 3]` for Flat, Slope (normal y at or above 2867/4096, the motor's limit, `character_motor.rs:38`) and Steep.
* `trim: Option<ResourceId>`: the material of band 0 whenever a verb creates a derived face in a room with no band default.
* `wall: Option<ResourceId>`: the `wall_material` verbs stamp on the leaves they raise.

Rules run **only inside verbs**, and only on surfaces the verb created or whose slope class it changed. A surface the user painted by hand sets `GridSurface.paint_locked = true` (the Paint tool and the Inspector material picker write it; serde default false) and rules skip it. So rules give a good first pass; everything they write is an ordinary material field, hand-editable afterwards, and the Paint tool or Inspector is the per-face override.

### 7.3 Raise / Lower (U)

| | |
| --- | --- |
| Input | A cells region. |
| Gesture | Drag up raises, down lowers. Tab toggles **Offset** (every corner moves by the amount, shape kept; the default on a non-flat region) and **Level** (every corner set to the reference height plus the amount; the default on a flat region, where the two agree). The reference is the height under the pointer at press. |
| Preview | A translucent plane at the new height over the region, the new derived walls (live), and the amount readout. |
| Writes | The floor corner heights of every region leaf; sets `wall_material` from the palette or the paint rules; `auto_split` surfaces re-split. Corners on the region boundary move with the region and the cells outside keep theirs, so a cliff and its derived walls appear; **no weld is propagated outward** (this is the one place the Height tool's link behaviour is deliberately off). |
| Ceiling | `follow` ON moves each region ceiling by the same delta (headroom constant). The Inspector button "Match ceiling" sets each ceiling corner to the floor corner plus a headroom `H`; the default `H` is the body height rounded up to a click plus one click, 4 clicks at the default scale [derived: 48 ru body, `character_motor.rs:20`, 20 ru per click]. |
| Edge cases | Region cells without a floor are skipped and counted. A platform whose edge meets a neighbouring big leaf subdivides the neighbour through balance. A raised floor reaching its ceiling clamps one click below it and says so. Offset mode on a region that includes a sloped surface and a flat one keeps each shape. |
| Undo | One step per drag; typing a number is one step. |

### 7.4 Sculpt (S)

Brush on the corner lattice. Parameters: mode Raise, Lower, Flatten, Smooth, Noise; radius in clicks (`[` and `]`); strength; target Floor, Ceiling or Both.

* **Field.** A stroke edits a virtual high-resolution height field at click resolution (shared points, welded exactly as `WeldIndex` welds corners at gesture start; a point across a cliff has one value per side). Raise and Lower accumulate a fractional amount per lattice point in the gesture, so slow strokes progress, and write multiples of the fine step (64). Flatten sets points to the height under the pointer at press (Alt+click samples another). Smooth replaces a point by the mean of its welded 4-neighbours. Noise adds a deterministic value from a hash of `(seed, point)`, seed fixed per gesture so redo and undo are exact.
* **Commit.** For each affected leaf, the field samples are compared with what the leaf can represent. A leaf represents the field **exactly** when its triangulated surface (either diagonal) matches every lattice sample inside it. If not, the leaf is **subdivided** one level and the test repeats on the children, down to level 4. Constrained hanging corners (3.2) then force neighbours to subdivide; the loop runs to a fixpoint. Where all four children of a node are exactly representable by the parent, `normalize` **merges** them again. Both thresholds are exact equality on the 64 gu lattice, so there is no tolerance to tune and no flicker. A lossy "Simplify" command merges within a tolerance chosen by the user (default one fine step, [proposal]).
* **Cost control.** The subdivision count is shown live in the option row ("+37 leaves, +2.2 KiB collision [derived]") and a stroke stops subdividing at `MAX_LEAVES_PER_ROOM`, reporting it.
* **Edge cases.** Leaves with diagonal walls, `heights_b`, or authored walls on an edge that a subdivision would split are refused or the walls are split along the new edge and kept; the status line says which. Ceiling target applies the same logic to ceilings. Cells without a surface are not created by Sculpt.
* **Undo.** One step per stroke (press to release).

### 7.5 Ramp (A)

| | |
| --- | --- |
| Input | A cells region and a direction. |
| Gesture | After choosing the verb the four sides of the region's bounding box light up; click the **A edge** (it keeps its height), then drag vertically to set the B edge's rise. R turns the direction by 90 degrees. Every corner gets `h = hA + amount * t`, `t` the corner's normalised position along A to B, rounded to the fine step. |
| Preview | The sloped surface as a ghost, the slope class colour (green walkable, orange steep) and the slope readout "rise per sector" and angle. Tab clamps the amount to the steepest walkable slope (normal y = 2867/4096, about a 1.02 : 1 rise to run [derived]). |
| Writes | Floor corners (and ceilings with `follow`); `auto_split` re-split so the diagonal follows the crease. A planar ramp over many cells is exactly the case big leaves help (one planar L0 leaf, section 3.2). |
| Edge cases | A region that is not a rectangle ramps by position, so L-shapes work. Cells without floor are skipped. |
| Undo | One step. |

### 7.6 Stairs (T)

| | |
| --- | --- |
| Input | A path: click the start cell edge, then one or more waypoints, Enter finishes (two points make a straight flight, more make an L or a zig-zag with a landing at each corner). Width in clicks (Inspector, default one sector). |
| Amount | Drag vertically for the total rise, or type it. Option chips: **Steps** (default) or **Ramp** (the same path as a ramp). |
| Step sizing | A **riser option** chooses the riser: `1 click` (default, [proposal]: half the climb limit, comfortable), `Max` (the motor's step-up height, 40 ru today and soon a config field: `character_motor.rs:27`; 40 ru = 512 gu = 2 clicks at the default scale [derived]) or a typed value, never above the limit. Number of steps `n = round(rise / riser)`, then the riser is `rise / n` snapped to the fine step and the verb re-picks the nearest `n` whose riser snaps cleanly; tread depth `= run / n` snapped to whole clicks. If the run has fewer clicks than `n` the verb refuses with "too steep for treads, use Ramp or lengthen". Remainder clicks go to the first tread. |
| Subdivision | A tread of at least one sector needs no subdivision (each cell is a tread, the riser is the derived wall between cells). A tread of half or one quarter of a sector subdivides the cells along the run to level 3 or 4 (phase T11; before the quadtree lands, phase T7 supports whole-sector treads only and refuses finer). Cells off the stair stay untouched. |
| Preview | Ghost treads and the readout "12 steps, riser 1 click (20 ru), tread 1/2 sector". Orange when a riser exceeds the step limit (cannot happen unless the user typed an override). |
| Writes | Flat floor tread surfaces at `base + i * riser`; ceilings per `follow`; floors are created where missing. Walls come from derivation. |
| Edge cases | A path crossing existing slopes overwrites; the five-point footprint sampling of the motor can step over a tread narrower than its radius, so the verb warns when tread depth is below the character radius (config); the headless stair-climb test in section 14 decides the minimum. |
| Undo | One step. |

### 7.7 Room (B)

Input: a rectangle in the Top view or on the floor plane in 3D, then a vertical drag for the **height** (clicks above the floor). Creates floor and ceiling over the rectangle as maximal aligned leaves (so a 12 by 12 room is nine level-0 leaves, "big cells by default"), flat at the current base height, ceiling at `base + height`, `auto_split` true, materials from the palette or the paint rules. No walls are written: the perimeter faces void, so derivation closes it (R3). Where the rectangle overlaps existing cells it overwrites floor and ceiling and derivation reconciles the edges (a room merged into a corridor opens by itself where heights agree). In an `Authored` room the verb asks to convert first, so the rule "walls come from derivation" has no exceptions. Undo: one step.

### 7.8 Corridor (K)

Input: a path of waypoints (Enter finishes) and a width in sectors (Inspector; widths that are not whole sectors subdivide the edge cells, flat, so the cook merges them back). Corners are mitred squares; legs are axis aligned. Floor height is taken from the first waypoint and may be changed per waypoint by a vertical drag on that waypoint (the leg between two heights becomes a Ramp). Ceiling height from the Inspector (headroom). Writes floor and ceiling leaves over the swept footprint. Where the corridor meets an existing room, derivation opens the junction if floors agree (a doorway for free) and builds a drop or riser if they do not. Undo: one step.

### 7.9 Opening (I)

Pick a wall segment (hover shows the run), drag along it to set the horizontal extent (clicks), release, then drag vertically to set the lintel height; chips choose **Door** (bottom = floor) or **Window** (a second vertical drag also sets the sill height). For a derived face it writes `Opening` records (split at leaf boundaries); for an authored wall it splits the record into sill and lintel pieces. Preview shows the cut rectangle and the lintel. Across a room border the verb writes the matching opening on the neighbouring room's closure wall. Edge cases: an opening narrower than the segment is allowed to any click; an opening on a hidden or fully covered edge is refused. Undo: one step.

### 7.10 Ceiling follow

Not a separate tool. It is a chip on Raise, Ramp, Stairs, Sculpt and Room (`follow`, default on for rooms that have ceilings) and the Inspector command "Match ceiling" (7.3). Both write ordinary ceiling corner heights.

### 7.11 Stamps (Y)

Save: with a region selected, Y opens the Stamps panel; "Save selection as stamp" names it and stores leaves, overrides, openings and the band sets they use (3.8). Place: choose a stamp, a ghost follows the pointer on the floor; R turns it 90 degrees, H and V flip (the same keys as the paste ghost, `U/keys.rs:45-51`, because a stamp is a persisted paste), click stamps, Shift keeps stamping, Esc ends. Heights are relative to the floor under the pointer. Stamping that needs finer or coarser leaves than the target has splits the **target** leaves (lossless) and runs `normalize`. Undo: one step per stamping.

### 7.12 Keys

Existing and untouched: `V F C W H P D O`, `?`, `.`, `L`, `R` (wall direction, ghost turn), `Delete`, `PgUp PgDn [ ]` (height nudge in Height and Select), `Esc`, `Ctrl+A/C/V/Z/S`.

| Key | Verb | Notes |
| --- | --- | --- |
| U | Raise / Lower | |
| S | Sculpt | `[` `]` brush radius (these keys nudge heights in the Height tool; inside a verb they mean radius) |
| A | Ramp | R turns the direction |
| T | Stairs | Enter finishes the path |
| B | Room | |
| K | Corridor | Enter finishes the path |
| I | Opening | |
| E | Bands | |
| Y | Stamps | |
| Alt+W | Walkability overlay | Alt is also the fine snap while dragging; as a chord with W while idle it is free |
| Alt+P | Performance overlay | |
| Tab | cycle the active verb's mode | |

Letters kept free for the planes tool and later: `N G J M Q X Z`. If planes chooses one of the letters above, that verb moves to the Shift chord of the same letter; nothing else changes.

---

## 8. Editor surfaces

Following the UX audit: one home per function, the Inspector shows one context at a time, a visible 3D/Top switch (`docs/grid/ux-audit.md` section 4.1 and 4.2). This design does not redo that audit; it adds to it.

### 8.1 Tool strip

The strip (`U/strip.rs:10`) grows from one group of eight to four labelled groups that wrap as today. The Build group is the verbs; Draw is the existing manual tools (Floor, Ceiling, Wall stay because they are the escape hatch and the way to author on legacy rooms).

```
 PICK            DRAW (manual)                                  BUILD (verbs)
 [V Select][H Height]  [F Floor][C Ceil][W Wall][P Paint][D Split][O Place]  [U Raise][S Sculpt][A Ramp][T Stairs][B Room][K Corr][I Open][E Bands][Y Stamp]
 -----------------------------------------------------------------------------------------------------------------
 options of the active tool:  Raise: [Offset|Level] Follow ceiling [x]  Material: brick   Amount: [ +2 ] clicks
                                                                              Snap 256 (Alt 64)   (?)   [Walk][Perf][3D|Top|Split]
```

The Place options and `?` page stay where they are (`U/strip.rs:76-109`); the `?` page (`U/help.rs`) gains the Build keys. The key hint strings are generated from one table, which also fixes the triple duplication the audit noted (`ux-audit.md` section 5, last item).

### 8.2 Inspector contexts

A header line names the context; only that context's controls show. Contexts: Face(s), Wall run, Region (with the active verb's parameters), Band, Node, Room, World. Selecting a wall run shows:

```
+- Inspector ----------------------------------------------------+
| WALL RUN   12 segments, derived (R1 riser, owner: room 2)       |
|   Material   [ brick_a ]  (from owner wall_material)            |
|   Tiling     (o) World  ( ) Fit                                 |
|     World: [32] texels/sector = 0.5 tile/sector                  |
|     effective on this run: 0.5 tiles per sector                 |
|     Offset [ 0 ][ 0 ]  Turn [<][>]  Flip [H][V]                 |
|   Bands      scope: (o) This run  ( ) Whole room                |
|     top    ----------------------------  Top - 1 click          |
|       frieze   [ cornice_b ]  Fit  [ 8 ]x[ 1 ]                  |
|     body   ----------------------------  Bottom + 1 click      |
|       main     [ brick_a ]   World                              |
|     base   ---------------------------- (wall bottom)          |
|       trim     [ plinth_c ]  FromBottom                        |
|   Openings   2  [select]                                        |
|   Hidden [ ]  Solid [x]                                         |
+-----------------------------------------------------------------+
```

A Region context shows the region count ("12 cells in 3 leaves"), the verb's parameters, `follow ceiling`, `material`, and the live amount. The Rooms panel (left dock) gets per-room: wall mode (Authored / Derived with the Convert button), open wall height, default wall material, default band set, paint rules, and the band set list. The Materials palette stays the single material home; a swatch has a small badge when the material has a grid tiling default. The Material tab gains the `Grid tiling default` row (WORLD texels per sector or FIT repeats).

### 8.3 Gesture states

Raise with a 3 by 2 region, drag in progress, Top view:

```
   Top view                                   amount: +2 clicks (512 gu, 40 ru)   Level | Offset
      . . . . . . . . .
      . . .+--+--+--+. .         # region cells         ~ derived riser edges (live)
      . . .|##|##|##|. .         step limit: 40 ru, riser 40 ru: climbable
      . . .+--+--+--+. .
      . . .|##|##|##|. .         status: Raise  region 6 cells  follow ceiling ON
      . . .+--+--+--+. .         [Esc] cancel   [Alt] fine   [Tab] mode
      . . . . . . . . .
```

Stairs, path being placed, Top view, a flight of 2 sectors (8 clicks) rising 4 clicks:

```
   A o-------+-------+-------o B          path A -> B, width 1 sector
     | | | | | | | | | | | | |           ghost treads, one per 2 clicks (1/2 sector)
   4 steps, riser 1 click (256 gu, 20 ru), tread 1/2 sector     limit 40 ru: ok
   subdividing 2 cells to level 3; collision +2 leaves [derived]
```

Bands, cutting a line, 3D side view of a wall run:

```
   top   +---------------------------+--- frieze (Fit 8x1)
         |                           |
   - - - + - - - -  new line  - - - -+--- click to insert, drag to move
         |                           |
   base  +---------------------------+--- trim (FromBottom)
```

---

## 9. The cook

### 9.1 Pipeline

```
validate -> normalize check -> faces (derive, bands, stitch, UV split)      shared with the preview, `faces.rs`
         -> snap to ru, weld vertices, bake light per corner                 as today (`E/cook/render.rs`)
         -> [optimise A] merge -> generic T-junction repair -> pack
         -> collision (leaves, index, sub-blocks)                            `E/cook/collision.rs`
```

The cook keeps its rule that it never re-derives geometry (`E/faces.rs:1-12`); everything new (derivation, bands, stitching, UV splitting) lands in `faces.rs` or modules it calls, so the editor preview shows exactly what is cooked. The cook keeps its four-corner limit (`E/cook/render.rs:227`) because `faces.rs` only ever emits quads and triangles.

Until phase T14 the cook **expands every leaf larger than a sector into per-sector faces** (a planar polygon clipped to the sector squares is trivial because leaves above level 2 are planar, 3.2) and emits sub-sector leaves as their own quads. So phases T9 to T13 render exactly as cost-efficiently as today for flat big leaves and slightly worse where a sculpt created many small leaves; the heatmap (11.2) shows it and it is the thing T14 and T15 fix.

### 9.2 Merge (optimise A, T14)

The cook merges neighbouring quads into one quad when all of these hold:

* same material, same sidedness and blend, and **coplanar after rounding to ru**;
* **UV continuity**: one affine texel map reproduces every corner texel of every merged quad (the linearity test of `merge` in `E/cook/lod.rs`, generalised from horizontal 2x2 to planar rectangles in the face's own parameter space: floors, ramps, ceilings, and walls in `(u, y)`). WORLD faces satisfy it, FIT faces from different logical faces do not (5.6). Texels are compared **before** any rebase, and the merged quad is then rebased once, with no clamp at 63 (the old clamp is `E/cook/lod.rs:311`; it is one of the causes of the checker shifts, 10.4);
* **light**: the merged quad's four baked corner colours, interpolated, reproduce the baked colour at every original corner it swallows within `LIGHT_TOL` per channel. A quad near a point light does not merge; an ambient-only quad merges freely. `LIGHT_TOL` is [proposal], reported with the merge statistics so it can be tuned; the shadows come from the same `line_of_sight` the bake already uses (`E/cook/mod.rs`);
* **size**: extent caps of 9.5 and the UV span limit of 5.5 (at most 4, or 3 for non-zero phase, tiles per axis per primitive).

Shape: maximal axis-aligned rectangles in the parameter space, found greedily as `lod.rs` does. A merged quad spans several render cells, so it is listed in **every** cell it covers through an indirection (`CELL_INDIRECT`, 10.1) and deduplicated per frame by a quad stamp; ordinary cells keep the contiguous run and pay nothing.

### 9.3 T-junction stitching

Two mechanisms, one in `faces.rs` for correctness from phase T4, one in the cook for merged output.

**(a) Document stitching, in `faces.rs`.** Where a coarse leaf has finer neighbours across an edge (3.2: with 2:1 balance exactly one hanging vertex M per edge), the coarse leaf's polygon includes M as a boundary vertex, and polygons of more than four corners are fan-split into triangles in the same function. Walls along that edge are derived per finer segment (4.1), so their bottoms and tops pass through M. Band polygons insert the cut heights of a neighbouring segment on a shared vertical edge (6.4). A coarse leaf with k hanging vertices costs `k` extra triangles over the one quad it would otherwise be; with balance k is at most 4 per leaf. Constrained corners (3.2) make M lie exactly on the coarse edge in grid units; the rounding to ru can move it up to half a unit, so M is inserted as a real vertex rather than assumed to lie on the line. On the GPU the integer screen rounding of a vertex that lies on an edge but is not part of the neighbour's polygon opens one pixel gaps; inserting the vertex is what closes them.

**(b) Generic repair in the cook (T14).** After merging, for every quad edge the cook looks up vertices of the room's weld table that lie strictly inside the edge segment (exact integer collinearity in ru, the lattice makes the candidates few) and fans the quad through them. This also covers merged-next-to-unmerged edges, quads next to wall tops and merged walls next to bands. Free planes are excluded (they are never welded to the lattice, `grid-runtime:E/plane_faces.rs` header). Node-LOD seams are the separate mechanism of 10.4.

### 9.4 Courses

The 1024 gu course split (`E/faces.rs:317`) is kept for `Authored` rooms with no band sets, so legacy output is byte identical. Every other face is cut only by the UV splitter (5.5) and by band lines, so a tall plain wall is one face and the cook sees fewer, larger quads.

### 9.5 Near-camera size cap

Large quads near the camera cost twice: affine texture warp, and the near-plane split (about 9 to 11k cycles per near tile [measured], against about 800 per ordinary quad [measured]); `GRID_RENDER_PROFILE` already splits once below 170 depth units (`P/render/mod.rs:58-61`). The cap keeps the merge from making that worse.

* The cook computes a **reach field**: the set of leaves with a walkable floor (the `FLOOR_WALK` bits, `E/cook/collision.rs:88`), each expanded to the volume a camera can occupy above it (floor to floor plus the camera arm height, read from the project's camera resource, not a constant).
* For each candidate merged quad, `d` is the distance in sectors from the quad's box to that volume. The allowed extent per axis is `cap(d) = clamp(floor(d / K), 1, E_max)` sectors with `K = 2` and `E_max` = the UV limit of 5.5 [proposal for K; the existing far LOD uses a 2x2 merge beyond 6 sectors, a ratio of 3, `E/cook/lod.rs:29-31`].
* Consequences: walkable floors (d = 0) are never statically merged beyond one sector, ceilings two or more sectors overhead and floors of pits you never visit merge freely, and **vertical** extent of a wall is not capped by distance (depth barely changes along a vertical wall seen from a camera at roughly constant height), only horizontally along a wall seen obliquely. Every threshold is reported by the cook and shown by the heatmap so it can be tuned from data; none of them blocks building.

### 9.6 Texture-window repeat (T3) and where window state lives

* **Material table** (`P/format.rs:128`, `Material.reserved`): becomes `tex_shape: u16`, `bits 0-3 = log2(width in texels)`, `bits 4-7 = log2(height)`, 0 = unknown (legacy, treated as 64 by 64 and never repeat-eligible). Only power-of-two sizes from 8 to 128 are eligible, the limits of the VRAM window atlas (`sdk/crates/psx-vram/src/lib.rs:357`).
* **Quad** (`P/format.rs:451`, the `reserved: u8` becomes `flags2`): bit 0 `REPEAT` = the packet uses the material's window and **raw** UVs (no origin added); bit 1 `MULTI` = listed in several cells, dedupe by stamp (T14).
* **Runtime** `GridMaterial` (`P/render/mod.rs:67`) gains `window_word` (the E2 command for the texture's placement: mask from the size, offset from `slot.texture_window.origin_texels()`, `engine/examples/editor-playtest/src/grid_runtime.rs:143`). The fast packet already has a window word in slot 1 (`P/render/emit.rs:235,273`); a `REPEAT` quad writes `window_word` there and `uv` without the `+ origin`, others write the reset and add the origin as now. The near path sets `texture_window_word` and zero `uv_offset` for `REPEAT` quads (`AffineSurface` already has a `texture_window_word` field, `P/render/emit.rs:365`; T3 verifies the splitter writes it into every sub-packet and adds a test). No packet grows.
* **Batching.** Packets are linked into the depth-sorted ordering table, so consecutive draws in memory are not consecutive on the GPU; each packet is self-contained, which is why every packet carries its own window word today. Grouping by window therefore cannot be done without breaking depth order and this design does not try. Whether the GPU pays for a window change between consecutive textured primitives (texture cache) is **[hypothesis]**; the experiment is one step in the hardware-test suite (extend `hardware-tests`, no bespoke disc): draw N textured quads alternately with equal and with differing windows and compare time, run headless first, then on the console. If it costs, the fix is a stable sort by material inside each cell (equal-depth ties only), which is safe to add later.
* **Cook rule.** `REPEAT` is set only when the material is eligible and the quad's UV span exceeds one tile (or non-zero phase needs raw UVs); all other quads are byte-identical to v2 apart from `flags2 = 0`.

### 9.7 Bands and openings in the cook

They are already faces by the time the cook sees them (6.3). The cook adds only: the opening sill and lintel pieces become `ColWall`s with a partial span for R3 and authored walls (10.2); the band set index is not stored at all (the geometry carries it).

### 9.8 Free planes

Planes stay `QUAD_PLANE` runs appended **after** all cell quads, as in the planes branch; the merge and repair passes run on cell quads before planes get their indices, so `PlaneRef.quad/count` stay contiguous. After the UV unification (5.7) a plane is cut by the shared UV splitter instead of the per-sector cut, which with T3 turns a 4x4 tiled plane into one quad.

---

## 10. Format, collision and the runtime tree

### 10.1 PGL v3 (phase T3)

| Change | Where |
| --- | --- |
| `Material.reserved` -> `tex_shape` | `P/format.rs:128` |
| `Quad.reserved` -> `flags2` (`REPEAT`, `MULTI`) | `P/format.rs:451` |
| `RCell.flags` bit 2 `CELL_INDIRECT`: `first/count` index a new `CELL_REFS` list (`u16` quad ids) instead of `QUADS` (T14; ordinary cells unchanged) | `P/format.rs:475`, 9.2 |
| `VERSION = 3`, `MIN_READER = 3`, `LAYOUT_HASH` mixes the new fields | `P/format.rs:27,608` |

Golden fixtures per version are kept (`level-format-and-runtime.md` section 3.2). v2 chunks are not read by v3 readers; cook and guest ship together.

### 10.2 PGL v4: leaf collision (phase T10)

The collision block keeps its shape (section order, 20 byte `Sector`, `SectorExt`, `ColWall`) and gains three sections; nothing in the render block changes.

| Section | Record | Meaning |
| --- | --- | --- |
| `SECTORS` (id 0) | `Sector`, 20 B | now one record per **leaf**, not per sector. Flags bits 13 to 15 hold the leaf level 0 to 4. |
| `SECTOR_INDEX` (new) | `u16` per sector, `cz*width+cx` | bit 15 = SUB (the sector is subdivided, the id names a sub-block), bit 14 = CLEAR (the per-sector flat/headroom summary that today lives in `SECTOR_CLEAR`, `P/format.rs:256`, moved here because it depends on the sector's neighbours, not on the leaf), bits 0 to 13 = leaf id or sub-block id, `0x3FFF` = no leaf |
| `SUBBLOCKS` (new) | 16 x `u16` = 32 B per block | row-major 4x4 table at click resolution; each entry is the leaf covering that quarter sector (a level 3 leaf appears four times) |

`RoomHeader` gains `origin_mod` (the room's world origin modulo 4 sectors, one byte from `reserved`) so a leaf's local origin is `((cx + origin_mod) & !(n-1)) - origin_mod` for a leaf of n sectors, one and-and-subtract. `CHUNK_TREE` (chunk flag bit 4) is set only when the room has any leaf that is not level 2; otherwise `SECTOR_INDEX` has count 0, `SECTORS` stays dense, and the reader takes today's path with no extra load, so **uniform rooms and all six reference levels cost exactly what they do now**.

Point query (floor under a body, the hot path), no loop and no recursion:

```
i   = SECTOR_INDEX[cz*width + cx]                      // load 1 (absent for uniform rooms)
id  = i & 0x3FFF
leaf = if i & SUB { SUBBLOCKS[id*16 + qz*4 + qx] } else { id }   // load 2, only for subdivided sectors
rec  = SECTORS[leaf]                                   // the 20 byte record
size = (4*S) >> level(rec); (lx, lz) = offsets inside the leaf
y    = height_at(rec.floor, split, lx, lz, size, inv24[level])    // P/height.rs:85
```

`qx = lx_sector / (S/4)` uses a second exact reciprocal built like the sector one (`Recip::for_sector(S/4, S)`, `P/collision/world.rs:68`); the five `inv24` values (one per level) are computed at load. Cell lookup in the room keeps the existing exact reciprocal (`P/collision/world.rs:99`). Rays keep the existing cell-skipping walk and descend into a sub-block only when crossing a SUB sector; the tree (10.4) is used by rays in the optimise phase. `ColWall` gains a span (`reserved: u16` = `from:u8, to:u8` in quarter-edge units) for openings; geometry stays implied by the owning leaf and edge. Planes copy into the extension of the leaf (every sector the plane overlaps resolves to the same leaf, so the list is shared, and a sector in a big leaf tests the plane list of the whole leaf, a few extra tests per plane, [estimate] small).

Hard limits written into the cook: `S % 4 == 0` when any sub-sector leaf exists; 14 bit ids; `COORD_LIMIT` and `MAX_CORNER_DELTA` (`E/cook/collision.rs:30`) apply per leaf, so a very tall level-0 leaf with a wide corner spread is refused with the leaf named.

### 10.3 The shared height function

`GridWorld::floor_height_at` (`E/world.rs:92`) and the runtime already call the same `psx_grid::height` code, parameterised by the edge length `s` and its reciprocal (`P/height.rs:85,115`). Leaves call it with `s` = the leaf edge. The overflow argument of `level-format-and-runtime.md` section 2.5 holds with `s` up to `4S`: `amount * inv24 <= 2^24 + s` still fits `u32`, and the corner delta bound is per leaf. `floor_height_at` is changed to find the leaf (`LeafMap::leaf_at`) and a test asserts equality with the runtime over every point of every leaf of the reference levels and of randomly generated quadtrees (14).

### 10.4 The runtime tree (optimise phases T15 and T16)

Layout, per room, pointerless and integer only:

```
ROOTS      u16 per level-0 block (room rect rounded up to 4 sectors), node id or EMPTY
NODES      8 B per node, breadth first, children of a node stored contiguously (implicit indexing):
             kind u8 (empty/leaf/split), flags u8, payload u16 (child group base, or leaf id),
             min_y i16, max_y i16          // vertical bounds, the cull and ray early-out data
```

Node ids are `u16`, depth at most 5, children of node n at `payload .. payload+4` in Morton order. For the Cathedral (33 by 26 sectors) the root level is 9 by 7 = 63 nodes, 504 bytes [derived], small enough for the 1 KB scratchpad where the culling and ray loops can keep it hot; deeper levels stay in RAM. The per-sector index of 10.2 is what the motor uses, the tree is what rays, culling and LOD use.

Where the tree is used, with the expected effect. **All effects are [estimate] until the phase T15 measurement exists.**

| Use | How | Expected effect |
| --- | --- | --- |
| Hierarchical frustum culling | Test a node's box (bounds from `min_y/max_y` and the node rectangle) against the frustum; reject the whole subtree, accept it wholesale when fully inside | Fewer per-cell tests in big rooms with large flat regions; no effect on a view that sees most of the room. Replaces nothing in the row-span cull until measured to win. |
| Node LOD | A far node draws its own **merged representation** instead of its children | The only change that reduces visible quads in open halls; see below. |
| Ray early-outs | Line of sight, camera arm, hook traces skip an empty node, or a flat node whose bounds the ray misses vertically | Fewer cell steps per ray; the walk already skips cells ("cell-skipping ray walk", `grid-perf` commit d5081561). Gain unknown, reported per route. |
| Memory compression | A flat open area is one leaf record (20 B) instead of one per sector | See 10.5. |
| Streaming and activation | A level-0 block is a natural streaming unit and an activation grain finer than a room (`level-format-and-runtime.md` section 6.1 admits the Cathedral is one activation unit) | Lets a room larger than the pool stream by blocks; design only, no format change needed beyond a block directory. |

**Node LOD must do better than the flat LOD measured earlier**, which showed checker re-registration across 10 to 17 percent of the screen, one-frame pops up to 12 percent, 1 px seams, and only about 16 percent saving in the worst nave view (714k to 597k) [measured, supplied]. The design answers each:

1. **Re-registration**: the merged representation is built from the same texel map as the children (the exact-UV rule of 9.2, rebased once, no clamp at 63, texture window repeat so a multi-tile merge keeps the phase). The earlier clamp and rebase are the known source of phase shifts.
2. **Pops**: a node offers a merged representation only if it is equivalent to its children in plane (exact, in ru), material, UV map and baked light (`LIGHT_TOL`); a node that fails the test has no LOD and never pops. The affine warp of the larger quad is bounded by selecting the merged form only when the node's depth range is small against its distance (the same ratio as 9.5), so the switch changes sub-texel positions, not the pattern. A hysteresis band prevents flicker.
3. **Seams**: each node carries precomputed **edge stitch patches** for the states where a neighbour is finer: a merged node whose neighbour across edge e is drawn at the finer level draws a small fan including that neighbour's boundary vertex, selected by a 4 bit mask computed from the neighbours' states in the same frame. With 2:1 balance of the *selected* levels (the selection is made monotone in distance) there is only the midpoint vertex per edge. The patches are exact geometry, not skirts, so there is no overdraw and no gap.
4. **Saving**: the node form only helps if merged nodes are large, so selection prefers whole subtrees (a 4-sector node replacing 16 sector quads and up to 4 patch fans). The expected saving is therefore larger than the flat 2x2 scheme in open nave views, but this is an [estimate]; the phase T15 acceptance is a **measured** pose-matched comparison, not this number.

### 10.5 Size and RAM estimates

All **[derived]** from record sizes unless stated; nothing here is a measurement of a cooked level except the supplied Cathedral figure.

| Item | Formula | Example |
| --- | --- | --- |
| Dense collision sectors (today) | 20 B x sectors | Cathedral 858 sectors = 17,160 B (the whole collision block is 22,748 B [measured, supplied]) |
| Leaf collision | 20 B x leaves + 2 B x sectors + 32 B x sub-blocks | A 128 x 128 flat room: dense 327,680 B; as 1,024 level-0 leaves 20,480 + 32,768 = 53,248 B |
| Sub-sector detail | each subdivided sector adds 32 B of block table plus its leaves at 20 B | A 10 x 10 sector sculpted hill with all 100 sectors at level 4: 100 x (32 + 16 x 20) = 35,200 B |
| Nodes (optimise) | 8 B x nodes | root level of the Cathedral: 504 B |
| Quads | 40 B each (`P/format.rs:451`) | merging is the lever, measured in T14 |
| Hanging-vertex fans | 1 triangle per hanging vertex, at most 4 per coarse leaf | counted by the cook report |

RAM is nearly full on the console (`level-format-and-runtime.md` section 7.4); `MAX_SUB_BLOCKS_PER_ROOM` and the leaf count are shown in the status bar next to the face budget, and the cook reports bytes per section with each cook as it does today (`E/cook/mod.rs` summary).

---

## 11. Overlays

### 11.1 Walkability (Alt+W)

Computed in the editor from the document using the **runtime's own rules and numbers**, so the overlay agrees with the motor:

* Heights are rounded to ru through `RuntimeUnits::rt` first, as the cook does, so what the overlay says is what the motor sees.
* **Too steep**: a floor triangle whose normal y is below 2867/4096 (`character_motor.rs:38`, computed with `psx_grid::height::triangle_normal_q12`, the function the cook uses at `E/cook/collision.rs:88`). Tinted orange.
* **Step too tall**: for each pair of walkable floors that share an edge, if the floor profiles differ by more than the step-up height at either end. The edge segment is drawn red. The step-up is the constant 40 ru today and the config field soon (`character_motor.rs:27`); the overlay reads one shared source and its legend prints the value in use ("step 40 ru = 2 clicks").
* **Low headroom**: ceiling minus floor at the four corners and the centre of the leaf is less than the body height (48 ru today, `character_motor.rs:20`). Tinted blue.
* **Drop**: a walkable edge whose drop exceeds the step-down (40 ru) is dashed yellow: the body falls there. Informational.
* **Reachability** (toggle): flood fill from the spawn nodes over walkable edges that pass the step and headroom tests; walkable leaves not reached are hatched, which catches rooms that were built and never connected.

### 11.2 Performance heatmap (Alt+P)

A **model**, labelled as such in the legend, built from the supplied measurements:

```
cost(spot, yaw) = Q * 800 + N * 10,000          cycles
   Q = quads facing the camera inside the frustum (cull + backface only; projection not run)
   N = quads with a corner behind the near plane or nearer than 170 depth units (the near split path)
heat = cost / (1,128,960 - reserved)             reserved default = player 470k + sky 58k [measured, level doc section 1]
```

800 cycles per visible quad and 9 to 11k per near tile are the measured values (10,000 is the midpoint); the 470k and 58k are measured on the Cathedral and the field is editable because the character cost changes. Samples: every walkable leaf centre at the camera eye height, 8 yaws, the maximum over yaws (the analyzer convention used for the bench maps, `docs/grid/other-maps-survey.md` section 1). Colour: green below 60 percent of the world budget, amber to 90 percent, red above [proposal]. It runs on the **cooked** tables (so merge, stitch and UV splits are included) through the same row-span cull and facing test the renderer uses (`psx_grid::render` select functions on the host), without the GTE projection.

Honesty rule for this feature: the model is checked against the headless emulator's own counters (`GridFrame.quads_visible`, `robust_quads`, world cycles) on the Cathedral and Dungeon route tapes in phase T7, the ratio is reported in the phase notes, and the two constants are recalibrated from it. The legend shows the build of the calibration.

### 11.3 Refresh

* Both overlays keep an `AnalysisCache { rev, per_room }` keyed by the document revision (`GridEditState.rev`, `U/mod.rs`). `normalize` returns the dirty leaf set; walkability recomputes only dirty leaves plus a one-leaf ring on the UI thread (a local, small job); a config change recomputes everything.
* The heatmap runs the cook on a worker thread, debounced after the last edit (500 ms [proposal]), coarse while a gesture is in progress (every second leaf, 4 yaws) and complete when idle, restricted to samples within the room's far distance of the dirty region. Stale tint is dimmed until the new result for the current revision arrives; a result for an older revision is dropped. The cook time on the Cathedral has not been measured for this use and is reported in phase T7.
* The status bar budget readout (`U/strip.rs:177`) gains the cooked quad count and the hottest sample ("worst spot 71 percent") when the heatmap has a result.

---

## 12. Coordination with free planes

Planes (`grid/planes`) and this work touch the same files. Order: **planes merges first**, then T1 starts from the merged tree.

| File | Planes change | This design adds |
| --- | --- | --- |
| `E/room.rs` | `planes` field, `trim_empty` counts plane footprints | `leaves`, `wall_mode`, `defaults`, `band_sets`, `edge_overrides`, `openings`; `trim_empty` must keep counting planes when it works on leaves (T8) |
| `E/faces.rs` | `GridFaceKind::Plane`, `unlit`, `Corner`/`Builder` made `pub(super)` | derived walls, bands, stitching, UV frames (shares `Builder`) |
| `E/limits.rs` | plane caps and issues | new caps and issues (3.7) |
| `E/world.rs` | plane bounds, `material_ids` | leaf lookups in `floor_height_at`, material ids from bands |
| `E/cook/render.rs` | `RoomBuild::add_face` refactor, plane refs | the UV splitter output, `flags2`, merge and repair (T3, T14) |
| `E/cook/collision.rs`, `E/cook/mod.rs` | plane collision, `CHUNK_HAS_PLANES` | leaf records, `SECTOR_INDEX`, closure walls |
| `P/format.rs`, `P/view.rs`, `P/render/select.rs` | VERSION 2, `PlaneRef` run | v3 and v4 |

The plane agent's choices that this design depends on and does not change: `PlaneRef` is a run of quads (`grid-runtime:P/format.rs`), `ColPlane` kinds, `MAX_PLANES_PER_ROOM = 64`, planes lie inside the room footprint (`PlaneOutsideRoom`). If the planes UI picks one of the verb letters (7.12) the verb moves to its Shift chord.

---

## 13. Phases

The order follows Manny's approved sequence (derived walls and auto split first, then Raise/Lower, Ramp and Stairs on the fixed grid, then the quadtree with Sculpt and stitching, then Room, Corridor, Opening and Stamps, overlays early) and his latest instruction: **make it happen first, optimise after.** Every phase is sized for one agent run. **Acceptance is correctness** (levels load and look identical, tools work, Play works with collision, no cracks). **Performance is measured and reported in each phase's notes, never a gate.** Measurements use the headless emulator on the Cathedral and Dungeon route tapes (cycles for world, collision and camera) and the cook size report.

Every phase also: leaves `make lint` clean, adds the tests of section 14 for its area, and shows frames (preview and a Play capture) before it is called done, because a visual change that was never looked at is not done.

| # | Phase | Delivers | Acceptance (correctness) | Measure and report |
| --- | --- | --- | --- | --- |
| T1 | **Derived walls** | `wall_mode`, `derive.rs` (R1 to R3), `wall_material`, `EdgeOverride` (hidden, material, uv, solid), `faces.rs` integration (ownership, per-sector clipping, culling bounds), closure `ColWall`s, validate warnings, `convert_to_derived`, Wall tool gap awareness, Rooms panel toggle and Convert button, starter room made Derived | Six reference levels and the starter load; `all_faces` unchanged for `Authored`; Convert (PreserveLook) leaves the face multiset unchanged; cook sections unchanged for legacy; derived platform, pit, bow tie, closure tests pass; Play walks a derived starter and cannot leave it | face counts before and after Convert; cook byte sizes |
| T2 | **UV model** | `FaceUv` extension (`UvMode`, turns, flip), `uv.rs` (frames, report), Inspector Tiling panel, material `grid_uv` default, cook UV splitter (limit 1 tile), planes adopt the shared frame | Legacy texels byte identical; FIT 1x1/2x2/4x4 and WORLD give the expected frames in the preview and the cook; report text correct, including split warnings; planes Tile/Stretch unchanged | quad count change per reference level (expected none) |
| T3 | **Window repeat** | PGL v3 (`tex_shape`, `flags2.REPEAT`), `GridMaterial.window_word`, fast and robust path, cook limit raised to 4 (3 for non-zero phase), hardware-test step for E2 cost | Pose-matched headless frames: a FIT 4x4 face equals the same face as 16 one-tile quads within texel rounding; v2 chunks refused; golden v3 fixture | cycles per packet with and without `REPEAT`; the E2 change experiment |
| T4 | **Bands** | `BandSet`, `Anchor`, resolution, polygon pipeline (6.3), neighbour stitching on vertical edges, Bands verb (E), Inspector Wall run context, cook UV frames | Band geometry tests (slope, Abs break, stack, hole), no T gaps on a banded wall with differing neighbours, palette/Paint targets bands; legacy rooms without bands byte identical | quads per band, cook size |
| T5 | **Auto split, Region, Raise/Lower** | `auto_split`, fit rule, `Region` front end (marquee, brush, path), Raise/Lower with Offset/Level and ceiling follow, smart paint rules and `paint_locked`, Inspector Region context, strip groups | Auto split deterministic and stable on ties; undo/redo/cancel exact; derived sides appear live; legacy splits untouched | gesture frame time in the editor |
| T6 | **Ramp, walkability** | Ramp verb, walkability overlay with reachability, motor step/height read from one shared source | Slope class and step tests match `character_motor` constants; a ramp built at the steepest walkable slope is walkable in a headless Play walk; unreachable hatching correct on a fixture | overlay recompute time per edit |
| T7 | **Stairs v1, heatmap** | Stairs verb (whole-sector treads, ramp option), performance heatmap (model, cache, refresh) | A built flight is climbable by the real motor in a headless walk; heatmap renders on all reference levels and its ratio to measured counters is reported | heatmap model vs `GridFrame` counters on the route tapes; cook time for the heatmap |
| T8 | **Leaf storage** (built, section 22) | `LeafMap`, `LeafKey`, serialisation negotiation, `FaceRef` on leaf keys, accessors, port of tools and tests; no behaviour change | Every existing test passes unmodified through the adapter; reference projects round trip as documents; dense disk form preserved | none expected; report editor frame time |
| T9 | **Quadtree operations** | subdivide, merge, balance, hanging corners, `normalize`, planar-only big leaves, per-sector clipping, **document stitching** in `faces.rs`, validate, Subdivide/Merge commands, leaf grid overlay | Property tests (14) hold; stitched meshes watertight in the preview (edge adjacency test); derived walls correct across leaf sizes | face and triangle counts of stitched fixtures |
| T10 | **Leaf cook, format v4, collision** | `SECTORS` per leaf, `SECTOR_INDEX`, `SUBBLOCKS`, closure walls on leaves, `origin_mod`, `CHUNK_TREE`, reader and `GridTraceProvider` changes, preview of leaves, Play | Leaf lookup equals the dense reference at every point of the reference levels (after Simplify) and of random trees; uniform rooms cook to dense sections unchanged; a headless Play walk across sub-leaves and big leaves behaves as the dense reference | collision cycles per tick and camera trace cost on Cathedral and Dungeon route tapes vs the uniform grid; collision block bytes |
| T11 | **Sculpt, fine Stairs** | Sculpt with auto subdivide and exact merge, Stairs subdividing along the run | Round trip: sculpt then undo restores the document; exactly representable results merge back; stairs of 1/2 and 1/4 sector treads climbable by the motor | leaf counts and bytes after a reference sculpt |
| T12 | **Room, Corridor** | Room and Corridor verbs, maximal-leaf decomposition, junction behaviour | A room and a corridor built in an empty project are closed by derivation, walkable, and portal-connected where openings exist | cook report |
| T13 | **Opening, Stamps** | Opening verb (derived and authored), Stamps (save, place, rotate, flip, split to fit) | Door in a banded derived wall keeps bands through the lintel; a stamp rotated four times returns the original; portal derivation sees openings | cook report |
| T14 | **Optimise A** | cook merge (9.2) with `CELL_INDIRECT`/`MULTI`, near cap (9.5), generic repair (9.3b) | Merged output pose-matched to unmerged within the light tolerance; no new gaps (edge adjacency test); legacy cook with merge off byte identical | quads before and after on each reference level; world cycles on the route tapes |
| T15 | **Optimise B** | runtime tree arrays (10.4), hierarchical culling, node LOD with edge stitch patches, ray early-outs | Pose-matched frames across LOD switches show no re-registration, no seam pixels and no pops beyond a threshold set from the unmerged-vs-unmerged noise floor; tree lookups equal the sector index | world savings on the open-nave views, collision and camera trace cost vs the uniform grid, node LOD saving vs the old flat LOD (714k to 597k) |
| T16 | **Streaming units** | root blocks as chunk and activation units | design-level; its own spec once T15 lands | |

T14 and T15 are the "optimise" block Manny asked for; if T14 or T15 disappoints, T1 to T13 stand on their own: the cook expands big leaves to sectors (9.1), so the game renders exactly as it does today.

---

## 14. Tests

Unit and property tests live next to the code, the headless gates drive the real editor and emulator.

* **Derivation** (T1): ownership and facing for R1 to R3 on hand-built worlds; bow tie cut at the crossing; equal profiles give nothing; `Convert` multiset identity on the six levels; `AuthoredDuplicatesDerived` fires; closure `ColWall`s match the face geometry.
* **UV** (T2, T3): `frame_texels` for floor, ramp, tri-split, wall, run and band frames under both modes with turns and flips; `report` against hand-counted fixtures; splitter output covers the face exactly (area sum) with shared edges; the renderer test of T3.
* **Bands** (T4): property test over random bottom/top profiles and lines: the band polygons tile the wall without overlap or gap (area sum equals face area); anchors follow slopes; openings cut through all bands.
* **Auto split** (T5): the fit error is minimal over both diagonals for random corners; ties keep; planar surfaces untouched.
* **Verb drag contract** (T5 on): begin, apply(a), apply(b), apply(0) restores the document byte for byte; cancel restores; undo restores.
* **Quadtree** (T9): after any random sequence of subdivide, merge and set-height operations: leaves are disjoint and aligned, balance holds, constrained corners equal the average, big leaves are planar, serialisation round trips, `leaf_at` equals a brute-force scan.
* **Stitching** (T9, T14): an edge adjacency test over the final faces: every pair of faces sharing an edge segment share the same vertices along it (no vertex of one lies strictly inside an edge of the other).
* **Collision** (T10): leaf lookup equals the dense reference at every point; random trees vs a brute-force height query; the existing motor and camera tests run against the leaf provider; a stair climb and a ramp climb with the real motor in a headless Play.
* **Format** (T3, T10): golden bytes per version, malformed-input suite (truncated, misaligned, bad index, bad `SECTOR_INDEX` id), layout hash.
* **Overlays** (T6, T7): fixtures with known too-steep, too-tall and low-headroom cells; the heatmap model's ratio against emulator counters is a report, not an assertion.
* **Editor harness**: tool gestures in the style of the existing headless editor interaction harness (`ViewportHarness`), one per verb: press, drag, release, assert document, undo, assert restored.
* **Looks identical**: frame census of the preview rasteriser before and after each phase on the six levels, compared pixel for pixel while the phase claims no visual change, and shown to Manny when it does.

---

## 15. Risks

1. **Balance ripple and fan cost.** Detail next to a big flat region forces transition leaves and stitch fans. Mitigation: they are flat and merge in the cook (T14); the heatmap and the cook report show them; the leaf level range can be reduced without a format change.
2. **Texture window cost.** Repeating via the window is cheap in packet size but the GPU cost of window changes is a hypothesis (9.6). The experiment is in T3; the fallback (1 tile per primitive) is the T2 behaviour.
3. **FIT frame semantics.** A run is the frame, so lengthening a wall changes its tile size. That is intended for FIT; WORLD never does it. The Inspector says which applies.
4. **Walkability on fine treads.** The motor samples five points; treads narrower than the body radius may be stepped over. T7 and T11 carry a headless stair test; the verb warns below the radius.
5. **Convert coincidence.** Matching authored walls to derived faces uses a one-fine-step tolerance; a wrong match would drop a wall. PreserveLook plus the multiset-identity test is the guard; Convert is undoable.
6. **Undo cost.** Undo snapshots the whole document (`U/mod.rs:212`); with leaves and overrides the document grows. Not a regression, but a smell worth a separate fix (a structural undo log).
7. **Merge conflicts with planes** (section 12). Mitigated by ordering.
8. **RAM growth from sub-leaves.** 35 KB for a fully fine 10 by 10 hill (10.5) is comparable to the whole Cathedral collision block. Cap, readout, and the Simplify command.
9. **Cross-room derivation** is not done: a doorway between side-by-side rooms is two Openings.
10. **u8 UV far-edge clamp** costs a 0.4 percent shift on a 4x primitive; tolerated, reported.

### Smells noticed (separate issues, not fixed here)

* `GridRoom::sectors` plus `sector()` is addressed by local `(x, z)` in about every edit function; leaf keys fix it as a side effect of T8.
* `FaceRef.cell: [i32; 2]` mixes world sectors with local indices in a few helpers.
* `EditorWorkspace` still mirrors `grid.tool` into the brush `ViewTool` (`U/mod.rs:383-387`, noted in the UX audit section 5).
* `limits::MAX_WORLD_FACES` is a heuristic bound that duplicates what the cook now checks exactly.
* `place()` in `faces.rs` and the plane code carry parallel UV logic (resolved by 5.7).
* Key hints are written three times (`U/help.rs`, tooltips, `strip.rs:119`).

---

## 16. Open questions for Manny (each with my recommendation)

1. **Verb letters.** U Raise, S Sculpt, A Ramp, T Stairs, B Room, K Corridor, I Opening, E Bands, Y Stamps, with Alt+W and Alt+P for the overlays, leaving N G J M Q X Z free for the planes tool. Recommend: approve; if planes already took one of them, the verb moves to its Shift chord.
2. **Legacy rooms.** Keep them `Authored` until you press Convert (look-preserving by default), rather than converting on load. Recommend: yes, never change a level behind your back.
3. **Leaf alignment.** World-anchored 4-sector blocks (new rooms snap their origin to a multiple of 4 sectors) so growing a room never moves a leaf. Recommend: yes.
4. **2:1 balance.** Enforce it (transition leaves appear automatically and merge away), so a coarse edge has at most one hanging vertex. The alternative is unlimited level jumps with longer fans. Recommend: enforce.
5. **FIT frame for walls = the run, not the single cell edge.** "8x1" then means eight repeats along the whole run. Recommend: run.
6. **Cross-room derived walls.** Not in v1; doors between adjacent rooms are Openings. Recommend: defer, revisit with stacked rooms.
7. **Heatmap reserve.** Default reserved cost 470k (player) plus 58k (sky), editable, because the character cost is still the biggest single item. Recommend: yes.
8. **Per-material tiling default** stored on the Material resource (so the same brick tiles the same everywhere unless a face overrides). Recommend: yes.

---

## 17. As built: T3 and T4 (2026-10-17)

What landed, and where it differs from the text above.

**T3, texture-window repeat.**

* PGL v3 as in 10.1 (`tex_shape`, `flags2.REPEAT`); a v2 level or chunk is refused with a message that says to cook again. The cook sets `REPEAT` when a primitive spans more than one 64 texel tile and the material has a shape; a primitive holds up to four tiles per axis, the first piece of an axis ends at the fourth tile line counted from the tile its lowest texel is in (so three tiles and a bit when the texture starts mid-tile), the next pieces are four tiles each (the fewest pieces the limit allows, not equal ones).
* **Legacy splitting is on** (decision of the coordinator): faces on the old 32 texels per sector mapping are cut like any other. `uv_split::SPLIT_LEGACY` is gone. With the window the reference levels need no extra primitives: Cathedral, level008, Dungeon and Cave have 18, 48, 8 and 18 repeating primitives and 0 cuts (one tile per primitive cost +38, +81, +8 and +18).
* The fast packet needed no growth: slot 1 carries the material's window word for a `REPEAT` quad and the reset for any other. The near-plane path takes the windowed packet shape of `affine_surface` (window in front, reset behind, one word more). The merged LOD quads exclude `REPEAT`.
* The E2 cost experiment is six GPU cases in the hardware-tests suite (`0x140` to `0x145`, "grid-line E2 probe, pending port to the consolidated suite", no version number of its own), built and not yet run on a console. Headless the emulator does not model GPU texture-cache effects, so no number from it is a prediction.
* **Facing shade** (new, decision of the coordinator): `GridWorld.shading` (`FacingShade`: floors, ceilings, east-west and north-south walls, Q8 factors, blended by the squares of the normal) applied in `light.rs` after the lit colour, one function for the preview and the cook. New worlds get `SUBTLE`, a world without the field is `OFF` (a level never changes behind its author's back). Rooms panel row with Off, Subtle, Strong and four numbers.
* **Global light** (`GridWorld.global_light`, `grid/lighting/`): one bake for the cook and the preview. `BakeRig` colours a corner from the lamps (shadowed by the runtime's own line of sight over collision tables built from the document), the facing shade and an ambient. With `global_light.enabled` the ambient is gathered: `gather.rs` sends cosine-weighted rays (Hammersley pattern turned by a hash of the corner, so a cook is reproducible) from every face corner through `caster.rs` (a 3D DDA over the drawn faces that returns hit point, face, normal and material). A blocked ray within `reach` sectors removes `occlusion` percent of the room ambient and brings in the hit face's reflectance (mean texel colour times tint, `albedo.rs`) times what that face received in the pass before; an open ray brings the room ambient; a ray that leaves the world brings `sky` (default the room ambient). The sum goes through a soft shoulder (linear to 180, bending to 255) instead of a hard clip. The cook turns a quad's corner order when the other diagonal fits the light at its middle better, only for parallelograms in position and texels so the texture does not move. The editor solves the gather on a worker thread and rebakes the faces when it lands; direct light and shadows rebake at once. A world saved without the field loads with it off, so a level never changes behind its author's back; new projects start with it on.
* **Click cycling** (decision of the coordinator): in the Top view a repeated click at the same spot takes plane, then floor, then ceiling; Alt is the fine snap only. `GridPointer.cycle` carries the step; a drag, another spot or a tool change starts over.

**T4, bands.**

* `BandSet`, `Anchor`, `BandV` as in 6.1; `GridRoom.band_sets`, `RoomDefaults.bands`, `EdgeOverride.band_set`, `GridWall.band_set` (index, `255` is none), all serde-defaulted. `limits::validate` warns on a malformed set or an index that names nothing (the wall draws plain); caps 6 lines and 32 sets.
* The polygon pipeline is `band_geom.rs`: every rail is linear in the position, lines are clamped between the previous line and the top and then into the wall, rails that cross inside the span cut it there (positions rounded to whole units), so a band is a quad or a triangle and the pieces tile the wall exactly (a property test over random walls). Where the design allows six corners for a band across a break, the pieces are cut at the break instead, so no band polygon passes four corners. Neighbour stitching (`stitch_edge`, `fan`) inserts the heights a collinear neighbour cuts a shared vertical edge at and fans the face into triangles; it applies to plain walls next to a banded one too, and does nothing in a room without bands.
* **Courses** are kept only for authored rooms with no band sets (the legacy golden hash of every face of the six reference levels and the planes demo is unchanged). Derived rooms and rooms with bands draw a wall as one face. Texels and outlines are the same (a test joins the courses of both sides and compares), but a tall wall near a point light is now lit from its top and bottom corners only, so its baked light differs from the course-cut version (measured on a lit derived room: 53 fewer primitives of 176, and 36 percent of the frame's pixels differ, all on the walls near the lamp). The design's cook merge has a light tolerance for exactly this and does not exist yet.
* **Frames**: a banded face keeps the run frame for a fit mapping across (derived) or the record (authored); a fit band measures its height between its own two lines at each position; a world band can seat V at the world, the band's lower line or its upper line (`BandV`).
* **Bands verb (E)** is a tool of the strip, not a Gesture of a region; scope is a toggle (this run copies a shared set first, the whole room edits in place). The wall-run Inspector context lists lines and bands top to bottom with material, V and tiling per band. The Paint tool and a Shift+click on a band target bands. The Rooms panel lists the sets and picks the room default.
* Not built: openings through bands (T13), the corner stitching of perpendicular walls (T14 generic repair), the Rooms panel band set list as a manager (rename, delete).
* **Item for T14 (optimise): light-adaptive wall split.** One-face walls (T4) are lit from their corners only, so a tall derived wall near a point light loses the gradient the old courses carried (36 percent of pixels differed on a lit derived room). Coordinator decision: keep one face per wall (fewer primitives) and, in the optimise phase, cut a wall only where the baked light gradient along it is large (a light-adaptive split, the same tolerance idea as the cook merge `LIGHT_TOL`). No code change before then.

## 18. As built: T5 (2026-10-17)

**Auto split: the design's rule cannot decide, so it was replaced.** Section 4.6 chose the diagonal that fits the bilinear surface of the four corners best over the quarter lattice. For every input the two diagonals have exactly the same error (the triangulated surface differs from the bilinear one by the same amount along either fold), so the rule always ties and, with "a tie keeps the split", never changes anything. `auto_split.rs` keeps a test that checks this over 3000 random cells. The shipped rule: **the diagonal whose two end corners are closest in height wins; a tie keeps the current split.** The fold then runs along a contour and the odd corner pops out of it (a raised corner leaves the level diagonal as the crease). It is integer, deterministic, independent of drawing order, and still never flips a flat, planar or symmetric cell. `GridSurface.auto_split` is `false` for every surface made before T5 (the six reference levels load with all diagonals locked; a test asserts it), `true` for surfaces the Floor and Ceiling tools and the verbs make, and the Split tool turns it off on the surface it flips. Planar and split-level surfaces are never re-split. A surface chooses again only when its heights actually changed, so an amount of zero returns the document exactly; `HeightDrag` restores the start diagonal before every choice for the same reason.

**Region** (`edit/region.rs`) is a set of world sectors of one room, editor state, never saved. Sources: marquee (a rectangle, or a click for one sector), brush (a disc of sectors, radius 0 to 6) and path (waypoints joined by a four-connected walk, widened sideways, Enter finishes). Shift adds. It survives a verb and a tool change until Esc; switching to a verb adopts the Select tool's sectors. Verbs are `begin` (snapshot of the touched sectors and the room's band sets and defaults) and `apply(amount)` (restore, recompute from absolutes); the drag contract (apply a, apply b, apply 0 equals the original document) is tested byte for byte. The verb never grows the room.

**Raise and Lower** follow section 7.3 with these specifics. Level's reference is the middle of the floor of the sector under the pointer at press (corner mean snapped to 64). "Follow ceiling" moves each ceiling corner by the delta of the floor corner under it, so Level mode keeps each corner's headroom. Without follow, a floor corner that would pass its ceiling is held one click under it and counted. `wall_material` is the palette's current material when one is chosen (the palette choice is sticky, so after the first pick every raise stamps it), else the room's rule, else unset. The amount snaps to a click, Alt to 64 and Shift+Alt also to 64 (heights have no off-lattice state). Drag amount rate is the Height tool's (18 px a click in Top, the grab point's projection in 3D). Cost of a gesture frame is under "Measurements".

**Smart paint rules and `paint_locked`.** `RoomDefaults.rules: PaintRules { floor_by_class [flat, slope, steep], trim, wall }` (Rooms panel, Paint rules). Slope class is `slope.rs`: heights go through the cook's grid to runtime rounding and `triangle_normal_q12`, compared with the shared motor limit. A rule paints a floor only when its class changed under the verb and `paint_locked` is false; `set_material` on a surface (the Paint tool, the palette, the Inspector) sets `paint_locked`. The `trim` rule makes one default band set ("Trim": a foot band of the rule's material below one click, the rest the wall) when a verb moves something in a room with no default set; the snapshot includes the room's sets, so a drag back to zero removes it.

**One source for the motor's walking numbers.** `psx-engine/src/walk_limits.rs` holds `WalkLimits { step_up 40, step_down 40, body_height 48, min_floor_normal_y_q12 2867 }`. The motor's constants, the cook's walkable-triangle bit, the plane platform limit, the slope classes and the verbs all read `WalkLimits::DEFAULT`; the step-up becoming a config later is one change there. (`psxed-mcp` keeps its own copy of 40 for its guidance text; it does not depend on the engine crate.)

**Keys.** U is the Raise tool. The reserved list is now S B K I Y; A and T are reserved until Ramp and Stairs land. Region keys: Tab (Offset and Level), Enter (finish a path), `[` and `]` (brush radius, path width), Esc (cancel the drag, then the region). The strip is grouped PICK, DRAW, BUILD.

**Measurements** (release build, the Cathedral, a region of all 676 sectors with a floor, follow ceiling on, 200 applications): one `apply` 0.04 ms, `all_faces` of the result (1662 faces) 0.26 ms, a whole Top view editor frame with the region shown 0.13 ms. The 3D preview itself (the emulator's rasteriser) is not in the rig and was not measured here.

## 19. As built: T6 (2026-10-17)

**Ramp** (`edit/verbs/ramp.rs`) follows 7.5 with these specifics. The A edge is a side of the region's bounding box (nearest the press, or held by R); its height is the mean of the floor corners on it, snapped to 64. Every corner of every region floor is then set to `A + rise * k / depth` (k the corner's lattice distance from A, depth the box depth along A to B), rounded to 64, so a non-rectangular region slopes by position. A split-level floor becomes one plane; ceilings follow by the corner delta or are held a click under; the diagonal is the auto-split one. `RampDrag::max_walkable_rise` scans up in 64 unit steps until a floor triangle would be steeper than the motor stands on (the cook's `triangle_normal_q12` against `WalkLimits`), so the clamp is exact at the fine step, including the rounding of each row. Not built: the ramp option of Stairs (T7), a skew between non-opposite sides.

**Walkability** (`walk.rs`, `walk_overlay.rs`, `analysis.rs`): `analyse(world, spawns, limits)` returns per sector `steep`, `low_headroom`, `ghost`, `unreachable` and per edge `StepTooTall` or `Drop`. Heights go through `RuntimeUnits::rt` (the cook's rounding); steep is `triangle_normal_q12` under `WalkLimits.min_floor_normal_y_q12`; low headroom is ceiling minus floor under `body_height` at the four corners or the middle; a step is flagged when two walkable floors differ by more than `step_up` at either end of the shared edge (strictly more: a step of exactly the limit climbs, a test walks the real motor over 40 and 45 runtime unit risers and agrees with the flag). Reachability is a directed flood from the spawn sector: the body steps up at most `step_up` and falls any distance, a solid wall record or a blocking derived wall stops it, and rooms join across adjacent sectors. Differences from the design's 11.1: low headroom is judged on the floor and ceiling corners rather than a leaf's five samples (a sector is one leaf here); drops are only listed from the walkable side; edges between rooms are not flagged. The analysis is cached against the document revision (`GridEditState.rev`) and recomputed on the drawing thread; the legend shows the last run time.

**Tests.** Slope class and step tests against the shared constants (`slope.rs`, `walk.rs`), a ramp at the steepest walkable rise climbed by the real motor and one fine step steeper not (`cook/ramp_tests.rs`, the cooked `GridTraceProvider` the guest uses), the analysis and the motor agreeing on a 40 and a 45 unit step, unreachable hatching on fixtures, and headless gestures for Ramp and the overlay in both views.

## 20. As built: T7 (2026-10-17)

**Stairs v1** (`edit/verbs/stairs.rs`) follows 7.6 for whole-sector treads. The path is an ordered, four-connected walk of waypoints widened sideways; each sector is a tread, a sector where the path turns is a landing (flat, the previous height), and the first tread stands one riser above the floor at the start of the path. Riser option: one click, `Max` (the tallest 64-multiple of grid units whose runtime height is within `WalkLimits::step_up`: 512 units, 40 ru, at the default scale) or typed, never above that. Steps are `round(rise / riser)` clamped to the treads the path holds, the riser is `rise / steps` snapped to 64, the rest of the path is a flat landing at the top; a flight that would need a riser over the limit is refused with nothing written (a test checks the document is unchanged). The ramp option slopes each tread from its entry edge to its exit edge. Floors are made inside the room where the path crosses holes; the room never grows. Not built: the tread-narrower-than-radius warning (whole sectors are far wider than the body), finer treads (T11). The **one source of truth for step height** is `WalkLimits` in `psx-engine` (section 18); the editor reads it, the motor's constants are defined from it.

**Heatmap** (`heat.rs`, `heat_overlay.rs`, `analysis.rs`): a host model on the cooked render tables, as in 11.2 with these changes forced by measurement. The cell cull tests the four plan corners of a cell against the near, left and right planes (the renderer's row spans are the exact form of the same test); the facing test is the renderer's own (Q7 normal and plane term, axis planes, double-sided); far blocks stand in as the cook made them. Two differences from the design text, both from fitting the renderer's counters: (1) the **near term counts quads with a corner behind the near plane** (8 ru), not "nearer than 170": 170 is the depth under which the near path splits a tile once, it is not what sends a quad down that path, and counting everything under 170 made the model 3 times too hot (28 near quads at the Cathedral spawn against about 2 measured); (2) **the constants are fitted**: on the Cathedral route tape (this tree's guest, 497 rendered frames of the last 1100 simulation ticks) the profile's world cycles are `741 x visible quads + 3123 x split triangles + 7.9k` (R2 0.996), so `per_quad` is 741 and a near quad, about three split triangles, is 9,400 (the supplied 9 to 11 thousand per tile agrees). The camera is the spot lifted by the world camera's height and pulled back by its distance (125 ru) while the sector has a floor; pitch, wall records and portal views are not modelled. Samples are every standable sector, 8 yaws, worst yaw. Colours: green under 60 percent of the world budget (1,128,960 minus the 528,000 reserve), amber to 90, red above. The result is cached per revision and (reserve); recomputed on a worker thread 500 ms after the last edit (a coarse pass, every second sector and 4 yaws, if a gesture is in flight and nothing is cached); the old tint stays, dimmed, until the new one arrives.

**Model against the renderer's counters.** Measured on the Cathedral route tape (`route.pxtape`, 2300 polls, last 1100 ticks, 497 frames): visible quads p50 99, p90 467, max 1442; world (room) cycles p50 108k, p90 374k, max 1.02M. At the spawn, where the player stands still for the first 150 rendered frames: 508 visible quads, 7 split triangles, 409,710 world cycles. The model at the spawn sector over its 8 yaws: 252 to 336 quads, 1 near quad; at the worst yaw 258k cycles. So at a matched pose the model gives **0.66 of the measured quads (max yaw) and 0.63 of the measured cycles**: it under-predicts a frame the real pitched third-person camera sees more of (it looks down at the player, the model looks level). Over the whole level (676 spots, worst yaw) it gives quads p50 519, p90 666, max 712 and modelled cycles p50 416k, p90 513k, max 566k (94 percent of the world budget): a spot-and-worst-yaw census is not a route, and the route's own worst frame (1442 quads, 1.02M cycles) is **1.8 times the model's worst spot**. The heatmap therefore ranks spots well and is a lower bound on the worst frame of an open hall; do not read a green spot as a promise. The other levels (model, worst spot as a percent of the world budget): level008 44, Dungeon 36, Cave 33, Sewers 32, West 18.

**Measurements** (release, the reference levels): walkability recompute 0.37 ms on the Cathedral (0.02 to 0.13 ms on the others); heatmap cook 2 ms and sampling 23 ms on the Cathedral (676 spots, 8 yaws), 0 to 5 ms elsewhere; both are far under the 500 ms debounce, so the worker thread is headroom for bigger levels, not a need today. A flight or ramp gesture re-applies in well under a millisecond (section 18).

**MCP tools** (`psxed-mcp`, thin wrappers over the same functions, one commit point `Workspace::grid_edit`): `grid_raise`, `grid_ramp` (`keep` a side, `rise` or `walkable`), `grid_stairs`, `grid_set_walkable`, `auto` on `grid_set_split` (and `grid_set_heights` re-splits auto surfaces), `grid_walkability` and `grid_heatmap` (reads). `tests/grid_tools.rs` runs each once over the stdio protocol.

**Play check of the verbs (T6, T7).** Beyond the host motor tests, the demo scenarios `ramp` and `stairs` (`grid_tools_demo`) were built into discs with the current guest and walked in the headless emulator from a poll-bound tape: the player climbs the steepest walkable ramp and a six step flight to the landing (frames in the progress directory, tools-22 and tools-23). The player's yaw in a scenario is in the engine's convention (0 looks north, 90 west), and the left stick is camera relative: the first demo discs faced west and walked into the wall; yaw 270 faces the ramp.

## 21. As built: natural terrain, N1 to N4 (2026-10-17)

The audit and designs are `natural-terrain.md`; this is what landed, and where it differs. Branch `grid/natural`.

**N1, diagonal geometry.**

* `GridSurface.void: u8` (bit 0 triangle A, bit 1 triangle B). A void triangle draws nothing. `derive_diag.rs` runs the three rules along a sector's fold diagonal (the edge its split names; triangle 0 is always the front side of that edge): R1d riser between the triangles of a split-level floor (the higher owns, the lower hosts), R2d drop for ceilings, R3d closure when exactly one floor triangle is void (floor to ceiling, or `open_wall`). Across a cardinal edge a void triangle is no space, so the sector next to it closes with an ordinary R3. `DerivedWall` gained `back` (the face is seen from the back of the diagonal; its `ring` runs b to a) and `edge_heights()`. `EdgeKey.axis` gained `NwSe`, `NeSw` (a diagonal is keyed by its own sector). `physical_edge_walls`, `cell_walls`, `run_members`, `run_extent`, `duplicates_derived` and Convert know the diagonals. Convert converts a front-seen diagonal that is exactly an authored wall; one seen from the back stays authored and its derived twin is hidden (a mirrored diagonal cannot be compensated with whole-texel offsets), so Convert still preserves the face multiset (test). The Cathedral's 7 hand-made diagonal risers are exactly what the derivation produces (4 NW-SE, 3 NE-SW, 9 and 18 runtime units); of the authored walls, three NE-SW ones face the high side in the data, which Convert leaves alone.
* **Cliff walls.** A derived riser taller than `WalkLimits::step_up` (40 ru; a 40 ru riser is climbed) cooks a `ColWall` flagged `WALL_FRONT_ONLY` (cardinal, hosted by the low side) or, for a diagonal seen from the back, the new `WALL_BACK_ONLY` (4). `scan_walls` and `wall_cross` read the flags (`on_front` in `collision/world.rs`). Experiment (the audit's H1): a ray under a 200 ru plateau passes with no wall (test) and is stopped with one; a body below is stopped; a body on the ledge walks off. A split-level cell also stops rays that start in the rock (the slab test), documented in a test. Triangle risers (bow ties) get no wall record (a record is a quad). A hidden or ghost riser gets none.
* **Collision cost.** The Cathedral route tape (`route.pxtape`, 2300 polls) played on two guests that differ only in the flag test (base `scan.rs`/`ray.rs` against this tree) over the same disc: 1,366,353,307 against 1,366,353,262 bus cycles, 570,162,868 against 570,294,265 CPU ticks (+0.023 percent). The Cathedral has no derived riser, so it carries no flagged wall; the number is the price of the flag test alone. A level that has cliffs pays 12 B and one more candidate in the scan per cliff.
* Walkability: a fold step taller than the climb limit is flagged (`EdgeFlag` with the diagonal edge), a void triangle does not count for the slope class, `walk::local_problems` counts problems around some cells (what Break's mask uses).
* Not built: the Height-tool diagonal handles as 3D gizmos; derived diagonals across the quadtree (T9 and later).

**N4, organic outlines.** `edit/outline.rs`: `polygon_cells` (centres inside, even-odd), `corridor_cells`, `fill_cells`, `cut_corners` (void, or crack for plateau corners; a corner is a cell with exactly two adjacent neighbours missing and nothing diagonally beyond), `restore_voids`. Unit steps line up into one 45 degree wall (test); steps wider than one sector stay steps; concave fills are not built (chamfering convex corners alone already closes a unit staircase). MCP `grid_polygon`, `grid_chamfer`.

**N3, Break.** `edit/verbs/break_verb/` (`noise`, `plan`, `apply`): every decision is `hash(seed, op, world position)`, operations are planned from a snapshot and applied one at a time; with `keep_walkable` each is checked with `walk::local_problems` around the cells it changes and retried at half size (4 times) before it is dropped; a gap also needs a flood (`analyse`) that does not add unreachable sectors and a player spawn that proves it. Operations: plateau lip crumble (the highest corner at a vertex drops by up to the step to the next level), edge bites (a crack on the half of a cell on the low side), interior vertex tilts, cracks, open wall top (`EdgeOverride.top_delta`, vertex based so neighbouring segments agree), stubs, gaps (from intensity 70), ceiling vertex moves and cracks. Rates at intensity 100 are in `plan.rs`; at intensity 40 about 16 percent of a flat level's vertices move and 6 percent of tiles crack, against the Cathedral's 1 percent, so 10 to 20 is its sparse look and the default is 15 (`DEFAULT_INTENSITY`; the `cathedral` style and the editor's Cathedral style button set that look). Holes (`hidden` segments) and rubble are not built. The Cathedral fixture (`cook/natural_tests.rs`) holds the output to the hand-made vocabulary: no diagonal riser above the climb limit, no new unreachable, steep or too-tall step, same seed same level. `Snapshot` now also captures the room's edge overrides. MCP `grid_break` takes a checkpoint (`before-break`), `reroll` restores it and uses the next seed, `dry_run` reports; the workspace remembers only the last seed.

**N2, Rough.** `grid/rough.rs`: `GridRoom.rough {seed, amp, tint}`, `GridSector.rough` (0 to 3). The offset at a lattice point is `hash(seed, point)` within the amplitude of the highest level among the four sectors touching it; a point along an edge is the linear interpolation of the two ends, and a floor corner never moves in height, so the surface is watertight (test: one undisplaced corner always lands in one place). Applied where `faces.rs` builds corners (floors, walls, derived and banded walls through their `point` closures); the texture stays on the undisplaced lattice; planes are not jittered. The cook welds, rounds and lights the displaced faces, collision is cooked from the document (test: sectors and wall records identical, the cooked vertex count unchanged). Noise tint: `GridFace.tint_noise`, applied per corner in `light::noise_tint` by the preview and the cook alike. Height paint rules: `PaintRules.floor_by_height`, `wall_by_height` (`HeightRule`: three thresholds in 64 unit steps, four materials); the verbs paint floors by height unless the floor is steep and the class rule names a material, and make the strata band set (`ensure_strata`, absolute lines). `limits::validate` warns `RoughTooLarge` above 4 runtime units. MCP `grid_rough`, `grid_paint_rules` (with a region it reapplies the floor rules).

**Clip census** (Rough's cap). Collision radius of the player 12 ru (192 editor units). Aletha's bind pose (`aletha_mirror_458.psxmdl`, vertex bounds by part, 0.0171 editor units per local unit): hips and legs about 6 ru either side of the axis, chest 4, hands in the T-pose 16.5 ru (outside the radius already, unjittered). The model sits about 6 ru off its origin along z. So inward rock at the R/3 cap (4 ru) leaves the torso about 2 ru; R/2 (6 ru) would touch it, so the cap stays at R/3. Camera: the arm backs off by the margin `ThirdPersonCameraConfig.collision_margin` (default 10 ru, a cap on a proportional margin; `third_person_camera.rs`), the preview near plane is 8 ru, so 4 ru of jitter stays inside both. Static numbers, bind pose only; animation reach was not measured. In Play (cave, player pressed into the west wall, level 3 against no rough): the path ends at the same position (collision identical) and the frame shows no clipping of the model into the rock.

**Bits (fewest-bits rule).** Cooked format: no new field and no version bump; `ColWall.flags` (u8, now 3 of 8 bits used) gets `WALL_BACK_ONLY` (1 bit), `ColWall` stays 12 B. Cracks cost the existing `SplitB` (16 B) and `SectorExt` (8 B) per cracked cell, a cliff wall 12 B. Document fields, current against minimum against chosen: `GridSurface.void` 2 bits needed, `u8`; `GridSector.rough` 2 bits, `u8`; `Rough.seed` 16 bits (a pattern selector; 8 would do), `u16`; `Rough.amp` 6 bits each (at most 51 grid units at the cap), `u8`; `Rough.tint` 7 bits (0 to 100), `u8`; `EdgeOverride.top_delta` 8 bits each in 64 unit steps (open walls are a few sectors tall), `i16` each in grid units; `Axis` 2 bits (was 1); `HeightRule.at` 9 bits each in 64 unit steps (about 16000 grid units), `i16`. These are editor document fields in RON; the runtime tables carry none of them. The face budget is unchanged for a level with none of these.

**Streaming constraints** (`streaming-design.md` section 4) respected: derived diagonals, cliff walls and jitter are functions of one sector and the world lattice (`Jitter` hashes the absolute lattice point and reads the sectors around it before any cut, as item 8 requires); `EdgeKey` for a diagonal is world anchored; nothing is indexed by room-local ids; `Rough.seed` is per room, so a room cut into units keeps one seed. No new conflict.

**Deviations.** `grid_polygon` takes sector coordinates as numbers. Rough's `targets` are not separate: floors, walls and ceilings that meet at a lattice point must move together, so a level covers all three. `grid_chamfer` mode `crack` takes the rectangle as the plateau. Break's `spawn` is a list of sectors, not node ids. Lasso and the editor verbs are in the editor guide.


## 22. As built: T8, leaf storage and spans (2026-10-18)

Branch `grid/quadtree`. The one data-model change, with no behaviour change: every level made so far loads, edits, draws, cooks and plays exactly as before. Step 0 (separate commit): `make runtime-numeric-guard` flagged the 64-bit code the streaming-ready phase added; `Lattice::build` now sizes its cells in `u32` and `StableId::of_node` takes the node id as two 32-bit words (`[low, high]`, same FNV bytes, a test pins it), so only the `cd_drive.rs` baseline is left.

**The model.**

| Type | File | What |
| --- | --- | --- |
| `LeafKey { level: u8, x: i32, z: i32 }` | `grid/leaf.rs` | Level 0 to 4 (16, 8, 4, 2, 1 clicks), minimum corner in **world** clicks, aligned to its own edge. `LeafKey::sector([x, z])` is level 2. Derived order `(level, x, z)` (for level-2 leaves that is the sector order, so reference ordering is unchanged). `order()` is the Morton position `(interleave(biased x, biased z), level)`; a leaf covers the half-open range `[m, m + n*n)`. `parent`, `children` (Morton order), `containing(level, click)`, `sector_range`, `touches_sector`. |
| `LeafMap` | `grid/leaf_map.rs` | `BTreeMap<LeafOrder, (LeafKey, GridSector)>`: disjoint, aligned, Morton iteration. `get`, `leaf_at(click)` (sector level first, then 3, 4, 1, 0), `conflict(key)` (ancestor probe plus one range scan for descendants), `insert` (refuses misaligned and overlapping keys, returns the old leaf for the same key), `get_or_insert_with`, `remove`, `iter`, `iter_mut`, `in_sectors(lo, hi)`, `retain`, `sector_bounds`, `is_sectors_only`. |
| `GridSpan { floor, ceiling, wall_material }` | `grid/sector.rs` | One open volume. |
| `GridSector` | `grid/sector.rs` | The leaf value: a **never empty** column of spans (private `spans: Vec<GridSpan>`, bottom to top, cap `MAX_SPANS_PER_CELL` 8) plus the six wall stacks and `rough`. `GridSector::default()` is one vacant span. |
| `GridRoom.leaves: LeafMap` | `grid/room.rs` | Replaces `sectors: Vec<Option<GridSector>>`. `width`, `depth`, `origin` stay: the rectangle the leaves must lie inside. Growing, trimming and moving the origin never touch a key. |

**Accessors.** On `GridSector`: `spans()`, `spans_mut()` (slice), `span_count()`, `is_single_span()`, `insert_span`, `remove_span` (the last span is replaced by a vacant one, never removed), `surfaces()` and `surfaces_mut()` (every floor and ceiling of the column), `span_at(y, reach)` (the one resolver: candidates are spans whose lowest floor corner is at most `y + reach` and whose highest ceiling corner is above `y`, a pit and open sky always qualify, the highest floor wins), and the **ground span** accessors `floor()`, `ceiling()`, `floor_mut()`, `ceiling_mut()`, `floor_and_ceiling_mut()`, `set_floor`, `set_ceiling`, `take_floor`, `take_ceiling`, `floor_slot_mut`, `ceiling_slot_mut`, `wall_material()`, `set_wall_material`, `with_surfaces(floor, ceiling)`. On `GridRoom`: the sector adapter keeps its signatures (`sector`, `sector_mut`, `sector_or_insert`, `local`, `world_cell`, `sector_count`, `iter`, `grow_to_include`, `trim_empty`; `sector_or_insert` now returns `None` where a leaf of another level holds the cell) and gains `sector_key`, `set_sector`, `take_sector`, `leaves()`, `leaf`, `leaf_mut`, `leaf_at(click)`, `leaf_count`, `contains_leaf`, `insert_leaf` (refuses a leaf that is outside the rectangle, misaligned or overlapping), `remove_leaf`, `is_uniform`. **`GridRoom::iter()` keeps the old `z` major order**, not Morton: the cook numbers quads and vertices in iteration order and every golden hash depends on it (the first port run failed `uv_golden` for exactly this reason). `FaceRef` is `{ room, leaf: LeafKey, target }` with `cell()` and `FaceRef::new(room, leaf, target)`; the `floor`, `ceiling`, `wall`, `derived` constructors still take a sector and build the level-2 key; `FaceRef::sector` resolves any leaf level; `CornerRef::grid_pos` measures from the leaf corner in leaf edges. `GridWorld::floor_height_at` finds the leaf of any level and takes the highest floor over every span.

**What reads the ground span only.** The port (99 files changed, 7 of them in `psxed-mcp`, see the report) replaced about 430 `.floor`, `.ceiling` and `.wall_material` field uses with these accessors, found by the compiler (the fields are gone) and rewritten at the reported spans. Every caller of `floor()`, `ceiling()`, `wall_material()` and their mutable forms reads span 0: the tools, derivation, faces, light, cook, collision cook, portals, walkability, heatmap, MCP, preview. That is the whole column in every level made so far. Aggregate readers use `surfaces()` or `spans()`: `height_range`, `material_ids`, the coordinate check, `floor_height_at`. Nothing is silent: `limits::validate` reports, as errors that keep the world from cooking, `ExtraSpansNotSupported` (a column of more than one span), `LeafLevelNotSupported` (a leaf that is not a whole sector), `TooManySpans` (over 8) and `LeafOutsideRoom` (replaces `StorageMismatch`; the old check had nothing to compare once the storage is a map). T9 lifts the leaf guard and MS2 the span guard, consumer by consumer; `rg 'floor\(\)|ceiling\(\)'` lists the sites still on the ground span.

**Disk form.** A room whose leaves are all whole sectors inside the rectangle is written as the dense `sectors` list (`width * depth` entries, `z` major) exactly as before; any other room is written as `leaves: [(level, x, z, sector)]` in Morton order, with no `sectors`. Both load. A sector of one span is written with the plain `floor`, `ceiling`, `wall_material` fields; the spans above the ground one go in `above`, left out while empty, and ride either room form (extra spans do not force the leaf form: they are per sector, so a tall hall in an otherwise uniform room keeps the dense list). A dense list longer or shorter than `width * depth` loads as far as it goes (entries past the depth land outside the rectangle, where validation reports them). The six reference projects, the planes demo, the gate fixture and the starter load dense, and `to_ron_string` of each is stable (loaded, written, loaded and written again gives the same text and an equal document; test `the_reference_projects_load_dense_and_round_trip_as_documents`). Serde is hand written for `GridSector` (`SectorDoc`) and `GridRoom` (`RoomDoc`, a streaming dense writer); no new field has a non-default value in an existing file.

**Fewest bits** (document fields; the runtime tables carry none yet). `LeafKey.level`: 3 bits needed, `u8`. `LeafKey.x`, `.z`: 12 signed bits over the editor extent of +-409 sectors (+-1636 clicks), 9 bits inside a room of at most 128 sectors, chosen `i32` (the type of every document position; the leaf format of T10 stores the 9 bit room-relative value). Span count: 3 bits (cap 8), the leaf format spends one flag per leaf and 3 more bits only when set (`multi-storey.md` 7.7); the document keeps a `Vec`. Per span the document adds nothing to a one-span column on disk. `MAX_SPANS_PER_CELL` 8 is a proposal, exposed as `limits::MAX_SPANS_PER_CELL`. The cooked format did not change: no version bump, no new section.

**For T9 and the storey phases.** Ready: `LeafKey::{parent, children, containing, sector_range, touches_sector}`, `LeafMap::{conflict, insert, remove, in_sectors, sector_bounds}` for subdivide and merge (T9 builds the balance and hanging-corner `normalize` on them), `GridRoom::{insert_leaf, remove_leaf, leaf_at, contains_leaf}`, `FaceRef` on any leaf, the span resolver and `insert_span` / `remove_span` for Slab, Insert and Cut. Not built, deliberately: `normalize`, balance, constrained corners, planarity checks, `SectorNotQuartered`, per-leaf faces (`GridFace` still names a world sector; the pick converts with `LeafKey::sector`), the clipboard (`ClipSector` copies the ground span), the storey cursor, `storeys()`, R4 and the open-set derivation, and the codec.

**Differences from the design text.** (1) `iter()` is `z` major, as above. (2) The ground span accessors were first named `floor()` and `ceiling()`; T9 renamed them `ground_floor()`, `ground_ceiling()` and so on (`ground_floor_mut`, `set_ground_floor`, `take_ground_floor`, `ground_wall_material`, `set_ground_wall_material`, `ground_floor_slot_mut`, `ground_floor_and_ceiling_mut`) so that `rg 'ground_(floor|ceiling)'` lists every span 0 reader (mechanical commit, compiler reported). (3) The leaf value type keeps the name `GridSector`. (4) `GridRoom::index` is gone (no dense storage to index). (5) The room test of a stored list of the wrong length became `LeafOutsideRoom`.

**`GridRoom::translate`.** Because leaves are keyed in the world, assigning `room.origin` alone no longer moves a room's contents (it used to). `translate(delta)` moves the origin and every leaf together and refuses a move that would misalign a level-0 or level-1 leaf. Every other assignment of `origin` in the tree happens on an empty room; the one test that moved a populated room (`uv_region`, merged from `grid/main`) now calls `translate`.

**Gates** (after merging `grid/main` 24afa37e and f8d3d1f9, whose `uv_region`, `graybox_set` and `fit_across` code compiled against the accessors unchanged; the single change in merged code is `translate` in that test and a borrow split in the MCP `planes.rs` ceilings-as-floors helper).

* Tests: `psxed-project` lib 685 pass, 1 ignored measurement (`uv_golden`, the all-faces hash of the six reference levels and the planes demo, passes unmodified); `psxed-ui` lib 482 pass, 3 ignored; `psxed-mcp` unit tests and the cave, diagonals, level, mcpfix, natural, perch, tools and registration integration tests pass; `frontend` has only the known red `editor_frame_selection_is_applied_before_preview_extraction`. Test changes: mechanical accessor syntax everywhere, plus four that had to change meaning: the room storage-length test became the leaf-outside-the-rectangle test, the starter caps test checks the new message, a `uv_region` test moves its room with `translate`, and a Top-view test sets floors through `leaves.iter_mut()`. `cargo clippy -D warnings` and `cargo fmt --check` are clean for `psxed-project`, `psxed-ui`, `psxed-mcp`, `frontend`, `psx-grid` and `psx-level`. `make runtime-numeric-guard` reports only the `cd_drive.rs` baseline.
* Cook hashes (`tools/grid-simgate/cookhash.sh`): all eight fixtures equal `cookhash-golden.txt` byte for byte, before and after (core, rooms and manifest). The cathedral disc image built before and after is identical.
* Sim-state gate (`simgate.sh`, route tape on the cathedral, dungeon, cave and starter, the gate fixture east and east then west; stop poll 2300): identical on every judged tick, 1200, 1197, 1198, 1198, 1198 and 1198 ticks, zero different. Baseline is the tree before the port (step 0 included), run with the frontend built from it.
* Frames: `t8-01-reference-levels-unchanged.png` shows the cathedral, dungeon, cave and starter in the editor 3D view after the port, and the four renders (1920 by 1080) are pixel-identical to the same renders from the pre-port frontend.
* New tests (`leaf.rs`, `leaf_map.rs`, `leaf_tests.rs`, `limits.rs`): key alignment, parents and children, Morton order and range containment; a random quadtree against a brute-force lookup with overlap and conflict probes; the dense form of every reference project round trips as a document and is stable as text; the leaf form for a mixed room; extra spans in the dense form; short and long dense lists; accessors that never overlap other leaves; the `z` major adapter order; leaves inside the room and disjoint; grow, trim and translate; `FaceRef` on level-3 and level-4 leaves with corner positions; the span resolver (storeys, slab, reach, pit, open sky, the column is never empty); `floor_height_at` over spans and a half-sector leaf; validation of leaves outside the room, other-level leaves, extra spans and the span cap.

**Measurements** (editor frame time, `measure_verb_gesture_cost_on_the_cathedral`, release, Cathedral region of 676 sectors and 1662 faces). Quiet machine, one run before the port: apply 0.048 ms, `all_faces` 0.276 ms, Top view editor frame 0.136 ms. The machine was loaded by other builds for the comparison (load average about 10), so base and new binaries were run alternately 12 times each: base median apply 0.097, `all_faces` 0.589, frame 0.301 ms (minimum 0.087, 0.519, 0.255); new median apply 0.224, `all_faces` 0.661, frame 0.331 ms (minimum 0.193, 0.586, 0.313). So a verb's `apply` costs about twice as much (restoring a sector is now an ordered-map store, 0.1 ms more over 676 sectors), `all_faces` about 13 percent more and a whole frame about 10 to 25 percent more; all well under a millisecond. A sector lookup is 31 ns through the `BTreeMap` (`measure_leaf_storage_on_the_cathedral`). Two cheaper variants were built and measured and dropped: a hash index with a slab (16 ns a lookup but no faster `all_faces`, because the neighbour probes of the face derivation are not what dominates) and an inline first span (`SmallVec`, no change). If the T10 measurement of the point query or a bigger room says the lookup matters, the place to spend is a dense table for the level-2 leaves inside `LeafMap`; the API does not change.

## 23. As built: T9, quadtree operations (2026-10-18)

Branch `grid/quadtree-ops` (from `grid/main` 24551208). The rename of the ground span accessors came first (section 22, difference 2). Everything below is host code (`psxed-project`, `psxed-ui`, `psxed-mcp`); nothing in `engine/` or `sdk/` changed, and the cooked bytes of every existing level are unchanged.

**Operations** (`grid/tree/`). `split_sector` / `join_sector` divide a leaf into four children and join four back. A surface is the triangulated quad, so each child samples the parent's two triangle planes: a child the diagonal crosses keeps the parent's split and fold, the other two lie in one triangle and are planar; materials, void bits, `walkable` and the auto-split flag copy; midpoints round half up (`half`), exact for the 64 gu heights the tools write. An outer-edge wall is cut at its middle, the first half going to the child at its `a` end, and a diagonal wall goes to the two children it crosses. `join_*` is defined as "split the candidate parent and compare with the four children", so a merge is exact or refused (`JoinBlock`: spans, surface, wall, roughness). `subdivide`, `subdivide_to`, `subdivide_all_to`, `can_merge`, `merge`, `merge_all` (finest first, repeated, `top_level` limits the size) are in `ops.rs`; the room's `edge_overrides` split and join with the leaves (`edge_overrides.rs`; unequal halves or an override between siblings block a merge). Not built: an automatic merge inside `normalize` (design 3.5 step 4). A subdivision the author asked for must survive the next edit, so merging is its own command; `normalize` only adds leaves and rewrites corners.

**`normalize`** (`normalize.rs`): (1) a leaf above a sector that is not one plane with no wall is subdivided (`big_leaf_ok`; walls on a big leaf subdivide it, so a big leaf never carries an authored wall); (2) `balance`: a neighbour two levels coarser is subdivided, to a fixpoint; (3) `constrain`: where a coarse leaf C has two finer leaves F1, F2 across a side, the hanging corner of each takes `half(a, b)` of C's two end heights **only when the surface is continuous along the whole edge** (F1's far corner equals C's `a` and F2's equals `b`). The first version constrained each half on its own end and was caught by the property test: it changed the look of a fine leaf next to a cliff that happened to match at one end. Coarse leaves are processed from the coarsest level down, so a corner that is itself hanging is settled first. Floors, ceilings and every span index present on all three leaves. `check` returns `TreeIssue`s (unbalanced, big leaf not planar, void/split-level/diagonal wall below a sector, hanging corner off its edge), mapped to `GridIssueKind::{LeafUnbalanced, BigLeafNotPlanar, LeafFeatureNotSupported, HangingCornerOff, SectorNotQuartered}` (the first three and the last are errors, the corner is a warning). `LeafLevelNotSupported` is gone.

**Faces** (`leaf_faces.rs`, `leaf_derive.rs`). `GridFace` now carries `leaf: LeafKey` next to `cell` (the sector a face is listed under). A room with a leaf that is not a whole sector (`LeafMap::is_sectors_only()`, a counter) is built by `leaf_faces::cell_faces`: per leaf touching the sector, floor and ceiling through the same `Builder::surface` (generalised to a `Square` of any size; a leaf above a sector is clipped to the sector by sampling its plane at the sector's corners, `height_in_leaf`), authored walls of leaves of a sector or smaller through `Builder::wall`, and derived walls per **segment of the finest common subdivision** of each edge, cut at sector borders so every face lies in one cell. **Stitching**: a side with finer leaves across it has its middle as a vertex of theirs; `stitch` inserts it into every floor and ceiling ring along that side (height `half(a, b)`, the texel the average) and fans a ring of more than four corners from the first hanging vertex, so there is no T-junction. A room of whole sectors keeps the old code path: a test runs the leaf path over every cell of the seven reference levels and gets the same faces. Differences in a mixed room, all deliberate: rough jitter is off (it lives on the sector lattice), banded walls get no neighbour stitching, a derived wall's fit run is its own segment, no diagonal derived walls below a sector. The cook's `uv_golden` and the eight cook hashes are unchanged.

**Cook and collision.** The runtime still reads per sector, so the cook builds collision, portals and the light bake's line of sight from `sector_view::sector_room` (also used by walkability, the heatmap and the overlays): a leaf above a sector expands to its sectors exactly (a plane), fine leaves that merge back (`merge_all_unbalanced`, exact, no balance rule) are that sector, and any other sector that holds fine leaves stands on the heights at its four corners with the split the heights ask for. That last case is **approximate** and the cook reports it, one warning per sector. The render tables come from the real leaves. `GridRoom::face_cells()` (the cells faces are listed under) replaces `iter()` in the cook, preview and picking. **What T10 must still do**: the leaf record and per-sector index in the cooked format (design 10.2), exact collision for sectors with fine detail (the demo's four stair sectors are collided as a ramp, within 32 gu of the treads), leaf rays and culling, the leaf-aware `wall_segments` for derived walls of the 3D picker (a derived wall on a leaf above or below a sector cannot be picked or painted yet), `Break`, `Ramp`, `Stairs`, `Rough` and the Floor/Ceiling/Wall rectangle tools on cells that are not whole sectors, and the preview change in the emulator repo (below).

**Editor.** `Q` is the Cells tool (`cells_tool.rs`): a brush (click or drag, radius in quarter sectors with `[` and `]`) that subdivides the cell under the pointer one level or down to half or quarter, Alt merges, Shift+M merges the cell under the pointer, one undo step per stroke (the Select tool's Subdivide cells and Merge cells buttons act on picked cells; the Cells tool has no selection of its own). Alt+L (and the Cells button) draws every cell outlined by size in the Top and 3D views (`leaf_overlay.rs`), the status bar counts cells by size (`GridBudget::{leaves, by_level, levels_text}`). The Top view draws leaves and their derived walls; hover, selection, `tri_corners`, the Height tool's Top picking and the pick cache address the leaf under the pointer. A corner drag first makes its corners movable on their own (`refine_corners`: a leaf above a sector is subdivided only when the corners that move would bend it, so lifting a whole big face moves it whole) and normalises after each application; Raise and Lower move every leaf in the region and split a bigger leaf to fit (`raise_leaves.rs`, which restores the room's leaf map before each application).

**MCP.** `grid_subdivide`, `grid_merge` (sectors, or clicks with `unit: click`), `grid_leaves` (counts and a per-click level map), `grid_raise` with `unit: click`, the leaf list in `grid_sector`, `%` in `grid_heights`, `leaves` in `grid_plan`. Test `tests/grid_cells.rs`; its ignored `build_the_cells_demo` makes the demo (below).

**Property tests** (`tree/tests.rs`, `leaf_face_tests.rs`, `cook/leaf_tests.rs`, `sector_view.rs`, `edit/leaf_tests.rs`, psxed-ui and psxed-mcp `grid_cells.rs`): subdivide then merge is the identity (12 random rooms, then two levels deep); the height field at every click is unchanged by subdivide and by merge; a plane over 8 by 8 sectors merges to four level-0 leaves and back; 20 random sequences of 60 subdivides, merges, corner lifts and deep digs keep the tree balanced, big leaves planar and corners constrained, `leaf_at` equal to a brute-force scan, `normalize` idempotent, and the room serialising round trip; stitching removes every floor and ceiling T-junction in 40 random mixed rooms (the check fails with stitching turned off); the leaf face path equals the sector path on all 7 reference levels; mixed halls cook with no hole (`cook::mesh::open_edges`).

**Gates.** `psxed-project` lib 724 pass (1 ignored), `psxed-ui` lib 487, `psxed-mcp` unit 66 and every integration test (cave, diagonals, level, mcpfix, natural, perch, tools, registration, cells). Cook hashes (`tools/grid-simgate/cookhash.sh`): all eight fixtures equal `cookhash-golden.txt`. `cargo clippy -D warnings` and `cargo fmt --check` clean for the three crates. `make runtime-numeric-guard` reports only the `cd_drive.rs` baseline. The sim-state gate was not re-run: the cooked bytes of every level it plays are identical to the golden hashes, so the guest cannot differ.

**The emulator file.** The 3D preview (`emu/crates/frontend/src/editor_preview/grid.rs`) walks `room.iter()` and `cell_bounds` with a level-2 sector; it is an editor-owned file tracked in this repository (the components check accepts edits to it). It walks `room.face_cells()` and bounds a cell by every leaf in it, so mixed rooms draw fully. Play builds do not use that file; the cook reads the leaves itself.

**The demo.** `editor/projects/grid-cells-demo` (untracked, like the other generated projects): a 12 by 12 sector derived hall merged to nine 4-sector cells, the south-east 2 by 2 sectors in quarter-sector cells, balanced all the way out (1/2, 1, 2 and 4 sector cells around it), and eight treads of 64 gu a quarter sector deep made by eight `grid_raise` calls with `unit: click`. 724 faces. Play: the player walks east up the treads (height 0 to 624 editor units, 28.9 fps with no telemetry) while collision stands on the four sectors' corner ramp.

## 24. As built: T10, leaf cook and collision (2026-10-18)

Branch `grid/leaf-cook` (from `grid/main` 0f86a1d6).

**Format (still version 4, no bump).** `CHUNK_TREE` (chunk flag bit 4, and `ROOM_TREE` bit 4 of the core room flags) marks a leaf tree. `SECTORS` then holds one 20 byte `Sector` per leaf; flag bits 13 to 15 carry a size code (0 sector, 1 two sectors, 2 four, 3 half, 4 quarter; code 0 is what every dense record already has, so dense bytes are unchanged). The 15 entry section table could not grow without changing every dense room, so a `TreeExt` (20 B, own layout hash, `TREE_LAYOUT_HASH`) at `RoomHeader.tree_ext` places `SECTOR_INDEX` (u16 per sector: bit 15 sub-block, bit 14 reserved clear bit, 14 bit id, `0x3FFF` none) and `SUBBLOCKS` (16 u16 per divided sector, a half leaf in four slots). `RoomHeader.origin_mod` (4 bits) gives leaves of 2 or 4 sectors their world alignment. `ColWall.flags` gained `WALL_HALF_FIRST`/`WALL_HALF_SECOND` (a derived wall hosted by a leaf next to smaller ones covers one half of the edge; 2:1 balance makes whole or half the only cases). Not written: the per-sector CLEAR summary in tree rooms, the SPANS section (MS5).
Fewest bits: index and block ids 14 bits (limit 16383 leaves, 1024 .. 16383 blocks by the same field), size code 3, origin_mod 4, wall span 2 flags; the leaf record is the existing 20 B `Sector`.

**Runtime.** `psx-grid::collision::leaf` resolves a point to its leaf in two dependent loads and a leaf frame (origin, edge, reciprocal per size code). Tree rooms use separate scan, wall and ray functions chosen per room by `ROOM_TREE`; dense rooms run the old code. Tests: a tree-encoded copy of a dense room traces identically over 6000 queries, random trees match a brute force leaf list at every runtime unit, treads, half spans, big leaves with offset origin, golden fixture for the tree layout. The scratchpad stack entry was at 1000 of 1004 bytes; outlining `trace_character_blocker` and `trace_aabb_blocker` in `psx-engine` (no behaviour change) takes it to 760.

**Cook.** `cook/leaf_collision.rs` cooks every leaf in its own frame (exact fine treads, no sector view approximation), derived walls hosted by the leaf, and diagonal derivation on leaves of a sector or smaller (`leaf_diag.rs`: split-level, void and chamfer detail on half and quarter cells; subdividing such a cell is no longer refused). Portals still read the sector view. Rough jitter works in mixed rooms (click lattice, hanging vertices move with the rock).

**Editing.** `edit/fit.rs` (split to fit), `edit/lens.rs` (a footprint of half or quarter cells is mapped to a temporary uniform room with heights scaled so slopes and steps keep their runtime meaning; heights come back unscaled, unchanged ones exactly, changed ones snapped to 64 gu; edge overrides of the real room are not carried), `verbs/fine.rs` (Raise, Ramp, Stairs, Break through the lens, drag-safe), floors and ceilings on cells of any size, cracks, voids and wall heights on any leaf, authored walls on leaf edges, derived walls on leaves addressed by `FaceTarget::Derived.seg`, Rough per leaf, Convert for mixed rooms, `Region` carries a cell level. MCP: `size: half|quarter` on grid_raise, grid_ramp, grid_stairs, grid_break and grid_rough regions and on grid_add_surface and grid_remove_surface; `unit: click` on grid_crack, grid_void, grid_place_wall and wall references (`segment` for a side with several faces).

**Gates run.** psx-grid, psxed-project (740) and psxed-mcp tests pass. Cook hashes of the eight reference fixtures equal `cookhash-golden.txt` (checked before the rough, lens and verb commits, which touch mixed rooms only). Sim-state gate on cathedral, dungeon, cave, starter, grid-gate east and east-west: identical on every judged tick against the T8 runs; total bus cycles at tick 2290 within 0.03 percent. Host motor test: the real motor climbs four fine treads at exactly their heights; Play of `grid-cells-demo` climbs the fine treads and of `grid-natural-cells-demo` (crack and chamfer on quarter cells) collides with the chamfer wall.

**Not done.** At a fine cell size the editor refuses Lasso, Cut corners and diagonal walls, and draws no Ramp or Stairs ghost (the cell size selector itself landed on grid/cell-ui); neighbour stitching of banded derived walls across leaf sizes; leaf rays and culling beyond the per-sector walk; screenshots and the editor pose captures; MS5 spans.
