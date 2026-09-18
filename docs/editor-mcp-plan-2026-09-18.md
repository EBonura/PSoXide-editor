# PSoXide Editor MCP: plan

Goal: an agent authors complete, textured, correctly-scaled level sections
(arena, square room with stairs, walkway, colonnade, arcade) in PSoXide,
modelled on the Blender MCP server's shape.

## What already exists

Three findings from surveying the repo reframe the work. None of them were
obvious before looking.

**The geometry primitives are already written.** `workspace/tools.rs` has
`brush_drag_cylinder` (N-sided pillar), `brush_drag_doorway_arch` (voussoirs
plus legs), `brush_drag_curved_wall`, `brush_drag_stairs`, `brush_drag_ramp`,
all built on `Brush::convex_prism`, whose doc comment calls it "the shared
primitive kernel for editor-authored ramps, cylinders, arches and stairs".
`brush.rs` adds `hollowed`, `subtracted_by`, `inset`, `clip`,
`extruded_from_face`. No new geometry is needed. What is needed is a headless
way to call it with validated parameters.

**The GUI already runs an MCP server.** `emu/crates/frontend/src/mcp.rs` is a
working rmcp streamable-HTTP server on port 7355, behind the `mcp` feature,
with the hard part solved: the emulator is non-`Send`, so tool calls become
`Cmd`s on an mpsc channel drained against `&mut AppState` in the redraw loop.
The editor is the same binary (`frontend` with the default `editor` feature).

**The headless renderer already exists.** `frontend dump-editor-preview`
renders the editor 3D viewport with full orbit-camera control and no GUI, and
`cmd_dump_editor_preview` is a clean pure function of (project, camera) →
RGBA via `build_phase1_frame`. `dump-editor-ui --view top` renders the whole
editor chrome.

## Premise test

Before designing tools, the premise was tested with what exists: can an agent
reason about a space from these renders? Three orbit views of the default
project were rendered headlessly.

Result: the render path works, but **neither existing view is usable for
spatial reasoning as-is**.

- The 3D preview is legible as *mood* and nearly useless as *measurement*.
  Cortex is a night-time city level; in Draft cook mode everything is
  fullbright, so what you see is the raw texture albedo, which is very dark.
  Volumes read as black masses.
- Aiming it is trial and error. Yaw/pitch/radius/target are six numbers with
  no framing helper, so getting a useful shot takes several attempts.
- `dump-editor-ui --view top` exposes no camera arguments at all. It opens at
  the origin at 0.035 px/unit while the default map is centred near
  (20160, -14272), so the plan view came back empty.

This is the single most valuable thing the premise test produced, and it
changes what Phase 1 is. Phase 1 is not "wire up a screenshot". It is **make
the spatial read legible and aimable**.

## Measured reference: the v0.4 default map

Measured from `editor/projects/default/project.ron` with the real parser.
These are the authored numbers a level tool has to reason in.

| Quantity | Value |
| --- | --- |
| Brushes / faces | 244 / 1464 |
| Faces per brush | exactly 6.0 average |
| World span (X, Y, Z) | 67200 x 11264 x 31104 |
| Brush X extent | min 128, p25 1024, median 3584, p75 5376, max 63360 |
| Brush Y extent | min 256, p25 1024, median 1088, p75 2304, max 9856 |
| Floor/ceiling slab thickness | 256 or 384, nothing else |
| Grid alignment | 100% on 64, 97.4% on 128, 87.9% on 256 |
| Faces carrying a material | 1464 / 1464 |
| Authored groups | 2 (one named `GroupS`, 235 brushes ungrouped) |

Enclosed headroom, sampled on a 512-unit grid at 6976 standable spots, with
the player hull inset:

| | units | player heights |
| --- | --- | --- |
| min | 1024 | 1.00 |
| p25 | 3840 | 3.75 |
| median | 5376 | 5.25 |
| p75 | 7616 | 7.44 |
| p95 | 8960 | 8.75 |
| max | 9344 | 9.12 |

Player and camera, from the live project: character radius 188, height 1024.
Heavy enemy radius 320, height 1741. Gameplay camera distance 3500 at height
1800, target height 1160. Gravity 96/tick. World sector 1024, draw distance
25000.

Three conclusions worth carrying into the tool design:

1. **The shipped level is entirely boxes.** 6.0 faces per brush across 244
   brushes means not one cylinder, arch, ramp or curved wall was used. The
   primitives exist and went unused, which is a usability signal about the
   drag-based UI and an opportunity for the MCP.
2. **Ceilings are generous.** A median 5.25 player heights, p25 3.75. This is
   an outdoor-feeling elevated structure, not corridors. Default room heights
   in generated content should sit near 3000-5500, not the 2000-2500 that a
   Quake instinct would suggest.
3. **64 is the working grid**, and slabs are 256 or 384. Generated geometry
   that respects those two facts will sit in the level without looking
   foreign, and 64 is coarse enough that curve snapping matters (below).

## Arches and pillars

The existing generators are correct but they **fail silently in three
places**. A human dragging a mouse notices; an agent does not. This, rather
than geometry, is what the MCP layer must fix.

1. `brush_drag_doorway_arch` returns an empty `Vec` when
   `thickness >= radius` or `thickness >= vertical_radius`. No error.
2. Every curve point goes through `primitive_snap(value, grid_step)`.
   Quantizing the curve can collapse a voussoir, `convex_prism` returns
   `None`, and the loop swallows it with `if let Some`. You ask for 6
   segments and get 4 with no complaint.
3. `brush_drag_cylinder` has the same problem. Snapped vertices dedupe, so an
   8-sided pillar can come back a square.

The collapse condition is derivable, not a matter of taste. The chord between
adjacent vertices of an n-gon of radius r is `2r*sin(pi/n)`, and it must clear
the grid step, so:

```
r > step / (2 * sin(pi / n))
```

On the map's 64 grid an octagon needs radius >= 84, meaning a footprint of at
least 168 units. The tool checks this up front and says "6 sides at this
radius, or widen to 168" instead of returning a mangled solid.

Arch geometry facts an agent cannot guess and the tool must surface:
`radius = width/2` and `vertical_radius = min(radius, height)`, so an arch in
a wide, short box silently becomes segmental rather than semicircular. Output
is `segments` voussoir brushes plus 2 legs, so the default 6 segments is 8
brushes and roughly 48 faces for one doorway.

Cost follows directly. An n-sided pillar is n+2 faces. Twelve 8-sided pillars
in one sightline is 120 faces in a single leaf, which is the open-sightline
corridor failure at 1.7M cyc/vblank. So every generator reports its
draw-cost delta, not just success.

**The real gap is array placement.** A colonnade is one pillar repeated eight
times at 512 spacing; an arcade is one arch repeated along a wall. Today that
is manual duplication. Linear and radial array over a brush group is cheap
(translate and rotate already exist in `groups.rs`) and it is what turns one
arch into architecture.

## Architecture

Destination is unchanged from the original sketch: an MCP server inside the
running editor, so edits appear live in Manny's viewport and land on his undo
stack. Copy `mcp.rs` wholesale, including the `Cmd` + drain-in-redraw bridge.

**Sequencing changed on the evidence.** The premise test showed the offline
path already works end to end, and that `cmd_dump_editor_preview` is a pure
function of (project, camera). The read-only tools are therefore pure
functions of `&ProjectDocument` with no GUI dependency. Building them offline
first is roughly ten times cheaper to iterate (the frontend is a 75-second
rebuild that drags in the whole GUI) and the tool bodies move into the GUI
bridge mechanically afterwards, because they never touched `AppState`.

So: **offline first, live second.** Same tools, same signatures, cheaper
iteration, and the live step becomes plumbing rather than design.

## Tool surface

Read (Phase 1):

- `scene_info` - bounds, brush and face counts, groups, materials, grid step,
  current draw cost.
- `metrics` - scale constants and the measured reference dimensions above, so
  the agent stops guessing how big a door is. The analogue of Blender MCP's
  `bpy_api_lookup`.
- `plan_view` - **the legibility fix**. An orthographic plan or section drawn
  directly from the brush solids, with grid, dimensions, a scale bar and the
  player capsule for reference. Pure geometry, no wgpu, no GUI. For reasoning
  about space this beats a dark 3D render, and it costs a fraction of one.
- `screenshot` - the 3D preview with auto-framing derived from world bounds,
  for checking material and mood rather than measurement.

Write (Phase 2): `add_shape` (the six existing generators behind validated
parameters), `make_room` (wraps `hollowed`), `array`, `set_material`,
`delete`. Every write goes through `push_undo` and stays in memory; the agent
never saves over `project.ron`.

Verify (Phase 3): `audit` - `brush_overlap`, the brush-world leak diagnostic,
and `analyze_pxbsp_draw_cost`, wired into the server instructions so it runs
after every structural change. This is the part with no Blender equivalent.
Blender has no shipping budget; PSoXide does, and it is what kills levels.

Phase 4, only if 1-3 earn it: prefab capture reusing
`editor/prefabs/<name>.ron`, so "arcade" becomes prefab plus array rather
than more code.

Deliberately not built: an `execute_blender_code` equivalent. That tool works
because bpy is a large, stable, documented API the agent already knows.
PSoXide has no scripting runtime, and the authoring surface is genuinely
small. Add it if tool requests exceed roughly twice per session.

## Phase 1: shipped

`editor/crates/psxed-mcp`, a stdio MCP server registered in `.mcp.json`.
Three tools, all pure functions of a `ProjectDocument` in `lib.rs` so the
move into the live editor is mechanical:

- `metrics` - the scale constants, reference bodies, working grid, measured
  ceiling heights, and the curve-snapping inequality.
- `scene_info` - extents, counts, faces-per-brush, groups, materials by face
  count, candidate floor levels.
- `plan_view` - orthographic plan or section, PNG plus a text legend, with an
  optional `center`/`extent` focus window.

Two gaps were found by using the tools rather than by reading them, and both
are fixed:

1. **No zoom.** The first whole-level plan put the player disc under one
   pixel, which reads the level's layout but says nothing about human scale.
   `focus` frames one space; at 11.5 units/px the player disc sits visibly
   against the 1024 grid and a gap can be judged in player widths directly.
2. **The skybox outranked every floor.** Candidate levels were ranked by
   footprint area, and the sky cube is a single brush covering most of the
   world, so the top candidate sectioned 1 brush out of 244. Brushes covering
   more than half the world footprint are now excluded as backdrop, and each
   level reports how many brushes top there.

One incidental finding worth knowing before Phase 2 writes anything:
`ProjectDocument::default()` is the **embedded starter level**, not an empty
document. A test that pushed two brushes onto it was silently working against
244 brushes plus a skybox.

## Phase 2: shipped

**The generators moved down into `psxed-project::brush_primitives`.** They
were private methods on the editor's drag tool, which would have forced the
MCP to either depend on the whole egui crate or reimplement them and drift.
They are pure geometry, so they belong beside `convex_prism`. `psxed-ui`
re-exports the three recipe types under their old names and its
`brush_drag_brushes` now delegates, so the drag tool is unchanged; its 525
tests pass untouched.

What is new is `generate`, which turns each silent failure into an error or a
warning carrying the arithmetic. Write tools in `psxed-mcp::edit`:
`add_shape`, `make_room` (authored by INTERIOR dimensions), `array` (linear
and radial), `set_material` (optionally filtered by face normal), `delete`,
plus `save`, `revert` and `materials`.

Edits stage in memory. `save` refuses when `project.ron` changed on disk since
the edits were staged, which is what happens when the editor saves over it.
That is the whole concurrency story and it is enough; no locking.

### The chord rule was wrong

The plan above derived `r > step / (2*sin(pi/n))` for pillar survival. A test
disproved it. That condition only keeps neighbouring vertices *distinct*;
snapping also drags them into a straight line, and `convex_prism` discards
collinear points and rejects non-convex turns. A radius-128 octagon on a 64
grid clears the chord test comfortably and still collapses to a diamond.

`minimum_pillar_footprint` searches by building instead. Measured minimum
square footprints:

| sides | step 16 | step 32 | step 64 | step 128 |
| --- | --- | --- | --- | --- |
| 4 | 32 | 64 | 128 | 256 |
| 6 | 48 | 96 | 192 | 384 |
| 8 | 96 | 192 | 384 | 768 |
| 12 | 128 | 256 | 512 | 1024 |
| 16 | 224 | 448 | 896 | 1792 |

So on the map's 64 grid an octagonal pillar needs a **384**-unit footprint,
not the 168 the formula predicted, and a 1024-wide box tops out at 12 sides.
`add_shape` reports the side count it actually built, which is the number to
trust.

### End-to-end validation

Built an arena against a copy of the default project: a 12288 x 5120 x 12288
room (5.0 player heights tall), two colonnades of seven 8-sided pillars, a
balcony, an eight-step stair run and a five-voussoir entrance arch. 36 brushes
and 242 faces across seven tool calls, verified by plan and section.

The plan view immediately showed a defect the tool call reported as success:
the stair block overlaps a pillar. Geometry tools cannot catch that, which is
the argument for Phase 3's `audit` (`brush_overlap` plus the leak diagnostic
plus `analyze_pxbsp_draw_cost`).

## Phase 3: shipped

`audit`, in three passes, cheapest first, selected by `depth`:

- `quick` (milliseconds): degenerate brushes, brushes past the extent limit,
  coplanar face overlaps, off-grid coordinates, untextured faces.
- `sealing` (adds ~0.1s): the brush-world leak diagnostic. A leak means the
  engine cannot compute visibility and the whole map falls back to drawing
  everything, so it is worth its own step.
- `full` (2.4s on the 244-brush default project): a real cook, its validation
  errors and warnings, and `analyze_pxbsp_draw_cost` per leaf.

Using it changed its design twice, the same way Phases 1 and 2 went.

**It needed scoping.** The first run on the arena reported 141 coplanar
overlaps, almost all of them pre-existing in v0.4. An incremental audit
drowned in the level's own history. `first`/`count` scope the report to the
work just done, and the overlap search still runs over the whole scene so a
new wall sharing a plane with an old one is still caught. Scoped to the 36
arena brushes it reported 13 overlaps, all mine, and the top offenders were
correct: the entrance arch's outer face is coplanar with the wall it was set
into.

**It needed to say what it does not check.** `find_brush_face_overlaps` finds
faces sharing a *plane*, which z-fight. Solids that merely interpenetrate are
legal here (`BrushContents::precedence` resolves them and the shipped level
relies on it), so the stair-through-pillar collision from Phase 2 is a
look-at-it decision rather than an error. The report says so rather than
letting a clean audit imply more than it means.

### What the shipped level measures at

The full audit on v0.4: 2390 world faces, 4464 base triangles, 347 non-solid
leaves, sealed, one cook warning (Aletha has no turn clip). The worst
sightline is leaf 186 at 3933 packet slots near authored
(18304, 3008, -15360), seeing 2012 faces across 282 leaves.

That is 84% of the entire map visible from one standing position, which is
the concrete shape of the open-sightline problem behind the measured 16.4 fps
floor. `audit` makes it a number an edit can be checked against.

## Risks

**Concurrent edits.** Manny will be in the editor while the agent drives it.
Blender MCP ignores this. Cheap mitigation: every write returns a document
revision, and the agent re-reads when it moved. Do not build locking.

**Draft versus Release lighting.** BSP point lights do nothing in Draft cook
mode, which is the default, so anything the agent judges by eye in the
preview is unlit. `plan_view` sidesteps this; `screenshot` must say which
mode produced it.
