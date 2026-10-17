# Multi-storey authoring on the grid (2026-10-17)

Status: design study, read-only. Written against `grid/main` (HEAD d48628db) in the `grid-editor` worktree, plus the `grid/natural` and `grid/compact` worktrees read as they stand (neither is committed: `compact-format.md` does not exist yet). Nothing was built or run; every "OK" or "wrong" below comes from reading code and docs, not from a test I executed.

Manny's brief: "in bonnie32 having multiple floors was VERY awkward, you had to plan everything in advance, I want you to find a better way of having multiple floors, and prepare a test level with an octagonal room like a tower, with stairs going around the sides opening to each floor."

How to read the numbers. **[measured]** came from a file or a run named beside it. **[derived]** is arithmetic on measured values or struct sizes. **[estimate]** is reasoning with no run behind it. **[proposal]** is a chosen constant with no measurement behind it. **[hypothesis]** is a claim I expect to be true and have not tested; each names the experiment. Citations are `path:line` in this tree, `sha:path` for history (read with `git -C /Users/ebonura/Desktop/repos/PSoXide-editor-wt/grid-editor show <sha>:<path>`), or `B32:` for `/Users/ebonura/Desktop/repos/bonnie-32`. Path prefixes: `P/` = `editor/crates/psxed-project/src/grid/`, `U/` = `editor/crates/psxed-ui/src/grid_tools/`, `M/` = `editor/crates/psxed-mcp/src/`, `G/` = `engine/crates/psx-grid/src/`, `D/` = `docs/grid/`.

Units used in every example (default scale 5/4, `D/level-tools-design.md` section 1): sector = 1024 grid units (gu) = 80 runtime units (ru); click = 256 gu = 20 ru; fine step 64 gu = 5 ru; body height 48 ru and step-up 40 ru (`WalkLimits`, `D/level-tools-design.md` section 18).

---

## 0. Summary

1. **Why it was awkward.** In bonnie-32, in the old PSoXide floors and in today's stacked rooms, a storey is a *container* (a room, a `floors_above` grid) that owns one floor and one ceiling per cell. Geometry cannot exist at a height until a container exists there, the container fixes its own footprint and Y, two containers connect only if their numbers match exactly, and every tool, pick, light and entity must know which container it is in. Each of those forces a decision before the first wall is drawn. Evidence in section 1.
2. **Recommended model: spans in the column, storeys as a view.** A cell (and later a quadtree leaf) holds an ordered list of **spans**, each span one open volume with its own floor and ceiling surface. One span is every level made so far, so nothing existing changes. A storey is not stored structure: the editor derives the list of storeys from the spans, lets you name them, and offers a **working height** (the storey cursor). Rooms stay as footprint, ambient, fog and streaming grouping, and stop being a vertical unit.
3. **Walls across storeys fall out of one rule.** A derived wall is where the open set of one column and the open set of its neighbour disagree along an edge (section 4.4). Today's riser, drop and closure rules (R1 to R3) are the single-span cases. One new case, the slab edge (R4), covers a floor slab seen from a taller neighbour. Bands, overrides and materials work per face as before.
4. **After-the-fact edits become one operation each.** *Insert slab* splits a span (a floor in the middle of a tall hall). *Cut hole* merges two spans (a stairwell). *Shift above* moves everything above a height (make this storey taller). None of them asks you to have planned.
5. **Editor UX.** A storey cursor with Alt+Up/Down and a Levels panel; the Top view shows the slice at the cursor with the storey below ghosted and slabs above hatched; the 3D view cuts everything above the cursor's storey (Sims style, render and pick share the cut); every verb acts on "the span at the cursor" through one resolver; one new verb, **Slab (M)**, with modes Insert, Cut, Fill.
6. **Collision gets cheaper for stacks.** The runtime already turns a column into forbidden feet intervals and scans every room for them (`G/collision/scan.rs:176`, `ray.rs:157`, `world.rs:317`). Spans are more intervals in one cell record chain instead of one scan per stacked room. Single-span cells take the path they take today.
7. **One data change, at T8.** The leaf value of the T8 `LeafMap` becomes a column of spans. The disc format carries extra spans in a new section at the v4 bump that T10 already needs. Existing projects load and cook byte-for-byte as now; the Dungeon's two stacked rooms migrate with one command.
8. **Test level.** `grid-tower`: a 12 by 12 sector octagon (chamfer 3 sectors), four floors 12 clicks (240 ru) apart, a 2 by 2 central well, a one-sector-wide stair ring winding one and a half times round the wall (a half turn per storey), built from a single tall hall by Insert slab, Cut hole and Spiral stairs. Section 8.
9. **Decisions I need from Manny** are in section 10, each with a recommendation. The ones that change the work most: spans in the leaf versus a tactical Sims-style layer over rooms (10.1), the slab thickness and minimum headroom constants (10.2), and exterior facing of derived walls for outside views (10.6).

---

## 1. Diagnosis: why every earlier attempt forced planning

### 1.1 bonnie-32

* **One floor and one ceiling per sector, rooms are the only way up.** `Sector { floor: Option<HorizontalFace>, ceiling: Option<HorizontalFace>, walls_* }` (`B32:src/world/geometry.rs:1499-1520`). A second floor above the first in the same footprint cannot live in the same room. The Floor tool refuses it: "Sector already has a floor" (`B32:src/editor/grid_view.rs:1272-1273`), and the 3D preview marks the cell occupied (`B32:src/editor/viewport_3d.rs:773-785`). So the author must already know that the area is two storeys, and create a second `Room` with its own `position.y`.
* **Defaults pin heights.** A new floor goes at Y 0 and a new ceiling at 3072 unless Shift-height is used (`B32:src/editor/viewport_3d.rs:790-794`, `CEILING_HEIGHT = 3072`, `state.rs:107`). `placement_target_y` persists between placements, so the "current level" is a hidden number.
* **Room Y has no editing UI.** The Properties panel prints `Pos: (x, y, z)` as text (`B32:src/editor/layout.rs:3173`), and the inventory records no field that sets `position.y` (`D/bonnie32-editor-inventory.md` section 2.8). BACKLOG asks for it: "We need a way to place rooms on top of each other, maybe the top view can be toggled with a side view?" (`B32:BACKLOG.md:18`).
* **Tools are bound to one room.** Every placement path reads `state.current_room` (`B32:src/editor/viewport_3d.rs:622`, 662, 718, 773, 809, 850, 882, 945, 983); vertices and edges are pickable only in the current room (inventory 2.3). Moving up a storey means changing the active room in a list and remembering which room is which.
* **Vertical connection is exact equality.** A horizontal portal exists only if one room's ceiling meets the other's floor at all four corners within 1.0 unit (`B32:src/world/geometry.rs:3877-3960`, `HEIGHT_TOLERANCE` at 3888), or if both have a hole (cases 3 and 4, 3974-3984). Raise a ceiling by one click and the portal silently disappears on the next recompute. The upper room's `position.y` must be computed from the lower room's ceiling before it is drawn.
* **Cross-room edits are partial.** Heights drag welds coincident vertices "in any room" (inventory 2.3), but no operation I found moves a face from one room to another.

### 1.2 The old PSoXide floors (retired 2026-08-25)

Design: a `Room` is a base grid plus `floors_above: Vec<WorldGrid>`, each its own free grid, auto-stacked "just above floor N-1's ceiling" (`e1e55509:docs/floors-plan.md`, "Model"). What worked, and the Sims-style floor view Manny liked (`D/old-grid-system-archaeology.md:234`):

* the active floor drawn at Y 0 with all floors below it descending (floors-plan "SIMS-STYLE EDITOR FLOOR VIEW");
* hole-gated vertical portals, 72 reduced to 22 once only real openings got one (floors-plan "AUTOPORTAL");
* an explicit `node.floor` for entities, after Y proved unusable as a key (floors-plan "demo11 reproduction", BUG 1: "Y is a placement DEFAULT ... a node's floor can't be inferred from Y").

What failed, with the cause each time a container was the abstraction:

* **Floor height was derived, not authored.** Auto-stack meant the storey height was the lower ceiling, so changing a ceiling moved every storey above it. The reference level demo11 was authored "SEALED": a solid slab, "no traversable hole", so the player could not reach floor 1 at all (floors-plan "demo11 IN-ENGINE PLAYTEST").
* **Container identity leaked everywhere.** "Which floor a node is on, and what Y it draws at" was re-derived at about nine call sites, six render walks plus cook, bounds and gizmo (`e1e55509:docs/floors-editor-architecture.md`, "The failure pattern"). Every floors bug of that week was one of those sites missing the check (entity drawn twice, selection hitting the floor below, player floating). Three fixes were proposed, none implemented.
* **The runtime did not know the stack.** Round 4: "the runtime never applies a room's `origin_y` at render time", so the upper room drew at Y 0. Round 6: the room switch condition was wrong for stairs (an open stairwell has no ceiling, so the up-branch never fired) and Y was not rebased into the new room frame (floors-plan, "RUNTIME" sections).
* **Stairs between floors were a cross-container problem.** A flight needs both floors, so it was a pair of edits and a per-cell link (`floor_above`/`floor_below`), wired at cook time and verified only by playtest.
* **The upper-floor entity gap was left open** ("entities/props/lights on UPPER floors still render at floor-local Y", floors-plan "BUG 3 ... REMAINING"). `D/old-grid-system-archaeology.md:251` lists it as item 12 of "what not to repeat" and recommends: "make floor a first-class container the tools, picking, preview and cook all resolve through one function." Item 13 adds: store world-space positions, derive room membership.

### 1.3 The current grid line (`grid/main`)

* **Floors are rooms with `y_offset`.** "Rooms may overlap in plan; that is how floors stack" (`P/room.rs:55-56`). Heights are relative to the room's offset (`P/surface.rs`, `GridSurface.heights`). The Rooms panel `+` adds an empty room "one storey above the highest" with the offset hard-coded to `top + 3 * SECTOR_UNITS` (`U/panels.rs:472-477`), whatever the storey height is.
* **"Which room am I editing" is a heuristic.** `top_room_at` returns the active room if it has geometry in the cell, else "the highest visible room that does" (`U/pick.rs:343-365`). The Top view draws rooms from the lowest `y_offset` up, upper ones translucent (`editor/crates/psxed-ui/src/grid_view.rs:158-190`), which is a stack of whole floors, not a slice.
* **Derivation stops at the room border.** A room's border is closure (R3) against empty space and cross-room derivation is explicitly out of scope: "Cross-room derivation would need the stacked-room disambiguation the dungeon needs for `locate`" (`D/level-tools-design.md:214`, open question 6 at 903). Dungeon room 1 (y 2048 gu) has "wall-only sectors that sit over 45 plan cells of room 0" (`D/other-maps-survey.md:58`), the author's way of faking a wall above a hall.
* **Queries scan every room.** `GridWorld::floor_height_at(x, z, y_limit)` loops all rooms and keeps the highest floor under `y_limit` (`P/world.rs:121-160`). The runtime does the same: `locate` loops rooms and breaks ties by `column_gap` (`G/collision/world.rs:279-370`), `scan_column` and the ray walk loop `0..rooms.len()` (`G/collision/scan.rs:176-190`, `ray.rs:157,211`). `D/streaming-design.md` section 4.1 item 12 already lists this as a CONFLICT for a world of hundreds of units.
* **Stacked portals are an O(cells x rooms) cook pass** that fires only when one of the two cells lacks a floor or a ceiling (`P/cook/portals.rs:153-175`).
* **MCP recipe.** "A second room at another height: `grid_add_room` with `origin` and `y_offset`, then `grid_add_surface` for its floor (and ceiling) at 0" (`D/mcp-guide.md:92`). Every tool is addressed by room index and heights relative to that room's offset (`M/grid/build.rs:48-65`, `M/grid/verbs.rs:46-72`).

### 1.4 The common cause, as six constraints

| # | Constraint | Where it bites | Evidence |
| --- | --- | --- | --- |
| C1 | The storey is a container with a fixed footprint and Y; geometry needs the container first | choose the room before you draw; footprint must hold the upper floor | 1.1 first bullet; `P/room.rs:55`; floors-plan "Model" |
| C2 | One floor and one ceiling per cell per container | a floor in the middle of an existing hall means splitting the hall into two rooms | `B32:geometry.rs:1499`; `P/sector.rs` `GridSector` |
| C3 | Vertical consistency by exact equality or by auto-stack | a ceiling edit silently severs the storey above; storey height is not a thing you author | `B32:geometry.rs:3888`; floors-plan auto-stack |
| C4 | Tools bind to one active container | stairs between storeys and holes through slabs are two-container edits | `B32:viewport_3d.rs:622...`; `U/mod.rs:566` (`grid_target_room`) |
| C5 | Derivation, portals and closure end at the container border | the seam between storeys is hand-opened (`hidden` + `solid false` on both closures) | `D/level-tools-design.md:214`; `D/mcp-guide.md` "An opening in a wall" |
| C6 | Non-geometry carries container identity | entities, picks, lights, cook binding | architecture doc "failure pattern"; archaeology items 12 and 13 |

The recommended model removes C1 to C3 by putting storeys inside the cell, C4 by one resolver, C5 by deriving across spans, C6 by never storing a storey id outside geometry.

---

## 2. Reference approaches (general knowledge, not checked against sources for this study)

* **Tomb Raider.** Rooms are separate meshes joined by portals; each sector stores a room below and a room above, so vertical lookups are O(1) but authoring is the same room-per-storey constraint, solved in editors by manual up/down room links. Our `floor_height_at` with `y_limit` plus room scan is the same idea without the per-sector link.
* **Build engine.** One sector per plan region; sector-over-sector is a hack (linked sectors) rather than a model.
* **Span per column** (the Recast navigation heightfield, multi-level surface maps in robotics, Minecraft-style columns). A grid cell holds an ordered list of spans along Y. Lookups are O(1) to the cell and O(spans) in it, and "what is open at this height" is the primitive operation. This is the closest fit to a PS1 grid world, and the existing collision already speaks intervals (section 7.3).
* **Dwarf Fortress style z-level slice view and the Sims floor switcher.** The editing metaphor for the cursor: show one level, ghost the one below, hide the ones above. Manny already liked this (`D/old-grid-system-archaeology.md:234`).
* **Doom 3D floors (control sectors).** Stacked floors as extra records attached to a 2D sector. Same family as spans, with an authoring story that needs the control-sector indirection we do not want.

---

## 3. Options compared

### 3.1 The options

* **A. Stacked rooms as today.** A room per storey, `y_offset`, derivation inside a room, stacked portals at cook. Fix the editor only (active-room chip, Top slice by room).
* **B. Per-cell span lists on the uniform grid.** `GridSector.floor/ceiling` become `spans: Vec<GridSpan>`; rooms stay a footprint. No quadtree.
* **C. Quadtree per storey.** A `LeafMap` per storey layer, storey index as part of the key, each layer with its own base height (the T8 `LeafMap` replicated).
* **D. Hybrid (recommended).** Quadtree leaf = a column of spans (B inside the T8 leaf), storeys derived plus optionally named, rooms as footprint and streaming grouping.

Rejected without scoring: real 3D brushes (the retired BSP path; the project went to the grid on purpose) and a dense voxel column (disc and RAM cost, no slope surfaces).

### 3.2 Scores

Legend: `++` clearly good, `+` good, `0` neutral or same as today, `-` costly, `--` blocks the goal. Reasons in 3.3.

| Criterion | A rooms | B spans, uniform grid | C layer per storey | D spans in the leaf |
| --- | --- | --- | --- | --- |
| 1a Add a floor in the middle of an existing tall hall afterwards | `--` | `++` | `-` | `++` |
| 1b Open a hole between storeys afterwards | `-` | `++` | `0` | `++` |
| 1c Change a storey's height afterwards | `--` | `+` | `-` | `+` |
| 2 Derived walls and bands across storeys | `-` (cross-room not derived) | `+` | `-` (layer pairing) | `+` |
| 3 Collision and camera cost | `-` (scan per room) | `++` | `-` (scan per layer) | `++` |
| 4 Render and culling | `0` (room ring) | `0` (span tag, see 7.5) | `0` | `0` |
| 5 Streaming units, compact encoding | `-` (chunk per storey) | `+` | `-` (tree per layer) | `+` |
| 6 Lighting across storeys | `0` | `+` (slab is an occluder in the column) | `0` | `+` |
| 7 Entities on upper floors (no container id) | `-` | `++` | `0` | `++` |
| 8 MCP expressiveness | `-` (room index plus offset arithmetic) | `+` | `0` | `+` |
| 9 Migration cost from today | `++` (none) | `0` | `-` | `0` |
| 10 Fits T8 to T10 once | `0` | `-` (second model change) | `-` | `++` |

### 3.3 Why

**1. Authoring after the fact.**
* A: no operation exists. To put a floor into a hall you cut the room in two, recreate the upper half as a room with `y_offset`, re-weld, and re-open connections (C1, C2, C5).
* B and D: the hall is one span; Insert slab splits it. A hole is the inverse (merge). Heights above a level can move together (Shift above), so a storey is no longer a number you commit to early.
* C: the layer index is the plan. A mezzanine at 1.5 storeys needs a layer, and inserting a layer between two existing ones renumbers every key, selection and entity that named a layer.

**2. Derived walls.** A and C derive inside one container and need a pairing step between containers (the "stacked-room disambiguation" `D/level-tools-design.md:214` defers). B and D derive from the column's open set, so a tall hall beside a two-storey column gets its slab edge from the same code that makes a riser (4.4).

**3. Collision and camera.** The motor is already built on forbidden intervals per column (`G/collision/scan.rs:1-6` "the forbidden feet-height intervals every body trace is built from"), scanning every room (`scan.rs:176-190`). Stacked rooms cost a footprint test and a possible view fetch per room per query. Spans cost one cell lookup and a loop over the cell's spans, usually one (`SECTOR_CLEAR` and `flat` fast paths unchanged for single-span cells, `G/format.rs:290-296`). I have no cycle figure for either [no measurement; the experiment is in 7.10].

**4. Render and culling.** No option changes the per-cell row-span cull. The multi-span cell lists quads of all its spans, so a camera under a slab still runs the facing test on quads above it. Upper floors facing up are back-facing from below and fall to the cheap test; walls of the upper span may pass it. This exists today for stacked rooms inside the portal ring (`D/level-format-and-runtime.md` header: "rooms in the camera room's ring that touch the frustum are drawn"). 7.5 describes the span tag that lets later work cull by span.

**5. Streaming and bits.** A room is a chunk (`D/streaming-design.md` 3.2), padded to a 2048-byte sector and a possible seek (one seek about 137 ms [measured per that doc]); a four-storey tower as four rooms is at least four chunks [derived from sector alignment], as spans it is one unit. A span record is the 20-byte `Sector` that exists now (7.3), and in the compact codec an extra flat span costs a handful of bits (7.7). C duplicates the tree overhead per layer.

**6. Lighting.** The cook's shadow rays use the collision's line of sight (`P/light.rs:1-9`), so a slab already blocks light in the bake with spans or with rooms. The editor preview passes no occluder (`D/natural-terrain.md` I2), so it leaks light through every slab in all options; section 4.9 proposes a cheap gate.

**7. Entities.** D keeps position as world `i32` and derives the span from `(cell, y)`. A and C need a room or layer id on every record unless the editor works around it; the old floors proved that workaround costs a bug stream (1.2).

**8. MCP.** With D every tool gets an optional `y` (or `storey` name), and `room` becomes optional (the cell finds it). With A the agent computes `y_offset` arithmetic per call and has to know which room holds which storey (`D/mcp-guide.md:92`).

**9 and 10.** A is free now and the most expensive later: it keeps the container model that T8 to T10 would otherwise absorb. B needs a second pass over the same ~350 sites that T8 touches anyway (grep count of `.floor`/`.ceiling` over `P/`, `psxed-ui`, `psxed-mcp` and `emu/.../editor_preview`: 348 lines in 70 files, of which 242 outside tests; a count, not a classification). D puts spans into T8's leaf port, so the compiler walks those sites once.

### 3.4 Verdict

D. The cheapest honest alternative is A with a Sims-style layer on top (a storey cursor that selects a room); it would make the editor feel better and leave C1 to C3 and C5 in place. The old floors did exactly that and still produced the bug stream of 1.2. I recommend D because the data model changes only at T8, which is where it has to change for the quadtree anyway.

One real cost of D to state plainly: all spans of a column share the leaf's size, because the tree is shared. A hillside with a flat bridge slab over it forces the slab to be replicated in each fine leaf the hill needs. That is bounded by (hill detail) x (bridge area) and compresses well (section 7.7). If measurement shows it matters, a later refinement can give a span group its own tree (a limited form of C) without changing the span semantics.

---

## 4. The recommended model

### 4.1 Data

```rust
/// One open volume of a column: a floor surface, a ceiling surface and the
/// material of the solid directly below the floor.
pub struct GridSpan {
    pub floor: Option<GridSurface>,       // None only on the lowest span (a pit)
    pub ceiling: Option<GridSurface>,     // None only on the highest span (open sky)
    pub wall_material: Option<ResourceId>,// what derived faces of the solid below this span's floor look like
}

/// The leaf value (T8). Today's GridSector, with its floor and ceiling moved into spans[0].
pub struct GridSector {
    pub spans: Vec<GridSpan>,             // bottom to top; len 1 on every level made so far; cap 8 [proposal]
    pub north: Vec<GridWall>, /* east, south, west, nwse, nesw: authored walls, unchanged */
}
```

* **Authored walls stay height-addressed.** `GridWall.heights` are absolute in the room frame, so a wall on the third storey is a wall whose heights lie in that range. No storey id, no span index (`P/surface.rs` `GridWall`, `P/sector.rs`).
* **Disk form negotiates.** A sector with one span is written with today's fields (`floor`, `ceiling`, `wall_material`); extra spans go in `above: Vec<GridSpan>` (serde default, skipped when empty), the same "dense form while uniform" rule `D/level-tools-design.md` 3.6 uses for leaves. So no existing project changes on disk, and no one has to convert anything to load. In memory there is only `spans`, so no code can quietly read "span 0 only".
* **Where today's fields go.** `GridSector.floor/ceiling/wall_material` disappear as public fields; `spans()` replaces them. This is the point: the compiler lists every site that assumed one span (the 348-line grep above) and each must choose. The old floors failed by leaving six walks unconverted (1.2).
* `GridRoom` keeps `y_offset` for legacy and for "lower this whole building"; new rooms write 0 and `validate` warns when two rooms overlap in plan ("merge into spans", section 7.9).
* `GridWorld.marks: Vec<LevelMark { name: String, y: i32 }>` (editor-only, serde default, never cooked) names storeys (4.7).

### 4.2 Invariants (`normalize`, `validate`)

1. Spans are sorted bottom to top and do not interpenetrate: `ceil_i <= floor_{i+1}` at all four corners. Equality (a zero-thickness slab) is allowed.
2. Only span 0 may lack a floor, only the last span may lack a ceiling. A span with neither is deleted. (A floorless span above another span is the same span: it merged.)
3. A span with less than `MIN_HEADROOM` (3 clicks = 60 ru, the 48 ru body plus a margin [proposal]) at every corner is removed, and the solid fill below takes its place. This is the **sliver rule**; it is what makes "raise a floor to the ceiling" and "the first treads of a flight above a walkway" do the obvious thing (4.6, 5.7).
4. At most `MAX_SPANS_PER_CELL = 8` [proposal, 3 bits].
5. Every quadtree rule of T9 applies to every span: a big leaf needs all its spans planar, and four siblings merge only when every span matches the parent (`D/level-tools-design.md` 3.2, 3.5).
6. The slab between span i and i+1 is solid. Its owner for materials is span i+1 (the floor above it). The solid above the highest ceiling is owned by that span.

### 4.3 One resolver

Every consumer asks one question: **which span of this column, at this height?**

```rust
impl GridSector {
    pub fn spans(&self) -> &[GridSpan];
    /// The span a working height y selects: the span with the highest floor at or
    /// below y + reach whose ceiling is above y. `reach` is the step-up, so a
    /// flight's first treads belong to the storey they leave.
    pub fn span_at(&self, y: i32, reach: i32) -> Option<usize>;
    /// Highest floor not above y_limit: the editor twin of the runtime floor query.
    pub fn floor_at(&self, lx: i32, lz: i32, y_limit: i32) -> Option<(usize, i32)>;
}
```

Render, picking, tools, derivation, light bake, cook, walkability and MCP all call these. This is the lesson of `floors-editor-architecture.md` "Option A" (a single chokepoint, so render and pick cannot disagree), applied to the document instead of the scene.

### 4.4 Derived walls across spans

Today's rules (`P/derive.rs:1-28`) are the boundary of the **open set** of two columns along an edge. Define, for a column, the open set as the union of its spans' open intervals `[floor(u), ceiling(u)]`, with `u` along the edge (profiles linear between the two edge corners, as today). For an edge between A and B, a face exists wherever a point is open on one side and solid on the other; it is seen from the open side and owned by the solid side.

| Case | Open set A | Open set B | Face | Owner (material chain) |
| --- | --- | --- | --- | --- |
| R1 riser | `[fA, cA]` | `[fB, cB]`, `fB > fA` | `[fA, fB]` seen from A | B |
| R2 drop | `[fA, cA]` | `[fB, cB]`, `cB < cA` | `[cB, cA]` seen from A | B |
| R3 closure | `[fA, cA]` | no cell, or no span | `[fA, cA]` | A |
| **R4 slab edge (new)** | tall span `[0, 12]` | two spans `[0, 4]` and `[8, 12]` | `[4, 8]` seen from A | the span above the slab (`wall_material` of span at 8) |
| none | open and open at the same height (an archway), pit edge (no floor), equal profiles | | nothing | |

* R1 to R3 above are the single-span cases of the rule; I checked by hand that `solid(A) ∩ open(B)` with `solid = (-inf, f] ∪ [c, inf)` reproduces R1 and R2 (`P/derive.rs:9-16`). A floorless neighbour keeps today's behaviour (no face, `P/derive.rs:18`), which is why a pit edge stays open. A neighbour with no cell is R3 as now.
* **Naive pairing is wrong.** Pairing each span of A with each span of B and applying R1 to R3 double-covers: a hall `[0, 12]` next to a column with spans `[0, 4]` and `[8, 12]` gives R2 `[4, 12]` and R1 `[0, 8]`, overlapping on `[4, 8]` with the same facing (z-fight). The open-set form gives the single `[4, 8]` face. I derived this by hand; the property test in 4.10 must confirm it.
* **Reuse.** The polygon cut for crossing profiles (`between()` in `P/derive.rs`, the "bow tie" cut) runs per candidate pair of intervals; the new work is the interval sweep that decides which pairs and the R4 owner. Diagonal edges stay as `D/natural-terrain.md` N1 designs them (R1d, R2d, void triangle), applied to the span pair that overlaps.
* **Bands and overrides.** Bands attach to a face (a run of collinear segments with one owner) exactly as now. `EdgeKey { x, z, axis, len }` (`P/overrides.rs:47-70`) gains a height key, `y: i16` in clicks, the face's lowest point at the run start, so an override survives inserting a slab elsewhere in the column; if the face moves the override is orphaned and listed, per the existing policy (`D/level-tools-design.md` 4.4, "kept and listed"). `Anchor::Bottom(o)` and `Top(o)` are already storey-relative because they measure from the face; `Abs(y)` stays a world datum.
* **Cost.** Per edge, O(spans of A + spans of B) after sorting; single-span edges take the current code path.

### 4.5 Rooms after the change

A room is: a rectangle of leaves, ambient, fog, band sets, defaults, planes, and the unit the cook may cut. It is no longer the vertical unit. Side-by-side rooms keep the Opening verb (`D/level-tools-design.md` 4.1). Two rooms in the same cell column are a validate warning with a Merge command (7.9).

### 4.6 Edit primitives (pure functions in `P/edit/spans.rs`, thin wrappers everywhere)

| Primitive | Does | Notes |
| --- | --- | --- |
| `create_span(cell, y, headroom)` | add an open span in empty space | refused inside a slab |
| `place_floor(cell, y)` | make a floor at exactly y in the column | if a span's floor is at y: edit it; if y is inside a span: split with a slab (`thickness`, default 1 click [proposal]); if the gap below the new slab is under `MIN_HEADROOM`, no lower span is made (sliver rule: the floor rises instead) |
| `insert_slab(region, y, thickness, mats)` | `place_floor` over a region with a ceiling underneath | the Slab verb, Insert mode |
| `cut_hole(region, which)` | merge span i and i+1 (drop the ceiling of i and the floor of i+1) | `which`: the slab above the cursor, or every slab above it |
| `fill_hole(region, y, thickness)` | the inverse | Slab verb, Fill mode; same as `insert_slab` where the span is whole |
| `shift_above(region, y, dy, move_nodes)` | move every surface at or above y up by dy; walls crossing y stretch; nodes above y move | "make this storey taller or shorter"; the storey list is the UI |
| `remove_span`, `merge_spans`, `normalize_column` | housekeeping | called by every verb before it returns |
| `ensure_headroom(cell, y, h)` | cut the slab above if it would leave under h of headroom over y | the "open above" option of Stairs and Raise |

Everything is absolute-height, so none depends on room offsets. All are undoable by the existing whole-document snapshot (`D/level-tools-design.md` 7.1).

### 4.7 Storeys: derived, named, never required

* **Derived list.** `storeys(world)`: histogram the flat floor heights of all spans (in clicks, weighted by cells); a storey is a peak with at least `K` cells (`K` = 8 cells [proposal]) separated from the next peak by at least `MIN_HEADROOM` plus a slab [proposal]. Stair cells put one or two cells at each in-between height, so they never form a peak. Each storey reports floor height, the headroom of its modal span, cell count and, for a cell, its ordinal. [hypothesis: the tower has four peaks at 0, 12, 24, 36 clicks with 84 to 88 cells each and nothing else above `K`. Experiment: run it on the built level and on the six reference levels, where I expect one storey for Cathedral, Cave, Sewers, West and level_008, two for Dungeon.]
* **Named datums.** `GridWorld.marks` gives a storey a name and pins its height ("Ground", "Gallery"). Marks beat the histogram. They are editor-only and never reach the cook, so they cost no runtime bits.
* **The cursor snaps to a storey**, moved by keys or by clicking a surface (5.1).

### 4.8 Entities

World-space `i32` position, as `D/streaming-design.md` 3.3 requires; `RoomIndex` stays meaningless. The editor's Place tool resolves the pointer to a surface (3D ray hit) or to `span_at(cell, cursor)` in the Top view and writes the Y of that floor plus the node's own offset. At runtime "which storey am I on" is `span_at(cell, y)` on the unit that holds the point. No record stores a storey, so there is no binding to get wrong when a slab is inserted: **a node above a `shift_above` moves with it** (4.6), and one below does not.

### 4.9 Lighting across storeys

* **Cook.** Ambient plus point lights with a shadow ray through the collision's line of sight (`P/light.rs:1-9`). A slab is a floor-slab interval and a ceiling-slab interval in the column, so it occludes. Nothing to change beyond the collision generalisation (7.3). Zero-thickness slabs are two back-to-back slab intervals of `SLAB_THICKNESS` 16 ru each (`G/collision/world.rs:8`), which also closes them to light.
* **Preview.** No occluder is passed (`D/natural-terrain.md` I2), so a lamp on storey 1 lights storey 0 through the slab, and with a tower that is every lamp on every floor. Proposal: gate a light by span connectivity within its radius. A breadth-first search over `(cell, span)` nodes, stepping across edges where the open sets overlap (the same interval test as 4.4), within `radius / S` cells, marks the faces that may be lit. The cost is a few hundred nodes per lamp [estimate, from radius over the 80 ru sector]. This approximates the cook's line of sight (it lets light round a corner that LOS would block; that is a looser gate, not a wrong one) and gets the preview closer to Play without cooking per frame. The exact answer is the host-run collision LOS the cook uses; running it in the preview is a later option.
* **Ambient.** One per room. Per-storey ambient is optional via `LevelMark.ambient: Option<[u8;3]>`, applied by the bake at the face's storey; I recommend not building it until a level needs it (the Dungeon's two rooms differ by 72 versus 74 [measured, `project.ron` of `grid-dungeon`]).

### 4.10 Tests the model owes (added to `D/level-tools-design.md` section 14)

* **Open-set tiling (property).** Random columns on both sides of an edge, flat and sloped: the derived faces plus the open sets tile the edge exactly (area sum), no two faces coincide with the same facing, bow ties cut at the crossing (the band test in section 17 is the template).
* **Primitive round trips.** `insert_slab` then `cut_hole` over the same region restores the document; `shift_above(+d)` then `shift_above(-d)` restores it; `normalize` is idempotent.
* **Collision equality.** For random multi-span columns, the span query equals a brute-force interval scan at every point; a single-span level answers identically before and after the port (all six reference levels, every point of a lattice, every `y_limit`).
* **Resolver consistency.** For random worlds, what the preview draws at the cursor equals what `span_at` returns equals what picking hits.

---

## 5. Editor UX

### 5.1 The storey cursor (working height)

* **State.** `work_y: i32` (grid units), editor view state, not in the document. Shown in a **Level chip** at the top of the viewport: `Level: Gallery  y +12 clicks (240 ru)  [<] [>]`.
* **Moving it.** Alt+Up and Alt+Down step to the next derived or named storey; the Levels panel lists them; `G` then a click on any surface (3D or Top) sets the cursor to that surface's height ("go to this level"); a vertical drag of the chip scrubs it in clicks (Alt for 64, as everywhere). The cursor also **follows an edit**: after a verb that creates or raises a floor, the cursor snaps to the storey of the result and the chip says so.
* **What it means for a tool.** In every cell the tool resolves `span_at(work_y, step_up)` (4.3). In 3D a click uses the surface actually hit, and the cursor moves to it (so 3D never "edits the wrong floor"; the old floors broke exactly here, 1.2).
* **Keys (proposals; the free letters per `D/level-tools-design.md` 7.12 and `D/natural-terrain.md` Break on X are G J M Q Z):** `M` Slab, `G` go to level, `Z` cycle the cutaway, Alt+Up/Down step. Alt is already the fine snap during a drag and a free chord while idle (`D/level-tools-design.md` 7.12 for Alt+W), so the chord does not collide. Needs a conflict check against planes and the natural-terrain keys before landing (10.7).

### 5.2 Top view slice

For each cell the view draws the **active span** (`span_at(work_y, step_up)`) at full strength with its heights in clicks relative to the cursor ("+0", "+3" on a stair). Where there is no active span:

* a span **below** the cursor is drawn as a dim ghost (the Sims floor below), so you see where stairs and holes must line up;
* a span **above** is drawn as a light hatch outline (a slab overhead), so a stairwell or hole reads as a gap in the hatch;
* nothing at all stays empty.

Wall lines are drawn solid when the derived face's height range meets the band `[work_y, work_y + body_height]`, dim otherwise. This replaces the current "all rooms stacked, upper ones translucent" (`editor/crates/psxed-ui/src/grid_view.rs:158-190`). The walkability overlay and the heatmap show the slice's spans only (their data is per `(cell, span)`, section 7.1).

### 5.3 3D cutaway

Modes (Z cycles): **Off** (everything), **Cut** (default), **Ghost**.

* **Cut.** Geometry whose lowest point is at or above the cut plane is not drawn and not pickable; faces that straddle it are clipped to it, so walls look sliced at the plane (the Sims wall cut). The plane is the underside of the first slab above the cursor in the camera target's column, or `work_y + one storey pitch` where there is none. [proposal; the rule must be one function used by the preview and by picking, so the two cannot disagree.]
* **Ghost.** Above-the-plane geometry draws as dim flat colour or wireframe.
* Everything below the cursor draws normally, so the storey below is real context.
* **Implementation shape.** A `y_max` and a clip in the preview's face walk (`emu/crates/frontend/src/editor_preview/grid.rs:26-47` already keeps per-cell `min/max y`), and the same bound in `U/pick.rs`. Polygon clipping by a horizontal plane is what the band pipeline already does for walls (`P/band_geom.rs`).

### 5.4 Levels panel (replaces the room list as the vertical control)

* A list of storeys: name (double click renames, writing a `LevelMark`), height in clicks, cell count, an eye, a lock.
* **`+ Level`**: add a level above the top with the previous pitch as default height, as a named datum and a cursor move. It creates no geometry until a verb draws something.
* **Drag a level's height** = `shift_above` for everything at or above it (4.6), with a preview of how many cells, walls and nodes move. This is the answer to "storey height is not decided once".
* The room list stays below as **Areas** (name, ambient, fog, wall mode, defaults) for footprint-level settings.

### 5.5 Every verb, relative to the storey

| Verb (key) | Resolves | Behaviour in the span model | New chip or option |
| --- | --- | --- | --- |
| Floor (F) | `place_floor(cell, work_y)` per cell | creates a floor at the cursor: in empty space a new span (headroom 4 clicks [proposal, the T5 default `H`]), in a tall span a slab split; cells whose floor is already at the cursor are skipped and counted | Shift sets a height relative to the cursor |
| Ceiling (C) | ceiling of the active span | sets or creates it; refused through the slab above (clamps one click under it) | |
| Raise / Lower (U) | floor of the active span | as today; the ceiling follows; against a span above it clamps at `MIN_HEADROOM` and says so | `Above: Clamp / Push / Merge`: Push lifts the spans above by the same delta, Merge fuses with the span above when the floor passes its slab (this is the "past headroom" case, 5.8) |
| Ramp (A) | active span | as today | `Open above` runs `ensure_headroom` along the ramp |
| Stairs (T) | start level from the floor at path start, cells by `place_floor(cell, y_tread)` | whole-sector treads as today; **each tread is a slab**, undercroft kept only above `MIN_HEADROOM` (sliver rule: the first treads of a flight over a walkway fill solid, later ones float); a flight that would touch a slab above cuts it (`Open above`, on by default) | `Support: Floating (default) / Solid`; `to` a storey name instead of a rise |
| Spiral stairs | a closed ring of cells | `grid_spiral_stairs` (6.2): runs the stairs rule round a ring, one storey per `rise` | Section 5.7 |
| Wall (W) | gap of the active span on that edge | the gap logic (`next_wall_heights`) uses the active span's open interval and the derived faces on that edge; authored walls are absolute | |
| Bands (E) | the face under the pointer | per face, so a trim on storey 2's wall is a different run from storey 1's; `Abs` lines are world datums, `Bottom` and `Top` follow the face | the wall-run Inspector shows the storey |
| Break (X) | region on the active span | floors, ceilings and derived faces of that span only | |
| Polygon | footprint at the cursor | `grid_polygon` writes spans at `y` / the cursor with floor and ceiling heights relative to it | `Storey` target |
| Void | triangle of the active span's floor or ceiling | `GridSurface.void` is per surface, so per span | |
| Slab (M) | region, then a vertical drag | new, 5.6 | modes Insert / Cut / Fill (Tab) |
| Place (O) | surface under the pointer | world-space Y from the floor hit | |
| Plane (N) | unchanged | planes are free, relative to the room's offset (`P/plane.rs`); a gangway across a well is a plane, not a span | |

Rule for refusals, as everywhere: a verb never silently skips; it reports "N cells skipped: inside a slab / no span at this level" in the status line (`D/level-tools-design.md` 7.1).

### 5.6 Slab (M): insert, cut, fill

* **Insert.** Pick a region (marquee, brush, path, or a polygon from the Select tool), press `M`, drag vertically to set the height of the slab's top (snaps to clicks, Alt 64), release. A translucent plane at the height previews the slab, plus the derived slab edges live. Options: thickness (default 1 click), materials (top from the palette; underside and sides default to the span's ceiling and wall materials). Cells whose active span does not contain the height are skipped and counted.
* **Cut.** Tab to Cut: pick a region, click a slab (or press and the one above the cursor is taken), release. The slabs over the region merge with the spans above; the hole's rim appears as R4 faces in the cells around it. Chip: `One slab / Every slab above`.
* **Fill.** The inverse for a region that is currently open through a slab height.
* **Sloped neighbours.** A flat slab at an absolute height meets a sloped floor or ceiling; where the floor is above the slab at a corner the cell is skipped and counted (never clamped silently).
* Undo: one step.

### 5.7 Stairs and spiral stairs that connect storeys

* **Stairs (T)** keeps its gesture (path, Enter, vertical drag). New: *Start level* defaults to the floor at the first path cell and is overridden by the cursor; *To* accepts a storey ("To Gallery") so the rise is `top of that storey - start`; *Open above* cuts the slab where a tread would pass under it.
* **Spiral stairs** is a wrapper, not a new geometry system: given a closed 4-connected ring of cells (picked as a path, or the one-sector-wide inset ring of a polygon), a start cell, a direction, a rise per turn or per storey and a riser, it **walks the ring and applies the stairs rule**: a cell the path crosses straight is a tread (one riser up), a cell where the path turns is a landing (flat at the previous height, the T7 rule in `P/edit/verbs/stairs.rs`, "a turn is a landing"). When the accumulated rise reaches the storey rise the walk records an arrival and continues flat through landings until the next tread cell. On an octagon ring that makes the four chamfer runs landings and the four axis runs flights, so a storey of 12 clicks is two flights and two landings (a half turn), and the stair winds around by itself (section 8.1).
* **Arrival opens onto the floor automatically**: where a landing is at the same height as the neighbouring storey floor there is no riser, so derivation leaves the edge open (no doorway to cut). Where it is one click off, the step-up walks it.
* **Railings** are optional authored low walls on the inner edge (absolute heights), or `grid_wall_override` ghost-solid combinations; see 8.5 for why the overlay may flag drop edges without them.

### 5.8 "Automatic storey detection" (my reading; flagged as an assumption)

The phrase in the brief is ambiguous. I read it as three behaviours and ask Manny to correct me (10.9):

1. **The list is automatic** (4.7): you never declare a storey; the histogram finds it and `G`/Alt+Up/Down use it.
2. **The cursor follows the edit** (5.1): raising, building stairs or drawing a floor that lands in another storey moves the cursor there.
3. **Raising past the headroom is decided by an explicit option, never guessed**: a floor raised until the span's headroom falls under `MIN_HEADROOM` fills solid (sliver rule); one raised through the slab above it is reported ("reaches the Gallery slab") and, with `Above: Merge`, fuses with the span above (equivalent to cutting a hole and keeping the surfaces). Default `Clamp`, because merging by accident throws a slab away.

### 5.9 Picking, hover and overlays

Click cycling in the Top view (plane, then floor, then ceiling, `D/level-tools-design.md` section 17) gains spans: a repeated click walks spans from the cursor's up and down. `FaceRef { room, leaf, target }` gains `span: u8` in the target (`Floor { span }`, `Ceiling { span }`); a selection is transient, so span index shifts on insertion do no harm. Hover highlight and the Inspector header name the storey ("Floor, Gallery span 2 of 3"). The walkability overlay and heatmap are keyed by `(cell, span)` and the slice shows one storey at a time, all storeys listed in the legend (`P/walk.rs`, `P/heat.rs`).

---

## 6. MCP

### 6.1 Addressing

Every grid tool gains two optional parameters and loses a requirement.

* `y` (grid units, absolute) or `storey` (a name or the derived ordinal), the working height. Default: the staged cursor (the MCP keeps one per session, `grid_set_level`), else the only span of a single-span cell.
* `room` becomes optional: the cell finds its room. It stays for adding rooms and for room settings.
* **Ambiguity is an error, not a guess.** A tool that touches a cell with several spans and has no `y` fails with the spans listed ("cell [3,0] has 3 spans: floor 0 to 3, 25 to 47, ...; pass y or storey"). Agents must not guess a storey.
* Heights stay grid units and absolute; the room's `y_offset` is accepted only for legacy rooms. Everything is a wrapper over the same `psxed_project::grid::edit` functions the editor calls (`D/mcp-guide.md` first paragraph).

### 6.2 Tools

| Tool | Kind | Does |
| --- | --- | --- |
| `grid_storeys` | read | derived and named storeys: name, floor y (clicks and ru), modal headroom, cell count, spans per cell histogram, current cursor |
| `grid_column` | read | one cell: every span with floor and ceiling corners, materials, slab thickness between spans, and the derived faces per edge with their owner |
| `grid_slice` | read | ASCII map of one storey for a rectangle (the `grid_heights` view but for `span_at(y)`), with `^` for a slab above and `v` for a span below only |
| `grid_height_at` | read (extended) | now lists every floor in the column, not only the best one under a limit |
| `grid_set_level` | act | set or name the cursor and the marks |
| `grid_insert_slab` | act | `{ rects or polygon, y, thickness?, materials?, ys?: [..] }`; a list of `ys` inserts several slabs in one call |
| `grid_cut_hole` | act | `{ rects or polygon, y?, which: "above" | "all_above" | "between", ... }` |
| `grid_fill_hole` | act | the inverse |
| `grid_shift_above` | act | `{ y, delta, rects?: world, move_nodes?: true }` |
| `grid_add_surface` | act (extended) | `y` creates a span or splits one; reply says which |
| `grid_stairs` | act (extended) | `y`, `to_storey`, `open_above`, `support`; reports treads made solid by the sliver rule |
| `grid_spiral_stairs` | act | `{ ring: { cells: [...] } or { polygon, inset: 1 }, start, direction, y0, storeys, rise_per_storey, riser?, open_above?, support? }` |
| `grid_polygon` (from `D/natural-terrain.md` 2.2) | act | gains `y`, `floor_height`, `ceiling_height`, and writes spans |
| `grid_merge_rooms` | act (migration) | merge a stacked room into a base room as spans (7.9) |
| `grid_walkability`, `grid_heatmap` | read | gain a `storey` filter |

Reply shape: every act tool returns what changed, what snapped, what it skipped and why, and the resulting storey summary for the touched cells, as the existing tools do.

---

## 7. Cook, format, collision and runtime

### 7.1 Document and editor code

* `GridSector` as in 4.1; `GridWorld::floor_height_at(x, z, y_limit)` becomes a column query (`P/world.rs:121`).
* `cell_faces(world, room, x, z)` (`P/faces.rs:346`) returns faces per span plus the derived faces of the cell's open set; face culling bounds are per cell as now and widen to cover all spans (`GridSector::height_range`, `P/sector.rs:178`).
* Walkability (`P/walk.rs`) and the heatmap (`P/heat.rs`) key their per-sector arrays by `(cell, span)`; the reachability flood steps between `(cell, span)` nodes (an edge is walkable between two spans if their floors differ by at most the step-up and their open sets overlap by a body).
* The old per-container resolution functions do not return; there is exactly one (4.3).

### 7.2 Cook faces and portals

* Faces come from the same `cell_faces`, so preview, bake and cook still agree (`P/faces.rs:1-12`).
* `portals::derive` (`P/cook/portals.rs`) loses its stacked pass (`153-175`): stacked content is inside one unit. Side-by-side portals use `open_span` per cell (`portals.rs:30-50`), which becomes a list of open intervals; each overlapping pair across the border is a portal, merged along the border as now.

### 7.3 Collision format and queries

* The 20-byte `Sector` (`floor[4] i16, ceil[4] i16, flags u16, ext u16`, `G/format.rs:298-306`) is already exactly one span. Span 0 stays in `SECTORS`. Cells with more spans set `ext` (the "rare data" link, `format.rs:309`), and `SectorExt` gains `span_first: u16, span_count: u8` (it is full today: `wall_first u16, wall_count u8, plane_count u8, plane_first u16, splitb u16`, so the record grows to 12 bytes, still word aligned). Extra spans live in a new `SPANS` section of 20-byte `Sector`-shaped records. Sector flag bits 0 to 12 are used and 13 to 15 are reserved for the leaf level in T10 (`D/level-tools-design.md` 10.2), so the flag cannot go there. This rides the v4 bump (`D/streaming-design.md` section 4.1 item 3 says new sections should ride v4).
* **Scan.** `scan_slabs` offers the floor and ceiling intervals of the cell's `Sector`; for a cell with `ext` span data it loops its extra spans the same way. `SECTOR_CLEAR` and the flat fast paths in `scan_slabs` are only set for single-span sectors, which are everything today. The loop `for room in 0..rooms.len()` (`scan.rs:176-190`) shrinks to the resident units whose footprint contains the sample (the spatial index item 12 of the streaming checklist already requires).
* **Ray.** The cell walk (`G/collision/ray.rs`) tests each span's slabs as it tests one today; a cell with extra spans costs proportionally more.
* **`locate(hint, p)`.** Unit by footprint, then `span_at(cell, y)`; tie-break by `column_gap` as now (`world.rs:279-370`) but over spans of one cell instead of rooms.
* **Slabs and tunnelling.** The slab intervals are `SLAB_THICKNESS` 16 ru each (`world.rs:8`). A solid between two spans thicker than 32 ru is not fully covered by intervals; that is the same assumption the motor makes for a thick floor today, and a fast body is not modelled to tunnel. Not a regression; mentioned because spans make thick solids common (the tower's slabs are 20 ru).

### 7.4 Runtime consumers

`GridRuntime` activation and drawing work on units, not storeys. Entities carry no room; `D/streaming-design.md` item 7 already forbids reading it. Camera and third-person arm trace through the same provider (`CollisionTraceProvider`, `D/level-format-and-runtime.md` summary 4), so nothing outside `psx-grid` changes.

### 7.5 Render and culling

* Quads are unchanged. The per-cell run (`RCell { first, count }`, `D/level-tools-design.md` 10.1) holds the quads of every span of the cell.
* **Span tag.** `Quad.flags2` uses bits 0 and 1 (`QUAD2_REPEAT`, `QUAD2_MULTI`, `G/format.rs` `QUAD2_KNOWN`); bits 2 to 4 can hold the span ordinal (0 to 7) at no byte cost. v1 does not read it. It exists so a later pass can cull by span.
* **The risk to measure.** A camera under a slab runs the facing test on the upper storey's quads; upward-facing floors fail it, but walls facing the camera pass and are projected and emitted, and with no z-buffer the order of the ordering table decides whether they show through the slab. The same is true of stacked rooms in the portal ring today, so this is existing risk made visible, not a new one. [hypothesis: on the tower, quads of storeys above the camera's slab appear in `quads_visible` and some paint over the underside. Experiment: pose-matched headless frames on the ground platform looking up and sideways, with `GridFrame` counters, before deciding on span culling.] If it bites, the fix is a per-frame span visibility mask: the camera's span plus spans reachable through open sets within the frustum, tested per quad against the tag.
* `RoomHeader.min_y/max_y` are per unit and stay correct (`format.rs:225-245`).

### 7.6 Units and streaming

A unit is one chunk (`D/streaming-design.md` 3.2); the planner cuts by recursive bisection on the 4-sector lattice. All spans of a column travel in the unit that owns the column, so a tower is one unit instead of N room chunks. Open question 10.8: the planner cuts only in XZ in v1; a very tall stack in a large footprint may eventually want a Y cut at a slab, which is the existing vertical portal mechanism (`PORTAL_VERTICAL`, `G/format.rs`). Chunk independence (checklist item 1) holds: spans reference only their own surfaces.

### 7.7 Fewest bits (the standing rule, memory `fewest-bits-rule`)

The dense v4 numbers, then what the compact codec should target. `compact-format.md` owns the final figures; this table is the requirement I hand it.

| Field | Current | Range I expect | Minimum bits | Proposed |
| --- | --- | --- | --- | --- |
| Spans per leaf | none (implicit 1) | 1 to 8 [cap proposal] | 1 flag per leaf, then `count - 1` in 3 bits only when set | adaptive 1 bit (near 0 cost in single-span rooms) |
| Extra span record, dense | n/a | | 20 B `Sector` + 4 to 12 B of ext | 20 B in `SPANS` plus the 12 B `SectorExt` [derived] |
| Extra span floor, compact | n/a | slab thickness 0 to 15 clicks | 4 bits | delta to the previous span's ceiling, coded with the west neighbour's value as context |
| Extra span ceiling, compact | n/a | headroom 3 to 63 clicks | 6 bits | delta to its own floor, same context |
| Span flags (sloped, split, walkable) | 16 bits | mostly equal to the neighbour's | 0 to 2 bits | predicted from the neighbour |
| Quad span tag | n/a | 0 to 7 | 3 bits | in `flags2` bits 2 to 4, no cost |
| `FaceRef.span` (editor only) | n/a | | not shipped | |
| `LevelMark` | n/a | | not shipped | editor only |

R3000 unpack cost for the dense form: one more `Sector`-sized fetch per extra span per sample [derived from `scan_slabs`]. For compact decode the extra spans are a loop of two small deltas [estimate; the codec's decode counters, `examples/grid_compact` `Ops`, are the measurement]. A flat slab spanning a hall costs one extra span per cell, which the codec predicts from the west neighbour and the cell above in the quadtree (a 4 sector flat slab leaf is one record, 20 B + 12 B for 16 cells in the dense form [derived]).

### 7.8 How it slots into T8 to T10 so the model changes once

| Phase (`D/level-tools-design.md` section 13) | What changes for spans |
| --- | --- |
| T1 to T7 (built) | nothing in the data. Verbs already work on "the floor" and become "the active span's floor" with the resolver defaulting to span 0 in every existing level |
| **T8 Leaf storage** | **the leaf value is `GridSector { spans, walls }` with the disk-form negotiation of 4.1; the port of tools and tests converts every `.floor/.ceiling` site (about 350 lines, 70 files)** to `spans()`/`span_at`. Acceptance unchanged ("every existing test passes unmodified", plus byte-identical round trip of the reference projects) |
| T9 Quadtree operations | `normalize` merges and subdivides over all spans; stitching and hanging corners per span surface; derived walls use the open-set rule (4.4) when a column has more than one span |
| **T10 Leaf cook, format v4** | `SPANS` section, `SectorExt` growth, the `scan_*`/ray/`locate` generalisation, portals per interval, quad span tag. One bump, with the items T10 already takes from the streaming list (`D/streaming-design.md` 4.1 items 3, 15, 17) |
| T11 Sculpt, fine Stairs | sculpt acts on the active span |
| T12 Room, Corridor | write spans at the cursor |
| T13 Opening, Stamps | stamps copy a column of spans with heights relative to the anchor floor; openings per face as now |
| T14, T15 optimise | the cook merge and node LOD operate per span run; the span visibility mask of 7.5 lands here if measured necessary |

If T8 has already started without spans, insert the span port before T9 (a "T8b"): it touches the same sites and should not be done as a second pass later.

### 7.9 Migration

* **Existing projects.** Load and cook as before: one span per sector, disk form unchanged. `validate` adds a warning `RoomsOverlapInPlan` with a "Merge as spans" button; nothing converts on load (`D/level-tools-design.md` open question 2, "never change a level behind your back").
* **Dungeon.** Room 0 at origin (-4, -10), 7 by 19, y 0, ambient 72; room 1 at origin (-7, -10), 10 by 17, `y_offset` 2048 gu, ambient 74 [measured, `editor/projects/grid-dungeon/project.ron`], 45 overlapping plan cells [measured, `D/other-maps-survey.md:58`]. `merge_rooms(base, upper)`: shift the upper room's heights by its offset difference, grow the base room to hold the union, append each upper span to the cell's list in order, keep authored walls (heights shifted; the "wall-only sectors" simply have no span), report cells where floors and ceilings would interpenetrate (clip and say), keep the base room's ambient and fog and report the other's.
* **Gate (correctness).** For the merged Dungeon: the multiset of cooked quad positions, materials and corner colours equals the pre-merge cook (the Dungeon is `Authored`, so no derived faces appear); `floor_height_at` at every point of a lattice and every `y_limit` equals the old answer; Play walks the old tape route (`D/other-maps-survey.md` route table) to the same player path within the rounding the cook already has. Portals differ, deliberately (the 45 + 45 stacked portals go away).

### 7.10 Performance (labels, no invented cycle counts)

| Item | Estimate or measurement plan |
| --- | --- |
| Single-span cells | the same code path; check `collision cycles per tick` on Cathedral and Dungeon route tapes before and after the T8 port (acceptance: unchanged within noise) |
| Multi-span collision | one more interval offer per extra span per sample [derived from `scan_slabs`]; **measured on the tower route** against the same geometry built as stacked rooms where that is possible, and against the Dungeon before/after merge |
| Collision, stacked rooms vs spans | the room loop shrinks from O(rooms) to O(resident units covering the point); the Dungeon has two rooms, so the visible gain there is small, the tower with four to six storeys is the case |
| Render | content-driven: each slab is a top quad, an underside quad and edge quads (R4) where neighbours open; the cook's 4x4 horizontal merge applies per span. Report quads before and after |
| Editor | `all_faces` of 1662 faces took 0.26 ms [measured, section 18]; the open-set sweep adds a sort per edge; report the same figure on the tower |
| Memory | 20 B per extra span plus 12 B ext [derived]; the tower's count is in 8.2 |

---

## 8. The test level: `grid-tower`

### 8.1 Geometry

Scale 5/4 (a new project's default). Cells are world sectors `[x, z]`, north is -Z.

* **Footprint: an octagon in a 12 by 12 square, chamfer 3.** The 12 outside cells are `(0,0) (1,0) (0,1)`, `(11,0) (10,0) (11,1)`, `(11,11) (10,11) (11,10)`, `(0,11) (1,11) (0,10)`. The 12 half cells (cut along a 45 degree diagonal) are NW `(0,2) (1,1) (2,0)` (diagonal NE-SW), NE `(9,0) (10,1) (11,2)` (NW-SE), SE `(11,9) (10,10) (9,11)` (NE-SW), SW `(0,9) (1,10) (2,11)` (NW-SE). That leaves 132 cells [derived: 144 - 12]. The four axis walls are 6 sectors long (cells x or z in 3 to 8), the four diagonal walls are 3 sectors long in x and in z.
* **Storey pitch 12 clicks = 3072 gu = 240 ru** (3 sectors) [proposal]. Floors at 0, 12, 24 and 36 clicks. Slab thickness 1 click (20 ru), so each platform span is 11 clicks (220 ru) tall. Roof (top ceiling) at 48 clicks = 960 ru.
* **The stair ring is the outer layer of cells, one sector wide**, a closed 4-connected walk of 44 cells [derived: 4 sides x 6 + 4 chamfers x 5]:

  ```
  N side   (3,0) to (8,0)             6 treads
  NE       (9,0) (9,1) (10,1) (10,2) (11,2)     5 landings (every cell is a turn)
  E side   (11,3) to (11,8)           6 treads
  SE       (11,9) (10,9) (10,10) (9,10) (9,11)  5 landings
  S side   (8,11) to (3,11)           6 treads
  SW       (2,11) (2,10) (1,10) (1,9) (0,9)     5 landings
  W side   (0,8) to (0,3)             6 treads
  NW       (0,2) (1,2) (1,1) (2,1) (2,0)        5 landings
  ```
  Clockwise from `(3,0)`. Under the T7 rule (turn cells are landings, first and last are treads, `P/edit/verbs/stairs.rs` header) the chamfers are flat landings and the axis runs are flights without any marking by hand. 12 treads per storey at a one-click riser consume exactly two axis runs, so each storey is a half turn: storey 0 climbs the N and E sides to the SE landing at 12, storey 1 the S and W sides to the NW landing at 24, storey 2 the N and E sides again to the SE landing at 36. [derived]
* **Platforms**: the 88 interior cells that are not ring cells [derived: 100 cells of the 10 by 10 inner square minus 12 ring cells that fall inside it]. As rectangles of cells `[x0, z0, x1, z1]` inclusive: `[3,1,8,1]`, `[2,2,9,2]`, `[1,3,10,8]`, `[2,9,9,9]`, `[3,10,8,10]` (6 + 8 + 60 + 8 + 6 = 88). Each storey floor spans all of them; there is no hole at ground.
* **Central well**: cells `[5,5,6,6]` (4 cells), open through the slabs at 12, 24 and 36, so one tall span from the ground floor to the roof there.
* **Spans per column [derived from this layout]**: platform cell outside the well: 4 spans (ground, 12, 24, 36), 84 cells = 336; well cells: 1 span, 4 cells; ring: N, NE, E and SE cells carry the ground span (where a tread is at least 4 clicks up: below that the sliver rule fills the base solid), flight 0 or its landing, and flight 2 or its landing; S, SW, W and NW cells carry the ground span and flight 1 or its landing. 107 ring spans, **about 447 spans in 132 cells, at most 4 per cell**. The cook report is the authority; these are for expectation only.

### 8.2 Height table (clicks relative to ground; ru = clicks x 20)

| Run | Cells | Heights |
| --- | --- | --- |
| Ground (interior, S/SW/W/NW ring) | | 0 |
| Flight 0 | N `(3..8, 0)` | 1, 2, 3, 4, 5, 6 |
| Landing | NE chamfer | 6 |
| Flight 0 cont. | E `(11, 3..8)` | 7, 8, 9, 10, 11, 12 |
| Arrival 1 (platform 1) | SE chamfer | 12 |
| Flight 1 | S `(8..3, 11)` | 13, ..., 18 |
| Landing | SW chamfer | 18 |
| Flight 1 cont. | W `(0, 8..3)` | 19, ..., 24 |
| Arrival 2 (platform 2) | NW chamfer | 24 |
| Flight 2 | N `(3..8, 0)` | 25, ..., 30 |
| Landing | NE chamfer | 30 |
| Flight 2 cont. | E `(11, 3..8)` | 31, ..., 36 |
| Arrival 3 (platform 3) | SE chamfer | 36 |

One click is a 20 ru riser, half the step-up of 40 ru (`WalkLimits`), and a tread is one sector (80 ru): a slope of 1 in 4 [derived]. Each flight's underside is the ceiling of the span below it, so the ground walkway under flights 1 and 2 has a ceiling that rises with the stair. Two flights over the same ring cell are 24 clicks (480 ru) apart [derived].

### 8.3 Build, step by step, with the proposed tools

Assumes the section 6 tools. All heights absolute, in clicks unless marked gu (x 256).

1. `psxed-mcp --new editor/projects/grid-tower` (the New Grid Project starter).
2. `grid_polygon { kind: "room", points: <octagon: (3,0) (9,0) (12,3) (12,9) (9,12) (3,12) (0,9) (0,3) in sector lattice corners>, y: 0, floor_height: 0, ceiling_height: 48 clicks (12288 gu), chamfer: "exact" }`. One tall hall, 132 cells, one span each, derived closure walls all round, void triangles on the 12 half cells (needs `D/natural-terrain.md` N1; today see 8.4 for the fallback).
3. `grid_insert_slab { rects: [[3,1,8,1],[2,2,9,2],[1,3,10,8],[2,9,9,9],[3,10,8,10]], ys: [12, 24, 36], thickness: 1 }`. The hall's span splits in the platform cells into 4 spans; the ring is still a tall hall.
4. `grid_cut_hole { rects: [[5,5,6,6]], y: 12, which: "all_above" }`. The well merges the three slabs' spans.
5. `grid_spiral_stairs { ring: { cells: <the 44 ring cells above, in order> }, start: [3,0], direction: "cw", y0: 0, storeys: 3, rise_per_storey: 12, riser: "click", open_above: true }`. One call; or the equivalent explicit calls: three `grid_stairs` (each with `y` and a 12-tread path of the two axis runs and the chamfer between) and three `grid_add_surface` for the arrival landings at 12, 24 and 36.
6. Materials: `grid_paint` stone on floors, a darker material on slab undersides and the roof, `grid_set_room` default wall material, `grid_band_cut` a trim at `bottom + 1 click` as the room default band set so every storey's walls get the same base.
7. Lights: `add_light` per storey on the platform, one at the top of the well. Check by `grid_walkability` and a screenshot that the preview gate of 4.9 keeps storey lamps out of the other storeys.
8. The player: `place_node` at platform 0, cell `[3,3]`, floor height read by `grid_height_at` (a spawn is world-space).
9. Optional: railings along the inner edge of each flight (authored walls, `facing both`, heights `[tread, tread + 2 clicks]`), and windows once the Opening verb exists (`D/level-tools-design.md` 7.9).
10. `grid_budget`, `cook_check`, then the verification list below.

Calls: 6 act calls reach a walkable, spawnable tower with the generator (steps 1 to 5 and 8), 11 without it (three `grid_stairs` and three `grid_add_surface` instead of the one spiral call); materials and lights come on top [counted from the list above].

### 8.4 What you can build today with rooms (baseline for comparison, not a recommendation)

Section 1.3's recipe applied to the same tower, as a list of what the author must get right up front:

* one `grid_add_room` per storey at `y_offset` 0, 3072, 6144 and 9216 gu, each with its own floor from `grid_add_surface`;
* for each lower room a ceiling at exactly the height of the next room's floor, or the portal never appears (the bonnie-32 rule, in `P/cook/portals.rs:153` the stacked portal needs a missing floor or ceiling on one side);
* the central well as a floor hole in rooms 1 to 3 plus a missing ceiling in rooms 0 to 2;
* a flight that climbs from one storey to the next lives in the room it starts in, so that room's ceiling over the flight must be raised above its top, and the shared landing cell must exist in exactly one of the two rooms (else two coincident floors);
* a cell holds one floor, so the second pass over the N and E sides (flight 2) and the ground walkway under flights 1 and 2 cannot share a room with the flights below them;
* the chamfers need authored diagonal walls on full-floor cells, so the hidden half floor stays (the Cathedral's way, `D/natural-terrain.md` 1.2);
* every room needs its own closure and a hand-opened seam at each arrival (`grid_wall_override`, `D/mcp-guide.md` "An opening in a wall").

None of it can be moved afterwards without re-aligning the neighbours. I have not built this variant. Build it as the A baseline only if the comparison needs measured chunk bytes and collision cycles; otherwise the argument in 3.3 stands.

### 8.5 Acceptance

Correctness first, as in `D/level-tools-design.md` section 13; performance is measured and reported, never a gate.

1. **Document.** `grid_column [6,0]` shows 3 spans with the heights of 8.2 (ground at 0 with a ceiling at 3, the flight 0 tread at 4, the flight 2 tread at 28); `grid_storeys` lists four storeys at 0, 12, 24, 36 clicks and no other peaks; `validate` has no errors and no `RoomsOverlapInPlan`.
2. **Derived faces.** The open-set property test of 4.10 is green; on the built level there are no two coincident faces with the same facing, the slab edges around the well (R4) are present on every storey, and the platform edges seen from the ring match the table of 4.4.
3. **Cook.** `cook_check` has no errors; one unit; the report lists spans, extra-span bytes, quads and collision bytes.
4. **Walkability overlay clean.** `grid_walkability` reports no orange (steep), no blue (low headroom), no red (step too tall) and no hatched unreachable floor from the spawn: the flood reaches ring, all four platforms and every tread. **Yellow drop edges** along the inner edge of flights and around the well are expected without railings (a walkable floor ending in nothing, `D/editor-guide.md` "Walkability overlay"); with the optional railings of step 9 there are none. Record which of the two was run.
5. **Headless Play route.** A poll-bound tape from the spawn: north to `(3,0)`, east along the N flight, round the NE landing, south along the E flight, through the SE landing (it opens onto platform 1 with no riser), along the S and W flights, through the NW landing, up the N and E flights again, and onto platform 3. The route legs are written from the cell path and the polls tuned with `track_player` (about 8 s of wall time per 1000 polls, `D/mcp-guide.md`). **Pass**: the tracked path's maximum Y reaches the platform 3 floor (36 clicks = 720 ru = 11520 editor units [derived]), the player ends within the octagon on platform 3, the minimum Y never drops below the ground floor except in a separate fall test through the well, and the camera never ends inside geometry.
6. **Screenshots** (`screenshot`, preview renderer, shown before anything is called done): from inside on each of the four platforms looking across and up the well; from the ring looking along a flight; from the ground looking up the well; from outside, three orbits at yaw 0, 90 and 225. Outside views need exterior-facing walls (10.6). Cutaway: one shot per storey with the cut on, to show the Sims-style slice. Frames are shown to Manny with the report.
7. **Edits after the fact** (the point of the exercise), each undone and redone exactly: insert a fifth slab at 18 clicks over the platform (a mezzanine inside storey 1), cut a 3 by 3 hole in it, then `grid_shift_above` the whole of storeys 2 and 3 by 4 clicks and re-run the route (the stairs must be rebuilt by the generator with the new pitch: record that as expected, not a failure).
8. **Perf.** Report, for the route tape: world cycles p50 / p90 / max, collision cycles per tick, `quads_visible` and `robust_quads`, cook bytes per section and span count, `GridFrame` counters, and the heatmap model's worst spot against them. Also report the camera clearance in the ring (10.5). The 7.5 pose-matched look-up frame is part of this step.

### 8.6 Risks specific to the test level

* **Camera clearance.** The ring is one sector (80 ru) wide and the third-person arm is longer than that in a typical project (Dungeon's camera `distance` is 2000 editor units = 125 ru [measured from the project file, derived]). The arm will shorten on walls and may sit in a ceiling. If the route looks cramped, widen the ring to 2 sectors (a 16 by 16 octagon) rather than changing the camera.
* **Whole-sector treads** (T7) mean 6 sectors of run for a 6 click rise; fine treads wait for T11.
* **Preview shadows**: the preview lets lamps light through slabs until the 4.9 gate exists; judge the lighting from the cook's bake (`playtest` frames), not the preview.

---

## 9. Phased plan

Every phase is sized for one agent run [estimate], leaves `make lint` clean, adds the tests of 4.10, and shows frames before it is called done. Acceptance is correctness; measurements are reported.

| # | Phase | Delivers | Acceptance | Measure |
| --- | --- | --- | --- | --- |
| MS0 | This design | agreement on 10.1 to 10.9 | Manny's answers | none |
| MS1 | Spans in the document (rides T8) | `GridSpan`, disk-form negotiation, port of every `.floor/.ceiling` site, `span_at`, `storeys()` read, `RoomsOverlapInPlan` | every existing test passes; reference projects round-trip as documents; cook sections byte-identical | editor frame time; `collision cycles per tick` unchanged |
| MS2 | Open-set derivation and primitives | R4 and the interval sweep in `derive.rs`, `faces_derived.rs`; `edit/spans.rs` primitives; `normalize_column`; sliver rule; validate | property tests of 4.10; Convert (T1) still preserves look on single-span rooms | face counts on the reference levels (expected unchanged) |
| MS3 | Editor: cursor, slice, cutaway, Levels panel, Slab verb | 5.1 to 5.6 and 5.9; picking and preview share one cut | headless gestures per verb (the `ViewportHarness` style); render equals pick at the cursor | gesture frame time on a 450-span level |
| MS4 | MCP | section 6 tools, ambiguity errors, `tests/grid_tools.rs` cases | each tool once over stdio; the 8.3 build runs | calls per level |
| MS5 | Cook, format v4 spans, collision, runtime | `SPANS`, `SectorExt`, scan/ray/`locate`, portals per interval, quad tag | collision equality tests of 4.10; headless Play on the tower and the merged Dungeon | collision cycles per tick, bytes per section |
| MS6 | Migration | `grid_merge_rooms` and the Rooms panel button | the Dungeon gate of 7.9 | cook size before and after |
| MS7 | Spiral stairs and the test level | `grid_spiral_stairs`, `grid-tower` project, acceptance 8.5 | all of 8.5 | 8.5 item 8 |
| MS8 | Light gate and, if measured necessary, span culling | 4.9 gate in the preview; 7.5 mask | pose-matched frames; no light through a slab in the preview | quads and cycles on the look-up frame |

Dependencies: MS1 with or just before T8; MS5 with T10's format bump (and told to the `grid/compact` and `grid/natural` lines now, since both touch `Sector` and the faces code); MS3 to MS4 can run in parallel after MS2. The natural-terrain N1 (void triangles, diagonal derivation) is a prerequisite for the clean octagon of 8.3 step 2; without it use the authored-diagonal-wall fallback and accept the hidden half floors the Cathedral has.

What each phase does to code other lines are changing: `grid/natural` edits `derive.rs`, `faces.rs`, `slope.rs`, `surface.rs`, `walk.rs` (uncommitted, in `grid-runtime`); MS2 changes the same files, so MS2 should start after N1 lands or be merged by the owner of N1. `grid/compact` reads the cooked tables (`examples/grid_compact/tables.rs`) and has no `spans` yet; hand it 7.3 and 7.7.

---

## 10. Open questions for Manny (each with my recommendation)

1. **Spans in the leaf, or a Sims-style layer over rooms?** Recommend spans (D). The layer is cheaper this week and leaves C1 to C3 and C5 standing, the way the old floors did.
2. **Constants.** Slab thickness 1 click (20 ru), minimum headroom 3 clicks (60 ru, body 48 plus a margin), default headroom of a new span 4 clicks, span cap 8, storey pitch of the test level 12 clicks. Recommend: take them, expose them as `limits::` constants, and allow zero-thickness slabs.
3. **`GridSector.floor/ceiling` disappear as public fields.** Recommend yes: it is what forces the 350-site port to be complete once.
4. **Keep `GridRoom.y_offset`?** Recommend keep for legacy and write 0 for new rooms, with the overlap warning.
5. **Camera clearance in a one-sector ring.** Recommend building the tower at ring width 1, looking at the frames, and widening only if it is cramped.
6. **Exterior view of derived walls.** Derived faces have no facing field (`P/derive.rs`, `DerivedWall`), so they are one-sided toward the room and the tower is hollow from outside. Authored walls have `facing` (`P/surface.rs:184-191`). Recommend a `RoomDefaults.closure_facing: Front | Both` (default `Front`, no change to any level), cooked as a double-sided quad.
7. **Keys:** `M` Slab, `G` go to level, `Z` cutaway, Alt+Up/Down. Recommend approve after a conflict check with the planes tool (N) and Break (X).
8. **Unit planner: XZ cuts only in v1?** Recommend yes; a Y cut at a slab is possible later through the vertical portal.
9. **"Automatic storey detection"** as read in 5.8 (derived list, cursor follows edits, raising past headroom by explicit option). Recommend confirm; this is an assumption I recorded rather than asked.
10. **Preview light gate** by span connectivity (4.9) now, host LOS later. Recommend yes.
11. **Support for stairs.** Default floating with the sliver rule (undercroft only where tall enough), `solid` as an option. Recommend floating: one rule, no per-flight bookkeeping, and it keeps the ground walkway usable under flights.
12. **Per-storey ambient** (`LevelMark.ambient`). Recommend not now.

---

## 11. Smells noticed, not fixed here

* `U/panels.rs:477` hard-codes the new room offset as `top + 3 * SECTOR_UNITS`; the Rooms panel `Raise` field and the MCP `grid_add_room` default 0 disagree with it.
* `U/pick.rs:343-365` `top_room_at` picks by a heuristic (active room if it has geometry, else the highest visible); with spans this is replaced by `span_at` at the cursor.
* The preview has no shadow occluder (`D/natural-terrain.md` I2) and so lights through floors; harmless with one storey, visible with several.
* `GridWorld::floor_height_at` and the runtime `locate`, `scan_column`, ray walk and `contents_at` all loop every room (`P/world.rs:121`, `G/collision/world.rs:317,~380`, `scan.rs:176`, `ray.rs:157,211`): the streaming checklist already lists it; spans make it more urgent, not less.
* `P/cook/portals.rs:153-175` stacked-room portals are an O(cells x rooms) pass with a Y chosen from `heights[0]` of the first surface it finds, which is wrong for sloped surfaces.
* `DerivedWall.edge` is documented "always a cardinal edge" in a comment, not in the type (`D/natural-terrain.md` 1.3 item 10).
* `D/mcp-guide.md:92` ("A second room at another height") becomes misleading once spans exist and must be rewritten with MS4.
* "348 lines in 70 files" is a grep of `.floor`/`.ceiling`, which also matches non-sector fields; the real port list is the compiler's, not mine.

## Decisions (2026-10-17, coordinator on Manny's standing instruction to go with the recommendations)

- Model: spans in the column (hybrid, spans in the T8 leaf). Rooms stay as footprint, ambient, fog and streaming grouping.
- Constants as proposed: slab 1 click, minimum headroom 3 clicks, span cap 8, storey pitch 12 clicks.
- Add `RoomDefaults.closure_facing` so derived exterior walls can render from outside (the tower must not look hollow).
- Camera ring clearance: keep a one-sector ring; widen only if frames look cramped.
- Keys: `Q` Slab (`M` went to Rough in N2), `Z` cutaway, Alt+Up/Down storey cursor, `G` go to level. `G` was reserved for Vault in natural-terrain.md; Vault moves to `J`.
- Storey detection: a derived list, a cursor that follows edits, and raising past headroom creating a new span only via an explicit option (as the study assumed); Manny may revise.
- Sequencing: spans land with the T8 leaf port (the one data-model change), derivation R4 and open-set pairing after natural-terrain N1 merges, format via the v4 bump shared with T10 and the compact-format C phases.
