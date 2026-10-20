# Natural terrain and architecture on the grid (2026-10-17)

> **N2 (Rough) was removed on 2026-10-20** (tool, `rough` data, the render jitter and tint, `grid_rough`, Break's rough level); sections that describe it are history, and projects that still carry `rough` data load and drop it on save. N1 (diagonal geometry), N4 (outlines) and N3 (Break v1) are built on `grid/natural`; what landed and the deviations are in `level-tools-design.md` section 21. The text below is the original audit and design.

Status: audit plus design addendum to `level-tools-design.md`. Written read-only against branch `grid/main` (HEAD c581c6c7) and, for T5 to T7, against the uncommitted `grid/tools` worktree (`grid-runtime`, merge 581de480 plus staged Stairs and heatmap files). No cargo, no builds, nothing was run; every "OK" below is from reading code and the Cathedral data, not from a test I executed.

Manny's direction (the contract): "I'll need to create environments which look as natural as possible, so no straight walls, mix between different slopes of different sizes, being able to do diagonals (which are what make the cathedral so nice looking), be sure to include all of this into the MCP server and into the engine." Plus two later requirements: a **Break** verb (quickly add detail by auto-breaking a raised wall or a floor, "like in the cathedral where I created those manually") and **architecture** generators (arches, vaults).

How to read numbers. **[measured]** came from a file or a run named next to it. **[derived]** is arithmetic on measured values or struct sizes. **[estimate]** is reasoning with no run behind it. **[proposal]** is a chosen constant with no measurement behind it. Units: at the default scale 5/4 a sector is 1024 grid units (gu), 1280 editor units, 80 runtime units (ru); a click is 256 gu = 20 ru; the fine step is 64 gu = 5 ru; one ru is 16 editor units (`units.rs:6-10`, `units.rs:110-111`, `cook/collision.rs` tests use 20 ru per click). Path prefixes: `P/` = `editor/crates/psxed-project/src/grid/`, `U/` = `editor/crates/psxed-ui/src/grid_tools/`, `M/` = `editor/crates/psxed-mcp/src/`, `G/` = `engine/crates/psx-grid/src/`, `D/` = `docs/grid/`.

---

## 1. Audit

### 1.1 Capability table

Cells: OK (works as read), PART (works with a stated limit), MISSING (no code), BROKEN (code exists and gives a wrong or unreachable result). Notes follow the table.

| # | Capability | Document | Editor tools | Preview | MCP | Cook | Runtime render | Collision: motor | Collision: camera, LOS, shadow rays |
|---|---|---|---|---|---|---|---|---|---|
| A | Diagonal split per cell, independent triangle heights (`heights_b`) | OK [A1] | MISSING to create [A2] | OK [A3] | MISSING to create, PART to edit [A4] | OK [A5] | OK | OK [A6] | OK [A6] |
| B | Diagonal walls, authored (NW-SE, NE-SW) | OK [B1] | OK [B2] | OK [B3] | OK [B4] | OK [B5] | OK | OK, bounding span [B6] | OK, exact [B7] |
| C | Diagonal walls DERIVED (triangles of a split cell differ; cells meeting along a diagonal) | MISSING [C1] | MISSING | MISSING | MISSING | MISSING | n/a | n/a | n/a |
| D | Sloped floors and ceilings, any corner combination | OK [D1] | OK (Height tool; Raise and Ramp on grid/tools) | OK | OK (`grid_set_heights`; `grid_raise`, `grid_ramp` on grid/tools) | OK, refuses corner spread over 4096 ru [D2] | OK | OK, walkable up to normal.y 2867/4096 [D3] | OK |
| E | Mixed slope sizes across cell sizes | MISSING (cells are one sector) [E1] | MISSING | MISSING | MISSING | MISSING | MISSING | MISSING | MISSING |
| F | Slanted wall tops and bottoms | OK [F1] | OK (wall tool follows slopes; Height tool drags wall corners) | OK | PART, no direct wall-height tool [F2] | OK | OK | PART, bounding span [B6] | OK, exact [B7] |
| G | Free planes at any angle | OK [G1] | OK | OK | OK (`grid_add_plane`) | OK | OK | PART, three shapes only [G2] | OK |
| H | Sector-granular derived cliffs (R1 risers) as solid | n/a | n/a | OK | n/a | PART, no collision record [H1] | OK | OK by slab and step rule | BROKEN, rays pass under tall cliffs [H1] |
| I | Per-corner light variation, AO | OK, per-corner colours exist [I1] | n/a | PART, no shadows [I2] | MISSING | OK, shadows from runtime LOS | OK, Gouraud | n/a | n/a |

Notes, with where I read it.

**A1.** `GridSurface.heights_b: Option<[i32;4]>` is the second triangle's own four corner heights (`P/surface.rs:85`), `heights_for(tri)` returns them (`P/surface.rs:129-135`). A shared diagonal vertex can therefore sit at two different heights, one per triangle: a step along the diagonal.

**A2.** Nothing in non-test code ever writes `heights_b`. A grep over `editor/` and `engine/` finds `heights_b = Some(..)` only in tests and in the clipboard transform (`P/edit/clipboard.rs:115-121`). The Height tool, Split tool (`U/face_tools.rs:267`, flips the diagonal only) and the inspector (`U/panels.rs:243-298`) read it. The Cathedral's seven split-level cells exist because they were imported from bonnie-32 (`D/bonnie32-editor-inventory.md:67`). On `grid/tools` it is no better: `auto_split.rs:39` skips split-level surfaces and Ramp "becomes one plane" (design doc section 19). So Manny's central look has no way to be authored in this editor.

**A3.** `P/faces.rs:296-319`: planar, single-material surfaces are one quad; anything else is two triangles at `heights_for(index)`.

**A4.** `grid_set_heights` has `layer: 1` (`M/grid/build.rs:180-182`) but refuses when the cell "has no second triangle layer" (`M/grid/build.rs:240-244`). `grid_set_split` flips the diagonal. No tool makes the layer.

**A5.** `P/cook/collision.rs:148-181` writes `SplitB` (`G/format.rs:354`) and `SECTOR_SPLIT_LEVEL`; render comes from the same faces.

**A6.** `G/collision/world.rs:129-155` picks the corner set by triangle, `scan.rs:328-361` evaluates each footprint sample on its own triangle, `ray.rs:589-617` cuts a ray piece where it crosses a triangle diagonal so each piece is linear. A step along the diagonal is handled by the same slab and step rule as a step between cells (motor step up 40 ru: `engine/crates/psx-engine/src/character_motor.rs:27`, lift-and-settle at `:1599-1640`).

**B1.** `WallEdge::{Nwse,Nesw}` and per-edge `Vec<GridWall>` (`P/sector.rs:8-55`, `P/sector.rs:84-104`). A diagonal wall is `GridWall.heights = [bottom a, bottom b, top b, top a]` like any wall (`P/surface.rs`), front direction `[1,-1]` / `[-1,-1]`.

**B2.** Wall tool, R cycles auto, N, E, S, W, NW-SE, NE-SW (`U/mod.rs:189-202`, `U/help.rs:39`).

**B3.** `P/faces.rs:323-360` uses the diagonal length (`faces.rs:336`) for texture `u`, normals are the diagonal's (`faces.rs` test `diagonal_walls_have_diagonal_normals`).

**B4.** `grid_place_wall` accepts `nwse` and `nesw` (`M/grid/mod.rs:124-134`, `M/grid/walls.rs:31-35,104`; "for a diagonal the 45 degree line").

**B5.** `P/cook/collision.rs:49-58,215-231` emits a `ColWall` with edge id 4 or 5 for every solid authored wall.

**B6.** `circle_hits_wall` handles the diagonals with a point-to-segment test (`G/collision/world.rs:416-451`) and `wall_code` gives diagonal normals (`world.rs:483`). But the vertical extent a body sees is `wall_span`, the lowest bottom to the highest top of the piece (`world.rs:408-412`, used at `scan.rs` `scan_walls`). A piece with a sloped bottom (an arch spandrel, an overhang) therefore blocks from its LOWEST bottom along its whole length. Passable only if that lowest bottom is at least a body height (48 ru, `WalkLimits.body_height`) above the feet.

**B7.** `ray.rs:480-497` has the diagonal geometry and `ray.rs:504-509` interpolates bottom and top along the piece, so rays, the camera arm and shadow rays see the true slanted polygon.

**C1.** `derive::physical_edge_walls` returns `Vec::new()` for a diagonal edge (`P/derive.rs:356-358`). `DerivedWall.edge` is documented "always a cardinal edge" (`P/derive.rs:68-69`), `EdgeKey.axis` is only X or Z (`P/overrides.rs:47-70`), so a diagonal face could not even be addressed by an override, a material or a band set. Consequence: where a split-level cell has differing diagonal heights, the derived pipeline draws a hole between the triangles (see through to the void below). The Cathedral covers this by hand: each of its 7 split-level cells carries an authored diagonal wall `[0,0,256,256]` or `[-256,-256,256,256]` (census in 1.2). In a Derived room those stay authored and additive; `Convert` (`D/level-tools-design.md` 4.7) matches only cardinal edges so it would keep them, and a future derived diagonal riser would then draw on top of them (z-fight), which `duplicates_derived` (`P/derive.rs:533-562`, cardinal only) cannot see.

**D1.** Four independent corner heights plus a diagonal (`P/surface.rs:68-85,149`); a non-planar quad folds along its split. Heights live on the 64 gu lattice, so the gentlest slope over a whole sector is 64/1024 (about 3.6 degrees [derived]) and the steepest walkable is about 1.02 : 1 (`D/level-tools-design.md` 7.5).

**D2.** `MAX_CORNER_DELTA = 4096` ru (`P/cook/collision.rs:30-32`). 

**D3.** `MIN_WALKABLE_NORMAL_Y_Q12 = 2867` (`P/cook/collision.rs:28`, motor `character_motor.rs:38`), about 45.6 degrees [derived].

**E1.** The quadtree is designed (`D/level-tools-design.md` 3.2, phases T8 to T11) and not built. Today a slope can change direction only at sector boundaries (80 ru), so a rock wall cannot have a 1/4-sector knuckle.

**F1.** `GridWall.heights` (`P/surface.rs`); derived rings are triangles or quads (`P/derive.rs:216-249`). The Cathedral has 40 cardinal wall records with a slanted top or bottom (census).

**F2.** The MCP can place a wall that fills a gap (`M/grid/walls.rs`) and override a derived one, but nothing sets an authored wall's four heights. The editor can (Height tool on a wall corner, `P/edit/refs.rs:301-307`).

**G1.** `GridPlane` has integer yaw, pitch, roll in degrees, any size from 64 gu to 8 sectors, material, sidedness, collision class (`P/plane.rs`). `grid_add_plane` takes degrees (`M/tools/planes.rs`). The editor snaps rotation to 15 degrees (`P/plane.rs` `ROTATION_STEP`).

**G2.** A plane cooks to Platform (normal.y at or above 0.7), Wall (at or below 0.35) or Blocker, a box around the plane, for the leans in between (`P/plane.rs` `PlaneShape`, `D/level-format-and-runtime.md:699`). A leaning rib, an arch voussoir or a buttress at 45 degrees collides as a box. At most 64 planes per room (`P/limits.rs:33`, with the comment that the renderer tests every plane box each frame) and a plane is always one rectangle.

**H1.** Derived risers and drops get no collision record (`DerivedWall::blocks`, `P/derive.rs:97-103`; `cook/collision.rs:232-252`). Bodies are handled by the floor slab: 16 ru thick (`G/collision/world.rs:8`), forbidden feet interval `(top - 16 - h, top)` (`scan.rs` `offer_flat`). Rays and the camera arm test the same thin slab (`ray.rs:536-560,631-680`). A tall cliff face (taller than slab plus body, about 64 ru with the test body of 48 ru [derived]) therefore leaves a ray free to travel BELOW the high floor, inside the rock: the third-person camera can swing into a cliff, line of sight passes through it, and the cook's shadow rays leak light through it. The motor is probably still stopped, because `trace_supporting_floor` finds no floor under the feet in the rock cell (`character_motor.rs:1520-1560`), but I did not run it: the existing test only covers a 60 ru riser (`G/collision/tests.rs:615-650`), just under that threshold. Cheap experiment for whoever owns collision: add a 200 ru riser case to that test and a camera-arm case. `WALL_FRONT_ONLY` exists in the format (`G/format.rs:334-337`) but nothing reads it.

**I1.** Per-corner baked colours are in the format (`Quad.color: [u32;4]`, `G/format.rs:518-535`) and the bake is per corner with shadows from the runtime LOS (`P/light.rs:1-9`).

**I2.** The preview passes no occluder (`bake_face_corners` is "without shadows", `P/light.rs:146-151`), so anything that depends on LOS (shadows, AO) differs between the preview, hence between MCP `screenshot`, and Play.

### 1.2 What the Cathedral reference level uses

`editor/projects/grid-cathedral/project.ron`, one room 33 by 26 sectors, an Authored room (no `wall_mode`, no band sets, no planes). Counted by parsing the RON; the numbers are exact for this file.

* 720 sectors, 676 with a floor, 6 with a ceiling (all six sloped, the vault lead-ins).
* **Diagonal walls: 37 records in 22 cells** (20 NW-SE, 17 NE-SW). 30 of them are 15 pairs of stacked pieces `[768,768,5888,5888]` over `[256,256,768,768]` (or the 512 variant) at the corners of the nave and the apse: **corner chamfers**, a tall diagonal wall that cuts the corner of a cell. The other 7 are **diagonal risers** `[0,0,256,256]` / `[-256,-256,256,256]` inside the 7 split-level cells.
* **Split-level cells (`heights_b`): 7**, always one triangle at one level and the other one or two clicks away (256 and 0, 0 and 256, 256 and -256, -256 and 256). 5 floors use the NE-SW split, 677 NW-SE. 4 more cells have per-triangle materials (`tri_b`, checkered 230/234 alternation).
* **The broken-tile pattern**, read from the height map around (31..32, 4..6): a plateau edge is cut at 45 degrees by a chain of cells that alternate between a chamfer cell (tall diagonal wall, the rock side) and a split-level cell (one triangle on the plateau, one a click or two below, a low diagonal riser between them). The low riser is 256 or 512 gu (20 or 40 ru), which a body steps up (the step is 40 ru). Around (23,5), (7,6), (7,17), (23,18) it is a single bite at a plateau corner.
* Bent floors: 5 non-planar cells and a 2 by 2 dip at (23..24, 15..16) with corners at -256 (a click-deep bowl folded along the NW-SE diagonal), a single raised corner at (5,14). 27 floors have unequal corners; 12 of those are the one-click ramp `[0,-256,-256,0]`.
* Arches: the nave arcade is a polyline of **one wall piece per sector**. Along the column x=1, z=10..13 the lintel piece bottoms run 3840, 5120, 5632, 5120, 3840 (gu) up to a flat top of 5888: a four-segment pointed or round arch, 4 sectors wide, springing at 3840 gu (300 ru) and crowning at 5632 gu (1792 gu = 7 clicks of rise [derived]). Column x=0 repeats it at lower values (3584 to 4608). Each piece is one `GridWall` with a sloped bottom, no jambs, no soffit thickness. That is the bar to beat: one segment per sector, slope steps of +1280, +512, -512, -1280 gu per sector.
* Walls: 114 north, 52 east, 113 south, 31 west, 347 records in all with the diagonals.

What the runtime collision would get wrong for this level (all by reading, not by running):

1. The arcade lintel pieces collide by bounding span (B6). Fine here, springing is 300 ru high against a 48 ru body, but a generator that springs lower (a doorway of 2 clicks, 40 ru) is not passable under the crown either, because the piece blocks from its lowest end.
2. The seven diagonal risers are ordinary solid walls 20 or 40 ru tall; the motor's step-up (lift 40 ru, sweep, settle, `character_motor.rs:1599-1640`) clears a 20 or 40 ru wall, so they are climbable, but only because they are at most the step height. A diagonal riser of 3 clicks would block both the body and, since the wall is solid, would be the only thing stopping rays.
3. Chamfer cells keep a full floor under the hidden half of the cell (the rock side). The floor draws (hidden behind the diagonal wall, wasted primitives) and exists in collision; nothing walkable reaches it because the wall blocks, so this is a render cost, not a collision error.
4. Nothing in the Cathedral relies on derived cliffs, so H1 does not bite it.

### 1.3 Broken or missing, ranked

1. **No tool creates a split-level surface** (A2, A4). Manny's look cannot be authored.
2. **Derived diagonal walls do not exist** (C1). Even with item 1 fixed, a split-level cell in a Derived room has a hole along its diagonal.
3. **No half-cell void.** An outline that follows a 45 degree line cannot be expressed without leaving the hidden half-floor the Cathedral leaves (1.2 point 3). The old PSoXide grid had `dropped_corner` for this (`D/old-grid-system-archaeology.md:102`).
4. **Tall derived cliffs are not solid for rays** (H1). Caves and canyons are exactly the levels with tall derived cliffs.
5. **Mixed slope sizes need the quadtree** (E1), phases T8 to T11.
6. **MCP gaps:** no wall height tool (F2), no smart-paint-rule setter (`M/` has no `rules`; `PaintRules` is Rooms panel only), no way to create a split-level cell, no outline or polygon fill, no break, no jitter, no stamps, no vault, no arch cut.
7. **Preview has no shadows** (I2), so AO or shadow-based looks cannot be judged through `screenshot`.
8. **Plane cap 64 per room** (G2) is too small for decoration scatter or spanning ribs, and plane collision for leaning pieces is a box.
9. **Body collision of sloped-bottom walls is the bounding span** (B6), a hard limit for low arches.
10. Smell, not fixed here: `WALL_FRONT_ONLY` and `ColWall.reserved` are in the format and read by nothing (`G/format.rs:334-348`), and `DerivedWall.edge` carries a "cardinal only" invariant in a doc comment, not in the type (`P/derive.rs:68`).

---

## 2. What makes grid levels look natural, and how to get it

Eight techniques, plus the Break verb (section 3) that composes them. Each stays on the grid, writes ordinary data wherever it can, and has an MCP tool. Fields per item: data model, gesture, MCP, cook, runtime render, collision, perf.

### 2.1 N1: Diagonal-aware derivation (diagonal risers, half-cell void, diagonal closure)

The foundation for everything else. Without it Manny's diagonals are hand-placed walls; with it a diagonal edge is the product of heights, like every other edge.

**Rules** (extend `P/derive.rs`, same `between()` polygon cut, `P/derive.rs:219-249`):

* **R1d diagonal riser.** In a cell with a split-level surface, take both triangles' heights at the two diagonal end corners (corner indices `[0,2]` for NW-SE, `[1,3]` for NE-SW, `edge_corner_indices`, `P/edit/walls.rs:30-39`). Profiles `A(u)`, `B(u)` along the diagonal, `u` normalised 0..1024 and emitted at diagonal length `1024 * sqrt 2` (`faces.rs:336`). Higher triangle owns, the lower triangle's side hosts (the face is seen from the low side, as R1). Bow ties cut at the crossing exactly as today. Equal profiles make nothing, so a split-level cell whose triangles meet at the diagonal (a pure fold) grows no face.
* **R2d diagonal drop** for ceilings, same rule.
* **Void triangle.** New `GridSurface.void: u8` (bit 0 triangle A, bit 1 triangle B; serde default 0, skipped when 0). A void triangle draws no floor or ceiling face. A void floor triangle next to a floor triangle yields **R3d diagonal closure**: the sector closes itself along the diagonal from floor profile to ceiling profile (or `open_wall`), solid by default. Across a cardinal edge, `side_of` (`P/derive.rs:185-200`) reports the edge's triangle as `space = false` when it is void, so the neighbour's R3 closure fires normally.
* **Override addressing.** `Axis` gains `NwSe`, `NeSw` (`P/overrides.rs:47-70`); `EdgeKey` for a diagonal is the cell origin in clicks plus that axis. `DerivedWall.edge` may then be a diagonal.

**Data model.** One field (`void`), one enum variant pair, no change to `GridSector` walls. Legacy documents unchanged (all defaults). `duplicates_derived` and `Convert` learn the diagonals: `PreserveLook` converts an authored diagonal wall that coincides with a derived diagonal face into an `EdgeOverride`, so the Cathedral converts to Derived without doubling its 7 risers.

**Editor.** No new verb. Split-level creation is a Height-tool gesture: Alt-drag on one triangle of a cell (the cell shows its two triangles in the overlay, `U/overlay.rs:438-451` already draws the diagonal) creates `heights_b` from `heights` and then moves one triangle's corners. Void: the Split tool (D) gains Shift-click on a triangle to void or restore it. Raise and Lower get a "cut corner" chip that voids the triangle of every region cell that sits on a convex corner of the region (the N4 outline tool uses the same function).

**MCP.** `grid_crack { room, cells, kind: floor|ceiling, triangle: a|b, delta | height, snap }`: creates `heights_b` as a copy if absent, then offsets triangle `b` (so `grid_set_heights layer 1` works afterwards). `grid_void { room, cells, kind, triangle: a|b|none }`. `grid_set_wall { room, cell, edge, index, heights[4] | top_delta | bottom_delta }` (closes F2). Reads: `grid_sector` and `grid_walls` list derived diagonal faces and void triangles.

**Cook.** Derived diagonal faces go through `derived_wall` (`P/faces_derived.rs:22-131`) with `point(u)` already generic in the host edge's endpoints; the `u_grid` match (`faces_derived.rs:45-53`) needs the diagonal arm `faces.rs:336-350` has. Collision: nothing new for risers (slab and step rule, A6). R3d writes a `ColWall` with edge id 4 or 5 (`cook/collision.rs:49-58` already maps them), the same record an authored diagonal wall makes. The void triangle's floor stays in collision (harmless: the closure wall stops bodies, and the floor is unreachable); no format change.

**Runtime render.** Fewer primitives than the Cathedral's hand-made version, because a void triangle draws nothing where the Cathedral draws a full hidden floor [derived from 1.2 point 3]. A merged planar floor quad becomes two triangles plus one riser quad: +2 primitives per cracked cell [derived, faces.rs:296-319]. At the fitted 741 cycles per visible quad (`D/level-tools-design.md` section 20, Cathedral route, R2 0.996 [measured]) that is about 1.5k cycles per cracked cell in view [estimate]. A chamfer cell with a void triangle saves one primitive.

**Collision.** Motor, camera and rays are already correct for diagonals (A6, B6, B7). Add the H1 fix in 2.10.

### 2.2 N4: Organic outlines (Room and Corridor with 45 degree corners, auto-diagonalise)

Today a room is a union of axis-aligned rectangles, so every cave outline is a staircase. The grid already has half of the answer (diagonal walls); N1 gives the other half (void triangles).

**Algorithm (auto-diagonalise).** Input: a set of cells (the Region), output: the same set where every convex staircase corner is replaced by a diagonal. For every cell `c` in the set whose four-neighbourhood has exactly two adjacent cardinal neighbours missing (an outer corner), void the triangle of `c` on the outside of the diagonal through `c`. For a run of such corners along a staircase of unit steps the voids line up into one 45 degree line. Concave corners (an inner staircase step) fill the missing neighbour cell's inside triangle: the neighbour gets a floor with its far triangle void, so the line continues. Larger steps (2 by 1) stay staircases unless the user chooses "smooth": 1/2 slopes need the quadtree (section 2.3).

**Data.** No new fields beyond `void` (2.1). The outline tool writes cells, floors, ceilings and voids; R3 and R3d close it, so the outline is a derived wall.

**Editor.** Region source gains a **lasso** (click points, Enter closes, polygon rasterised at cell centres, 45 degree snap by holding Shift). Room (B) and Corridor (K) gain a chip "Diagonals": run the algorithm on the footprint after writing it. A Top-view handle on an outline corner toggles that corner cell between square and chamfered.

**MCP.** `grid_polygon { room, points: [[x,z]...], kind: room|corridor|region, floor_height, ceiling_height?, chamfer: none|corners|smooth, width? }`: rasterise, write floors and ceilings (one call), then run auto-diagonalise. `grid_chamfer { room, cells, mode }` applies only the corner step to existing cells. Both report the cell count, voided triangles, and the new face count.

**Cook, render, collision.** As 2.1. Perf: a 45 degree wall over `n` cells is `n` diagonal closure quads plus `n` cut floors; same count as a staircase outline of `n` cells (2 faces per cell) [derived], so organic outlines are about free.

### 2.3 N7: Mixing slope sizes (quadtree plus Sculpt, and what works before it)

Manny wants "different slopes of different sizes". Real slope-size variety is exactly what the quadtree buys: a 1/4-sector leaf can turn a slope direction every 20 ru, a 4-sector leaf is one clean plane.

**Before the quadtree** (usable now, no new phase): a slope is a sector-wide plane; mix sizes by choosing different corner heights per cell and letting auto split (grid/tools T5, `auto_split.rs`) fold along the level diagonal. Add a **Slope brush** gesture on top of Raise: drag across a region, each cell gets a slope direction and rise chosen by a noise field (seed, scale in cells, max rise), writing ordinary corners. MCP: `grid_slope_field { room, cells, seed, scale, max_rise, snap }` (a Sculpt precursor).

**With the quadtree** (T9 to T11, `D/level-tools-design.md` 7.4): Sculpt gains the modes the design already lists (Raise, Lower, Flatten, Smooth, Noise) plus **Erode** (each point moves toward the mean of its welded neighbours in proportion to its local slope, a thermal-erosion step, integer, deterministic by iteration count) and **Cliff** (snap a height field's steepest gradient lines to a vertical step, so walls become derived faces instead of steep slopes). Sculpt subdivides only where the surface is not exactly representable, so slope detail costs leaves only where it exists.

One design conflict to resolve (open question 1): `D/level-tools-design.md` 3.2 says leaves below level 2 carry no diagonal walls and no `heights_b`. Cracked diagonal cells and void triangles at 1/4 resolution are precisely the smallest natural detail, so I recommend lifting that rule for levels 2, 3, 4 and keeping big leaves (levels 0 and 1) planar.

**MCP.** `grid_sculpt { room, cells | brush | path, mode: raise|lower|flatten|smooth|noise|erode|cliff, radius, strength, seed, target: floor|ceiling|both, max_leaves }` returning leaves added, bytes, walkability delta.

**Cook, collision.** As design 10.2: leaves, sub-blocks, sector index. Perf: design 10.5 gives about 35 KB of collision for a fully fine 10 by 10 hill [`D/level-tools-design.md` 10.5, derived]; Sculpt shows the live count and caps at `MAX_LEAVES_PER_ROOM`.

### 2.4 N2: Render-only vertex jitter ("Rough")

Turns straight walls into rock without touching collision. Deterministic per vertex, identical in the preview and the cook because it is applied at face construction in grid units.

**Where it is applied.** In `faces.rs` when a face's corners are built (`Corner.p` and `Corner.g`, `P/faces.rs:184`, the `corner` closures at `faces.rs:276-282,347-355` and `faces_derived.rs:82-89`), before the UV splitter (`uv_split::split`, called from `cook/render.rs:296`) and before the cook snaps to ru (`cook/render.rs:317-324`). Offsets are integer gu; the preview and the cook therefore see the same displaced polygon and `rt()` rounds both the same way.

**Field.** `D(x, z) = (dx, dz)` from an integer hash of `(room.rough.seed, gx, gz)` on the **click lattice** (256 gu), range `[-A, A]` each, scaled by a weight `w(x,z)` that is the highest `rough` level (0 to 3) among the up to four sectors that touch the lattice point. Because the field is a function of the plan position only (not of the height), a vertical edge shifts rigidly, so every T-vertex that a band stitch inserts on a shared vertical edge (`P/faces_bands.rs:138`) stays on the displaced line and **the surface stays watertight by construction**. Floors, ceilings and walls that meet at a lattice point all move together. Mid-edge points on horizontal edges (UV-splitter cuts, quadtree hanging corners) take the linear interpolation of the two end points, applied before the split, the same constraint the design gives hanging corner heights (`D/level-tools-design.md` 3.2). A vertical component is not part of v1: it would put the floor render off the collision floor.

**Safe maximum offset.** Collision stays on the grid, so the visible rock is up to `A` from the surface the body stops against. Inward jitter (rock toward the player) clips the character if it exceeds the clearance between the character's visual extent and its collision radius; outward jitter makes an invisible wall of the same size. Facts: the default character collision radius is 192 editor units = 12 ru (`psxed-project/src/resource_types.rs:2291`, matches the motor test's 12, `G/collision/tests.rs:588-590`); a fine step is 5 ru; the preview near plane is 8 ru (`D/level-tools-design.md` section 20). I do not know the visual half-width of the Aletha model. **Proposal: `A_max = R / 3 = 4 ru` (51 gu at 5/4, under one fine step), default amplitude levels 1, 2, 3 = 1, 2, 4 ru.** Verify with a headless frame census: the player pressed into a jittered wall, measure covered pixels vs the unjittered frame; if the model fits inside R/2 the cap could go to R/2 = 6 ru. The camera: inward jitter can put the near plane behind a rock vertex; the camera arm's clearance to the grid wall is a world setting I did not read, so keep `A_max` below it (open question 3). `limits::validate` warns `RoughTooLarge` when `A` in ru exceeds R/3 of the player character.

Floor slope error: displacing a floor vertex by `A` in plan on a slope of up to 1.02 : 1 changes the visible height by up to `1.02 * A` = about 4 ru at the cap [derived], at most the same size as the offset, so it is inside the wall tolerance. Default `rough` is applied to walls and ceilings; floors only when the region says so.

**Normals and culling.** The cook already computes each quad's plane from the snapped ring (`cook/render.rs:372-395`) and takes the `QUAD_AXIS` path only if exactly two components are zero, so a displaced wall automatically falls to the general Q7 normal and plane constant. Cull bounds (`RCell` min/max y) are unaffected because `dy = 0`; plan bounds widen by `A`.

**Lighting.** The bake runs on the displaced face, so each quad's normal differs and `FacingShade` (`P/light.rs:47-120`, blends by the squared normal) and Lambert give per-facet variation for free: a jittered wall reads faceted without any colour data.

**Data.** `GridRoom.rough: Rough { seed: u32, amp_gu: [u16; 3] }` and `GridSector.rough: u8` (0 none, 1 to 3), both serde-defaulted (levels never change behind the author's back). Painted by region.

**Editor.** Tool "Rough" under the Paint family: region, then drag = level, `Shift` = floors too. Preview shows it live (faces carry it).

**MCP.** `grid_rough { room, cells | brush | path, level: 0..3, seed?, targets: [walls, ceilings, floors], amp_gu? }`.

**Cook.** Zero extra work beyond a hash per face corner [estimate: negligible against the cook]. The vertex weld (`cook/render.rs` `RoomBuild.vertex_of`) is by rounded position, so shared lattice points weld as before; the vertex count does not change.

**Runtime render.** No extra primitives, no format change. The only cycle effect I can name is losing the `QUAD_AXIS` cheap facing test on displaced axis-aligned quads (it skips a dot product, `G/format.rs:518-535` comment); the size of that cost is not measured. Report with the first slice (hardware-test step per the standing rule, not a bespoke A/B disc).

**Collision.** Unchanged, by definition.

### 2.5 N5: Jagged ceilings and caves

A cave ceiling is a floor flipped: the same machinery. No new data model.

* Ceilings use `GridSurface` with the same corners, diagonal and `heights_b` (collision stores `ceil_b` in `SplitB`, `P/cook/collision.rs:182-211`), so cracked and bent ceilings are free once N1 lands.
* **Ceiling Break** (section 3) tilts, drops and cracks ceiling tiles within a minimum headroom (the keep-walkable mask uses `body_height` from `WalkLimits`).
* **Stalactite stubs:** a ceiling cell whose corners are lowered to a point (three corners at the base, one low) draws a pyramid; the folded quad makes a downward spike. Collision: a solid ceiling blocks the head only where `ceil - floor < body_height`, so spikes above head height cost nothing.
* **Caves as drop rooms:** a room with ceilings below `open_wall` heights (R3 closure from floor to ceiling, `P/derive.rs:399-415`) makes closed caves; without ceilings, canyons with sky and an `open_wall` rim whose per-edge variation is the Break verb's `top_delta` (section 3.3).
* MCP: all through `grid_set_heights kind ceiling`, `grid_crack kind ceiling`, `grid_break targets [ceiling]`, `grid_sculpt target ceiling`.

Perf: a ceiling quad costs what a floor quad costs; a deeply jagged ceiling that is never merged is one primitive per cell, bounded by the visible cells [derived]. Indoor caves are cheaper than canyons because the portal and cell culling already hide what is behind.

### 2.6 N6: Slope-driven and height-driven material rules

Exists in part: `PaintRules.floor_by_class [flat, slope, steep]`, `trim`, `wall` on `grid/tools` (`P/paint_rules.rs:15-40` in the grid-runtime worktree), and bands with absolute anchors on `grid/main` (`D/level-tools-design.md` section 17, `grid_band_cut` abs). Gaps: the MCP cannot set `PaintRules` at all, and nothing reacts to height or to wall steepness.

* **Slope class for walls:** derived faces are vertical by construction, so the cliff look comes from `wall` plus bands. Add `PaintRules.wall_by_height: Vec<(i32 /*gu from room base*/, ResourceId)>`, expanded by the verb into the room's default band set with `Anchor::Abs` lines (existing machinery), so strata are bands, not a second system. Floors: `floor_by_height: Vec<(i32, ResourceId)>` (scree low, rock high), applied where the verb moves a floor and `paint_locked` is false (the same rule as the class rules).
* **Slope with Rough:** class rules run on the real triangle normals the cook uses (`slope.rs` reads `triangle_normal_q12`), so cracks and tilts from Break re-class automatically.
* MCP: `grid_paint_rules { room, floor_by_class?, floor_by_height?, wall?, wall_by_height?, trim? }` and a `reapply: true` flag that re-runs the rules over a region.
* Cook, render, collision: none (materials only). Perf: more materials per level raise `MaterialSet` slots (max 255, `cook/render.rs:106-110`) and texture page pressure, not cycles [estimate].

### 2.7 N3: Per-vertex colour variation and baked ambient occlusion

The format already stores four baked colours per quad (`G/format.rs:518-535`), so this costs no size.

* **Noise tint:** the bake multiplies each corner by `1 + n(x,z)` with the same click-lattice hash as Rough, amplitude per room. Deterministic, preview-equal if the preview applies it (it can: it is a function of position).
* **AO:** per corner, cast `k` short rays on the face's hemisphere with the runtime `line_of_sight` the bake already uses for shadows (`P/light.rs:1-9`), occlusion = hit fraction scaled into the corner colour. Cost at cook time `k` rays per corner [proposal k = 4 to 8]. No cost at runtime.
* Parity problem: the preview has no occluder (I2), so AO and shadows are Play-only. Fix before leaning on AO: let the preview build the same collision tables the cook builds (`cook/collision.rs::build`, 0.37 ms class cost for the walkability analysis on the Cathedral per design section 19; the collision build time is not measured) and pass `line_of_sight` as the occluder, cached against the document revision like the walkability overlay. Until then `screenshot` cannot show AO.
* Interaction with H1: shadow and AO rays leak through tall derived cliffs until the fix in 2.10 lands.
* MCP: `grid_bake_look { room, tint_noise_pct?, ao_rays?, ao_strength? }` (room defaults, stored as `RoomDefaults.bake`).

### 2.8 N8: Decoration through free planes and scatter

Rocks, roots, foliage cards, rubble. The plane system exists (G1) but its cap is 64 per room and each plane box is tested per frame (`P/limits.rs:30-33`), so hand-placed planes cannot be a scatter system.

* **Hand-placed (exists):** `grid_add_plane` for hero pieces (a fallen beam, a banner, a bridge). Rotation any integer degree; textured with alpha-cut or translucent materials (the camera passes through those, `D/level-format-and-runtime.md:699`).
* **Scatter (new):** `GridRoom.scatter: Vec<ScatterRule>` where a rule is `{ region (cells), material, count or density, size range, slope range, height range, yaw jitter, lean jitter, seed }`. Expanded deterministically in `faces.rs` into decor quads (a new `GridFaceKind::Decor`), each two-sided (`QUAD_DOUBLE`), no collision. The cook writes them as ordinary `Quad`s in the render cells (NOT `PlaneRef`s), so the normal cell culling applies and the 64-plane cap does not. Placement samples the floor height with the shared `psx_grid::height` function so cards sit on slopes.
* **Cost:** one primitive per card at about 741 cycles when visible [fit], so 100 visible cards is about 74k cycles, 12 percent of the world budget (1,128,960 minus 528,000 reserve, `D/level-tools-design.md` section 20) [derived]. Two-sided quads skip the facing test, so the card is not cheaper. The scatter report states visible-card count per hot cell and the heatmap includes them.
* **Collision:** none, or `solid` on rubble that should block (then it is a plane or a box obstacle, not a card).
* MCP: `grid_scatter { room, region, material, density | count, size: [min,max], slope: [min,max], yaw_jitter, lean_deg, seed, collision?: none }`, `grid_scatter_clear { room, region }`.

### 2.9 N9: Walls that are not straight (summary of what composes)

"No straight walls" is the sum of: derived walls following a bent floor (exists), cracks and voids (N1), diagonal chains (N4), Rough (N2), Break wall tops (section 3), band trims that follow slopes (T4, exists), scatter at the foot (N8). None needs a wall record per segment.

### 2.10 C1: Collision fixes the natural look depends on

1. **Solid cliffs for rays and camera.** Cook an extra `ColWall` for every derived riser whose height exceeds the step-up (`WalkLimits.step_up`, 40 ru) plus a margin, marked `WALL_FRONT_ONLY`, and make `scan_walls` and `ray_walls` honour that flag (it is defined and unread today, `G/format.rs:334-337`). Front-only means it stops a body or ray that approaches from the low side and lets one on the ledge side pass, so walking off a ledge is not blocked. Cost: 12 B per wall (`ColWall`, `G/format.rs:343-348`), one flag test per candidate wall in the scan [estimate: small, but this is in the motor's 1 KiB-stack hot path, so measure with the collision route tapes]. Where `wall_count` would exceed 255 per sector (`cook/collision.rs:276-277`) it is refused with the cell named.
2. **Document the bounding-span rule** and give the Opening verb partial-span pieces (T10, design 10.2, `ColWall.reserved`) so an arch lintel made of several pieces per edge blocks per piece.
3. **Experiment** (low risk, whoever has cargo): 200 ru riser in `G/collision/tests.rs` for the body and a camera-arm case, to confirm or refute H1 for the motor.

---

## 3. Break (X): the quick-detail verb

Manny: "imagine we're raising a wall, we should be able to auto-break it so we can build more natural-looking environments, same for the floor where we could have some broken tiles (like in the cathedral where I created those manually), the idea is to quickly add details and iterate on levels."

### 3.1 Principles

* A first-class verb with the standard shape (`D/level-tools-design.md` 7.1): region, verb, drag an amount (the **intensity**), commit. `begin` snapshots the touched cells and their neighbours, `apply(intensity, seed, options)` restores the snapshot and recomputes from absolutes, so a drag can be repeated, reversed or cancelled exactly (the contract in `P/edit/verbs/mod.rs:42-80` on `grid/tools`). One undo step.
* **Writes ordinary grid data** only: corner heights, `heights_b`, `void`, `EdgeOverride`, Openings (once T13 lands) and Rough levels. Nothing about the break is stored; the seed lives in the gesture, not the document. After commit it is hand-editable like anything else.
* **Seeded and re-rollable.** Every random decision is a pure function `hash(seed, world cell, op id, corner)`; it never consumes a sequence. Same seed means the same result regardless of region shape or cell order, widening the region does not reshuffle the interior, and raising intensity only adds operations (the threshold test is `hash < p(intensity)`, monotone). Re-roll draws a new seed and re-applies to the same snapshot.
* **Keep-walkable mask.** After generating, the verb runs the walkability analysis (`walk::analyse`, `walk.rs` on `grid/tools`, which reads the motor's own numbers through `WalkLimits`, design section 19) on the touched cells plus a one-ring. Any operation that makes a previously reachable cell unreachable, adds a `StepTooTall` edge (a step above `step_up`), makes a walkable triangle steeper than normal.y 2867, or drops headroom below the body is rejected and retried locally with a smaller amplitude (up to `K` tries [proposal K = 4]), then skipped and counted. The report lists rejects.

### 3.2 Parameters (editor option row and MCP are the same struct)

| Parameter | Meaning | Default |
| --- | --- | --- |
| `region` | marquee, brush, path or lasso; MCP: `cells` rectangle, `brush {centre, radius}`, `path {points, width}`, `polygon` | current Region |
| `targets` | any of `walls`, `floor`, `ceiling` | walls, floor |
| `intensity` | 0 to 100: probability and amplitude scale. The editor drag amount | 40 [proposal] |
| `seed` | u32; `reroll` draws the next | random per gesture |
| `amplitude` | largest vertical change in fine steps (64 gu); never above the walkable limit when `keep_walkable` | 4 (one click) [proposal, matches Manny's cathedral] |
| `keep_walkable` | the mask above | on |
| `spawn` | node ids that anchor reachability; default all player spawns | all |
| `snap` | `fine` (64 gu, default) or `click` | fine |
| `materials` | `mix`: swap per-triangle materials on a fraction of cracked tiles (the Cathedral's checkered `tri_b`) | off |
| `rubble` | scatter rubble cards at the foot of broken walls and on cracked floors (N8) | off |
| `jitter` | raise `rough` on touched sectors to this level (N2) | 0 |

### 3.3 What it does

**Walls** (the wall is the face the region's raised cells show, derived or authored):

| Operation | Data it writes |
| --- | --- |
| Crumbled top | the higher floor's edge corners drop by 0 to `amplitude` fine steps, varying per cell, so a derived riser's top edge follows (the lip of the plateau tilts with it). In a closure with a ceiling the ceiling corners move instead |
| Tilt corners | one end of a segment's top changes more than the other: a slanted top, from corner heights |
| Stub | with probability `p`, a segment's top drops by 1 to 3 clicks, bounded by the step rule |
| Missing chunk, hole | `EdgeOverride.hidden` on a segment today (one whole sector, coarse); a partial-width `Opening` (T13) later, down to 1/4 sector with the quadtree. Holes are see-through only if the cell behind has space, else they show the rock face behind: the verb checks and says so |
| Open-top wall variation | a closure with no ceiling gets its top from the room-level `open_wall` (`P/derive.rs:404-410`), so it cannot vary per cell today. New `EdgeOverride.top_delta: [i32; 2]` (ends in gu, serde default none) adds a signed offset to the top at each end of the segment. Smallest data change that makes open-top walls breakable |
| Render roughness | optional `rough` level (2.4) on the touched cells |

**Floors and ceilings:**

| Operation | Data it writes |
| --- | --- |
| Raise or sink | a cell's four corners move together by a fraction of a click (fine steps), shared edges welded to the neighbour only when `link` is set |
| Tilt | one or two corners move: a bent or tilted tile; auto split chooses the fold (`auto_split.rs`) |
| Crack along a diagonal (the Cathedral bite) | creates `heights_b` and offsets one triangle by `amplitude`, so a derived diagonal riser (N1) appears between the halves. Chosen only where a cell sits on a height boundary or, at lower probability, anywhere. The diagonal runs along the contour (auto split rule) so the break reads as the plateau's edge chipping, as in the Cathedral at (31..32, 4..6) |
| Chamfer chain | on a plateau edge cut at 45 degrees: alternate a chamfer cell (void triangle with a tall diagonal closure) and a cracked cell (one triangle a click or two lower), which reproduces the Cathedral's chain exactly (1.2) |
| Dip | a 2 by 2 cluster with one shared corner lowered a click: the Cathedral's bowl at (23..24, 15..16) |
| Gap | a floor cell becomes void (a hole to the layer below). Opt-in only (`gaps`, off by default; a gap in the middle of a floor does not cut a route, so the mask alone never kept them out), from intensity 70, and only if `keep_walkable` stays satisfied |
| Rubble | optional scatter cards (N8) in cracked cells |

**Reproducing the Cathedral style** is the acceptance test of the generator: seeded on the Cathedral's plateau regions, the output must consist of the same primitives (split-level cell with a 1 or 2 click triangle offset and a diagonal riser, chamfer chains, one-click bowls) and never of a step above `step_up` (the Cathedral never exceeds 512 gu = 40 ru). A fixture compares the histogram of operations (cracks, chamfers, dips, tilts) of the hand-made level with generated output at a matching intensity; it does not compare cells.

### 3.4 Editor gesture

Key **X** (free in `D/level-tools-design.md` 7.12 after the planes letters; avoids V F C W H P D O N and U S A T B K I E Y L R; proposal). Flow: region (marquee, brush `[` `]`, path, lasso) then `X`, drag vertically for intensity, live preview (the derived faces re-derive under the cursor), `Tab` cycles the target set (walls, floor, ceiling, all), `Shift+X` re-rolls the seed (the seed is shown and typeable in the option row), `Enter` commits, `Esc` restores exactly. Option-row chips for each parameter in 3.2, and a status line "+37 faces, 12 cracks, 4 rejected (would block the route), heat +3 percent". The walkability overlay (Alt+W) stays on while the verb is active.

### 3.5 MCP: `grid_break`

```
grid_break { scene?, room?,
  cells | brush {centre, radius} | path {points, width} | polygon {points},
  targets: ["walls","floor","ceiling"], intensity: 0..100, seed?: u32,
  amplitude?: fine steps, snap?: "fine"|"click",
  keep_walkable?: bool, spawn?: [node ids],
  materials?: "keep"|"mix", rubble?: bool, jitter?: 0..3,
  dry_run?: bool, reroll?: bool }
```

Returns: the seed used, counts per operation (crumbled, tilted, stubs, holes, cracks, chamfers, dips, gaps), cells touched, faces added, collision bytes added (derived: 24 B per cracked cell, see below), rejects with reasons, the walkability delta ("reachable cells 412 to 412, steepest 41 ru"), the heat delta per the heatmap model, and the name of the checkpoint it took. **Preview and iterate:** the tool takes an automatic `checkpoint` (`M/tools/project.rs:144`) before applying; `reroll: true` restores that checkpoint and applies again with the next seed; `dry_run: true` returns the report without writing. To look, the agent calls `screenshot` with an orbit around the region (the preview renders the STAGED document, `M/tools/verify.rs:83`), and `restore` or `undo_last_edit` (`project.rs:155,197`) put it back. A `variants: n` option is deliberately not offered: each variant needs a preview render (seconds), so the agent decides how many to look at.

### 3.6 Cook, collision, perf (labelled)

* **Cook:** no new format. Cracks write `SplitB` (16 B) plus a `SectorExt` (8 B) per cracked cell: 24 B each [derived from `G/format.rs:309-360`], 1000 cracked cells = 24 KB [derived], against RAM the level already budgets. Voids add `ColWall`s (12 B) for closures. `top_delta` and holes add nothing to collision unless an override makes them solid.
* **Render:** a cracked floor cell is +2 primitives (3.1's N1 estimate); a crumbled wall is 0 extra primitives (same quad, different heights); a hole is +1 or 2 quads; Rough 0. At 741 cycles per visible quad [fit], 100 visible cracked cells add about 148k cycles, about 25 percent of the 600,960 cycle world budget [derived from 1,128,960 - 528,000]. The verb therefore reports the heat delta and warns above a configurable share, and `intensity` for floors defaults conservatively. Merge (T14) re-merges coplanar neighbours but cannot merge cracks.
* **Collision:** cracks use the existing per-triangle floor model (A6); crumbled and tilted cells change slab heights only. The keep-walkable mask is exactly the same predicate the walkability overlay shows, so what the verb preserves is what the motor does (the overlay's step test is checked against the real motor at 40 and 45 ru, design section 19).
* **Works across cell sizes** once the quadtree lands: the verb iterates leaves, bites and cracks apply to level 2 to 4 leaves (needs the relaxation in open question 1), and a "split to fit" chip subdivides a coarse leaf before cracking it, as Stamps do (`D/level-tools-design.md` 7.11). Until then it works on whole-sector cells only.

### 3.7 Tests

Determinism (same seed, any region order, same document), monotone intensity (cells touched at 40 are a subset of those at 60), exact cancel and undo, no step above `step_up` and no unreachable cell under `keep_walkable` over random regions and seeds, the Cathedral histogram fixture, and a headless route that walks a broken plateau edge with the real motor.

---

## 4. Architecture: arches, vaults, free-standing pieces, a library

"Think also at how to build architectural features, like arches or vaults." Everything below is parametric, writes ordinary grid data or free planes, and has an editor gesture and an MCP tool.

### 4.1 What already exists

* The Cathedral's arcade (1.2): polylines of one wall piece per sector with sloped bottoms.
* **ArchProp**, a node kind with a parametric generator: span 1 to 3 tiles, depth 1 to 3 tiles, rise, leg height, band thickness in height quanta, `filled_top`, left, right or full portion, 2 to 6 segments per quadrant, four materials (fascia, soffit, extrados, end caps), and a curve enum with only `Round` ("later pointed, segmental or horseshoe profiles reuse the same pipeline", `psxed-project/src/arch_prop.rs:35-85`). It cooks to quads and one conservative AABB per profile segment (`arch_prop.rs:376-436`) that the playtest collects as character blockers (`engine/examples/editor-playtest/src/box_props.rs:70`, `game_logic_runtime.rs:291`; the motor can step onto a blocker top, `character_motor.rs:2438`). It is expanded by the legacy playtest cook (`playtest/cook_props_lights.rs:1572`) and drawn by `psx-game-runtime/src/arch_props.rs`. I found no reference to it in the grid cook or the grid MCP; whether a grid level cooks it through the same manifest path is unverified (open question 9).
* **ArchPerch** (Graybox Reach). It is not geometry. It is the Aletha animation action `ArchPerch` (`psx-level/src/lib.rs:3188`) played while the player hangs from an **`EntityKind::HookPoint` node**: the anchor is the node's position plus a marker height, the hand offset `PERCH_HAND` is relative to it (`engine/examples/editor-playtest/src/hook_runtime.rs:27-37`, `psx-game-runtime/src/hook_points.rs:92`). The bonnie importer places one per `--perch X,Z` about 1792 above the floor (commit 2a43766f, `import_bonnie32_level.rs`). So an arch generator keeps ArchPerch working by (a) never moving or reinterpreting HookPoint nodes, and (b) optionally emitting a HookPoint node at the crown (`perch: true`), through the ordinary `place_node` (`M/tools/nodes.rs:108`). The park decision (Manny does not care about Graybox Reach for now) stays; this only keeps the door open.

### 4.2 A1: Arched openings (the Opening verb gains a profile)

**Profiles:** `round` (semicircle, rise = half span), `segmental` (rise below half span), `pointed` (gothic: two arcs, rise above half span, `rise_ratio`), `flat` (lintel), `horseshoe` later. Parameters: `span` and `spring` (clicks or gu), `rise`, `segments` (polyline count over the span), `reveal` (soffit depth, see below).

**Geometry.** The cut is a polygon in the wall plane: left jamb, polyline intrados, right jamb. It goes through the polygon pipeline T4 built for bands (`P/band_geom.rs`: every rail is linear in position, rails that cross cut the span at the crossing): the intrados is a polyline whose segments are linear rails, so each segment is cut as its own piece and the existing `Span`/rail code applies unchanged. A wall face may not have more than 4 corners (`cook/render.rs` `keep.len() > 4` error), so the wall around the cut is emitted as: one quad per segment from the intrados up to the lintel top line, plus the jamb quads and the spandrel up to the ceiling. With `n` segments that is about `n + 2` primitives per wall face [derived]. At 741 cycles per quad [fit], an 8-segment arch costs about 7.4k cycles per face in view [estimate], twice that if both sides of a thick wall are seen.

**Bands continue around the arch.** New anchor `Anchor::Intrados(offset_gu)` (`P/bands.rs`): a cut line at a vertical offset above the intrados polyline. Because it is a vertical offset of a polyline, each segment's rail stays linear and the pipeline needs no new geometry code; the visible band is slightly thinner on steep segments than a true normal offset, accepted. `BandV::FromIntrados` seats V on the arch for an archivolt trim. `grid_band_cut` accepts `anchor: intrados`.

**Collision.** Native collision represents a wall as a zero-thickness vertical quad per sector edge with `[bottom a, bottom b, top b, top a]` (`G/format.rs:343-348`), bodies see its bounding vertical span (B6), rays see the true slanted piece (B7). So an arch is the lintel pieces only, one `ColWall` per piece with a sloped bottom, exactly how the Cathedral does it, and the opening is the absence of wall. Rules: (1) each piece is one cell edge until T10 gives `ColWall.reserved` a `from..to` partial span in quarter-edge units (`D/level-tools-design.md` 10.2); until then an arch is at most one segment per sector. (2) A body passes under only where the lowest bottom of the pieces it overlaps is at least `body_height` (48 ru) over its feet, so the verb refuses `spring` below `body_height + step`'s worth and says so (the Cathedral springs at 300 ru). (3) Camera and line of sight are exact.

**Reveal (jamb and soffit thickness).** Grid walls have no thickness. Two honest options: a thick wall as a row of void cells closed on both sides (R3, both faces get the arch cut; the passage cell's ceiling is the soffit, a barrel `Vault` across it, 4.3, which needs 1/4 cells to have more than one segment across a sector), or an ArchProp band node for the depth (it already has soffit, fascia and extrados materials and depth in tiles). v1 uses the ArchProp for the reveal if grid integration is confirmed, else a flat opening with no reveal, like the Cathedral.

**Data.** `Opening { edge, from, to, bottom, top, frame, profile: Option<ArchProfile> }` where `ArchProfile { kind, rise, segments, spring: HeightRef }`; serde-defaulted, so an Opening without a profile is the rectangle design 4.5 specifies. Authored walls are split into sill, jamb and lintel records at creation, as 4.5 says.

**Editor.** The Opening verb (I) gets the chips `Door`, `Window`, plus `Arch: round | segmental | pointed | flat` and a `Segments` stepper; the drag sets span and, vertically, the spring and crown. Live ghost shows the polyline.

**MCP.** `grid_opening { room, edge, cell | run, from, to, bottom?, top?, profile?: {kind, rise, segments, spring}, frame?: material, bands?: "follow" | "none", reveal?: "none"|"arch_prop" }`, reply lists primitives added, lowest bottom (the headroom check), and collision pieces.

**Phase.** After T13 (Opening) and T10 (partial spans); a **v0 that needs neither** ships earlier as `grid_arch_wall`: for an authored wall run, replace a rectangular opening by the Cathedral's stepped sloped pieces at one segment per sector (data only, `GridWall` heights), at least the Cathedral's quality, so agents can build arcades now.

### 4.3 A2: Vaults

A `Vault` verb writes ceiling heights along an arc across a span.

**Types:** `barrel` (profile across the span, constant along the run), `groin` (two barrels crossing: height = max of the two profiles, so the crossing gets the diagonal ridge that makes a groin vault; the ridge runs along the cell diagonals, which is where N1 helps), `pointed` (gothic profile), `cloister`/`dome` later. Parameters: `region` (the span), `axis`, `spring`, `rise`, `profile`, `ribs: bool`.

**How cells carry it.** Ceiling corner heights per cell. With whole-sector cells a barrel has one corner pair per sector across the span, so a 2-sector corridor can only be a gable (2 segments) and a 4-sector nave 4 segments, which is the Cathedral's resolution. Smooth vaults need 1/2 and 1/4 leaves across the span: **quadtree dependency** (T9 to T11), and the leaf relaxation of open question 1 is not needed here (ceilings are plain surfaces).

**Render cost and the cook merge.** A barrel's profile is constant along the run, so a strip across the full run with the same profile is planar along its length. T14's merge (design 9.2, aligned coplanar leaves) can merge those strips into one quad per profile segment instead of one per cell. Visible quads then scale with the number of segments across the span, not with the area: a 4-sector-long, 4-sector-wide barrel at 1/4 resolution is 16 segment strips, not 256 cells [derived]; without the merge it would be 16 by 16 = 256 quads, about 190k cycles in view at 741 each [derived, an unacceptable share]. So the generator reports quads before and after merge and refuses to ship above a configurable share, and the verb is scheduled after T14, with a no-merge fallback for small vaults.

**Collision.** Ceilings are slabs with the same height function and `ceil_b` for split-level ceilings (A6), so a vault is already supported; solid ceilings only block where headroom is below body height. Bodies jumping into the crown are the only place the curve is exact enough to matter [estimate].

**Ribs.** Optional: `ribs: true` also writes a band set or free-plane ribs along the groin diagonals (4.4).

**MCP.** `grid_vault { room, region, kind: barrel|groin|pointed, axis: x|z, spring, rise, profile_segments?, material?, ribs?: bool, leaf_level?: 2..4 }`, reports leaves added, collision bytes (20 B per leaf, `G/format.rs:298-306` [derived]), quads before and after merge, headroom minimum.

**Limits.** `MAX_LEAVES_PER_ROOM` 16383 (design 3.7); heights stay on the 64 gu lattice, so a vault of rise 1792 gu steps in 64 gu units, 28 levels [derived].

**Phase.** After T11 (Sculpt writes the same leaf data) and T14.

### 4.4 A3: Free-standing pieces from planes

*Built as static meshes instead (2026-10-17, `docs/grid/meshes.md`): `grid_arch_span` makes a round, segmental, pointed or flat arch between two floor points with piers, per-piece collision and a perch socket; columns, rubble and imported OBJ props are the same asset kind. Planes stay for ribs, lintels and leaning buttresses.*

**Spanning arch, rib, buttress, lintel, flying buttress** from `span`, `rise`, `thickness`, `segments`. One plane per voussoir segment (a rectangle `thickness` by `depth`), each placed by the existing from/to placement (`grid_add_plane` has a `from`/`to` span mode, `M/tools/planes.rs`).

* **Collision classes:** a plane steeper than normal.y 0.7 is solid and the shape is a box around the plane for leans between 0.35 and 0.7 (G2), so a round arch's flanks are blockers (boxes) and the crown segments with normal.y at least 0.7 are walkable platforms. **An arch top is walkable** where the crown segments are flat enough, which is what a grapple mechanic needs.
* **ArchPerch stays working** (4.1): `perch: true` places a `HookPoint` node at the crown; hook behaviour is entity-driven and does not consult the planes' collision.
* **Cap:** 64 planes per room (`P/limits.rs:33`) and the per-frame box test (comment at `limits.rs:30-32`). A 6-segment arch is 6 planes; ten arches exhaust the room. Options: (a) raise the cap after measuring the per-frame test cost, (b) emit the arch as ArchProp (one node, no plane records, per-segment AABB collision) instead of planes, (c) expand to ordinary quads like scatter. **Recommendation:** use ArchProp for the generic round and extended-curve case (it exists, is tested, and has the right materials and depth), and planes only for ribs, lintels and leaning buttresses where non-lattice angles matter.
* **Extend ArchProp's curve enum** with `Pointed`, `Segmental`, `Flat` (it was designed for it, `arch_prop.rs` doc comment on `ArchCurve`). That gives pointed arches free-standing and as reveals with the same `generate_arch_prop_*` functions the cook and preview already share (`arch_prop.rs:23-33`), so editor and cook cannot drift.

**Data.** For planes, ordinary `GridPlane` records plus an optional `group: u16` so the pieces move and delete together (new serde-defaulted field). For ArchProp, the node as it is plus the extended enum.

**Editor.** Plane tool (N) gains a `Span` mode: click two ends, drag for rise, chips for kind, thickness, segments. ArchProp is placed through the existing node tools.

**MCP.** `grid_arch_span { room, from, to, rise, thickness, depth, segments, kind: round|pointed|segmental|flat, as: "planes"|"arch_prop", material, collision: decor|walkable|solid, perch?: bool, group? }`; reply lists planes or node id, collision shapes each cooks to, and the room's plane count against the cap. Also `grid_rib`, `grid_buttress` as thin presets of the same call.

**Perf.** One quad per plane segment when visible at 741 cycles [fit]; plus the AABB or box tests in collision [unmeasured]. A 6-segment arch seen from the front is under 6k cycles, from both sides double (planes are one-sided unless set otherwise) [derived].

**Phase.** Independent of the quadtree; needs only the planes (merged) and the cap decision. Can be the first architecture slice.

### 4.5 A4: A library through Stamps

Stamps (`D/level-tools-design.md` 3.8, 7.11, T13) carry leaves, overrides, openings and band sets. Ship a **generated library**: parametric stamps made by the generators above, not hand-drawn files.

| Stamp | Built from | Needs |
| --- | --- | --- |
| Octagonal pier | a 3 by 3 cell footprint today (centre void, the four corner cells carry a void triangle and a diagonal closure, N1), a 1 by 1 sector pier with 1/4 corner chamfers once leaves allow N1 at level 4 | N1; quadtree for the small form |
| Square pier with plinth | void centre, band trims | bands (done) |
| Arcade bay | pier, arch opening (4.2), spandrel, bay width | 4.2 |
| Window | Opening with a profile and a frame | 4.2 |
| Door frame | Opening `Door` plus lintel and bands | T13 |
| Stair core | Stairs verb output (spiral as a repeated landing) | T7 (done on grid/tools) |
| Buttress | stepped raise plus a plane rib | 4.4 |
| Broken tile patch | Break output frozen as a stamp | N3 |

Stamps hold heights relative to the anchor floor and rotate and flip with the clipboard transform (`P/edit/clipboard.rs`, which already swaps diagonals under a mirror: test at `clipboard.rs:449-459`). **Stamp contents gain `planes: Vec<GridPlane>`** (relative) so a pier with a rib, or an arch, stamps whole; nodes (hook points) stay out in v1 as the design says.

**MCP.** `grid_stamp_library` lists the generated kinds with their parameters. `grid_stamp_make { kind, params, name }` generates and saves one into `GridWorld.stamps`. `grid_stamp_place { room, stamp, at: [x,z], rotate: 0..3, flip?, split_to_fit?: bool }`. The same pair serves the Break patch and any hand-saved stamp.

**Perf.** Whatever the contents cost; the library entries carry their own heat estimate computed by the heatmap model (`heat.rs`), shown in the picker.

---

## 5. Phase placement

The existing plan is T1 to T16 (`D/level-tools-design.md` section 13); T1 to T4 are on `grid/main`, T5 to T7 on `grid/tools` (T7's Stairs and heatmap staged, not committed), T8 to T16 not started. New phases are named N and slot beside them. Every phase stays "make it work first, report perf, never gate on it".

| Phase | Delivers | Depends on | Notes |
| --- | --- | --- | --- |
| **N1 Diagonal geometry** | R1d, R2d, R3d, `void` triangles, diagonal `EdgeKey`, `Convert` and `duplicates_derived` for diagonals; `grid_crack`, `grid_void`, `grid_set_wall`; Height tool Alt-drag per triangle; C1 collision fix (front-only cliff walls) | T1 (done), T4 (done); T5 region (grid/tools) for the chamfer chip | The unblocker for everything. One agent run for derive plus faces, one for collision plus tools |
| **N2 Rough and look** | `rough` (2.4), noise tint, `PaintRules` height rules and `grid_paint_rules`, `grid_rough`; the clip census | T3, T4 (done) | No format change. Preview parity for AO is a separate follow-up (2.7) |
| **N3 Break v1** | `X` verb, `grid_break`, `top_delta`, reroll via checkpoint, Cathedral-histogram fixture | N1, T5 region and verb contract, T6 walkability (for the mask) | Whole-sector cells. v2 below |
| **N4 Organic outlines** | lasso region, `grid_polygon`, `grid_chamfer`, "Diagonals" chip on Room and Corridor | N1; reuses T12's footprint code when it lands, does not wait for it | |
| **N5 Scatter** | `ScatterRule`, decor quads, `grid_scatter` | T4 (done); independent otherwise | Decides the plane cap question |
| **N6 Arch spans** | `grid_arch_span`, ArchProp curves `Pointed/Segmental/Flat`, HookPoint `perch` option | planes (merged), confirm ArchProp in grid (open question 9) | Architecture slice 1 |
| **N6b Arch wall v0** | `grid_arch_wall`: stepped sloped pieces, one segment per sector, at the Cathedral's quality | nothing | Cheap, gives arcades now |
| **N7 Sculpt+** | Noise (T11), Erode, Cliff, `grid_sculpt`, `grid_slope_field` interim | T8 to T11 (Sculpt itself); `grid_slope_field` needs none | |
| **N3b Break v2** | leaf-aware cracks, "split to fit", partial-width holes via Opening | T9, T11, T13, open question 1 | |
| **N8 Vault** | `Vault` verb, `grid_vault`, quads-before-after-merge report | T9 to T11, T14 | Small vaults ship without the merge with a warning |
| **N9 Arched openings** | Opening profiles, `Anchor::Intrados`, partial-span `ColWall` | T13, T10, N1 | The full 4.2 |
| **N10 Library** | generated stamps, `planes` in stamps, `grid_stamp_*` | T13 (Stamps), N1, N9 for arcade bays and windows | |
| **N11 Preview occluder** | preview builds collision tables for shadows and AO | none | Optional but needed to judge AO via `screenshot` |

### 5.1 Minimal first slice: a natural cave or canyon through the MCP

Goal: an agent, with only MCP calls, builds a cave and a canyon that read natural and verifies it. Smallest set, in dependency order:

1. **N1** (derived diagonals, void, `grid_crack`, `grid_void`, `grid_set_wall`, front-only cliff walls). Without it neither cracks nor 45 degree outlines exist.
2. `grid_polygon` and `grid_chamfer` from **N4** (a stand-alone MCP tool; the lasso gesture can follow).
3. `grid_raise`, `grid_ramp`, `grid_stairs` from `grid/tools` (T5 to T7), for the floor profile, merged first.
4. **`grid_break`** (N3) for walls and floors, with `keep_walkable`.
5. **`grid_rough`** (N2) for the rock.
6. `grid_walkability`, `grid_heatmap` (exist on grid/tools), `screenshot`, `checkpoint`/`restore` (exist) as the loop.

Canyon: `grid_polygon` an irregular floor with no ceilings, `grid_raise` the rims, `grid_break targets [walls,floor]` with rubble off, `grid_rough level 2 targets [walls]`, screenshot, reroll once, `grid_walkability`. Cave: add ceilings by `grid_polygon ceiling_height`, `grid_break targets [ceiling]`, a few `grid_crack kind ceiling`. Needs no quadtree, no Opening, no stamps, no planes. Everything else (Sculpt, vaults, arches, library) layers on without changing this slice's data.

---

## 6. Open questions (each with my recommendation)

1. **Allow `heights_b`, diagonal walls and void triangles on sub-sector leaves?** `D/level-tools-design.md` 3.2 forbids them below level 2. The smallest natural detail (a 1/4 sector chip) needs them. Recommend: allow on levels 2 to 4, keep levels 0 and 1 planar. Cost: 16 B `SplitB` per cracked leaf.
2. **How to represent a half-cell void.** New `GridSurface.void` field (render skips the triangle, closure wall, collision floor stays) versus collapsing the triangle's floor onto its ceiling (zero format change, wastes two coincident faces). Recommend the field.
3. **Rough amplitude cap.** Recommend `R / 3` = 4 ru (about 51 gu at 5/4) as the default cap, `R / 2` after the headless clip census shows the model fits. I could not find the Aletha model's visual radius or the camera arm's wall clearance; both need reading before the cap is final.
4. **Fix H1 with front-only cliff walls (flag read in `scan_walls` and `ray_walls`) or by deepening the floor slab under risers?** Recommend front-only walls: it reuses an existing format flag and costs 12 B per tall riser. Needs a collision tape measurement because it touches the motor's hot path.
5. **Do not persist break seeds.** Recommend: the document holds only the result (and `rough.seed`, which is a render parameter); the MCP workspace remembers the last break's checkpoint for re-roll.
6. **Arch cut collision before partial spans (T10).** Recommend shipping v0 (`grid_arch_wall`, one segment per sector) now and the profile cut after T10, rather than inventing a second partial-span encoding.
7. **Plane cap.** Do not raise `MAX_PLANES_PER_ROOM` (64) blindly: the per-frame box test cost is unmeasured. Recommend scatter as ordinary quads (no cap pressure) and ArchProp for arches; raise the cap only after a measurement.
8. **Keys.** Break `X`; Vault `G` (proposal; V is taken, G is in the "kept free" list of 7.12); Arch spans live in the Plane tool (N) as a mode. Recommend approving X and G, and Shift+X for re-roll.
9. **Is ArchProp usable in grid levels?** *(Answered 2026-10-17, `meshes.md` section 1: it cooks in a grid world (test), but the MCP cannot create one and a free-standing arch is now a static mesh with its own generator, profiles, collision and a crown hook socket.)* The playtest runtime collects arch-prop blockers (`box_props.rs:70`) but I found no grid cook or MCP reference. Recommend a one-line check by whoever has the cook running: place an ArchProp node in a grid project and cook it. If it works, extend its curve enum and expose it; if not, the plane route is the fallback.
10. **Preview shadows** (N11): recommend building it, because AO and shadow looks cannot otherwise be reviewed through the MCP `screenshot`, which Manny's workflow depends on.
11. **Break and the quadtree before T9:** recommend shipping Break v1 on whole sectors now (it already reproduces the Cathedral style, which is sector-granular) rather than waiting.

### Smells noticed (separate issues, not fixed here)

* `WALL_FRONT_ONLY` and `ColWall.reserved` are in the format and read by nothing (`G/format.rs:334-348`).
* Nothing writes `heights_b`: the field has a full read and edit stack and no creator (A2).
* `DerivedWall.edge` and `physical_edge_walls` encode "cardinal only" as a comment and an early return (`P/derive.rs:68,356`).
* `PaintRules` has an editor panel and no MCP setter; the MCP reaches only part of the editor's features.
* The collision body model's `wall_span` bounding box (`G/collision/world.rs:408`) is an undocumented approximation of every sloped-bottom wall.
* Character body height appears as 48 ru in the motor tests and `WalkLimits` and as 1024 editor units (64 ru) in the character resource default (`resource_types.rs:2295`), 12 ru radius in both. Which is authoritative for a headroom check is worth confirming.
