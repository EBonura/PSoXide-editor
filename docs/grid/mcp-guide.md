# Building grid levels through the MCP server

`psxed-mcp` is an offline stdio MCP server over a project file. An agent can build a grid level end to end with it: rooms, floors and ceilings at heights, walls, materials and tiling, wall bands, free planes, lights and the player, then look at the result and play it. Every geometry tool is a thin wrapper over the same `psxed_project::grid::edit` functions the editor's tools call, so a level made here is made by exactly the code a mouse gesture reaches. Nothing in the server re-implements an edit.

## Run it and register it

The repository's `.mcp.json` registers it:

```json
{ "mcpServers": { "psoxide-editor": { "command": "cargo",
  "args": ["run", "--quiet", "-p", "psxed-mcp", "--", "--project", "editor/projects/default"] } } }
```

To work on a grid project, change the path after `--project` to any directory under `editor/projects` that holds a `project.ron` (`grid-cathedral`, a project made with File > New Grid Project, or one made with `--new` below). The server fails at start if the file does not parse.

| Argument | Meaning |
| --- | --- |
| `--project <dir or project.ron>` | The project to edit. Default `editor/projects/default`. |
| `--new <dir>` | Write a New Grid Project at `<dir>` and exit: what File > New Grid Project makes (the Graybox Reach base with the 6 by 6 starter room, assets linked). Keep `<dir>` beside the other projects so the `../default/assets` style paths in `project.ron` resolve. |
| `--frontend <path>` | The renderer binary for `screenshot` and `playtest`. Default: `target/run-fast/frontend`, `target/release/frontend`, `target/debug/frontend`, in that order. Build one with `cargo build --release -p frontend --features editor`. |
| `--scratch <dir>` | Where frames, tapes and logs go. Default the system temp directory. |

`playtest` builds the guest with the repository's staged guest build. Point `PSOXIDE_GUEST_STAGE_ROOT` at a private directory if another build shares the machine. A rebuilt frontend is needed whenever `psxed-project` changes (derived walls, bands and planes only draw in a frontend built after they landed).

## Units

A sector is **1024 grid units**. Cells are addressed by **world sector** `[x, z]`, the same lattice in every room; north is -Z and east is +X. Heights are grid units **relative to their room's `y_offset`**: a click is 256 (the default snap), the fine step is 64 (`snap: "fine"`), and heights always sit on the 64 lattice.

The world's scale turns grid units into **editor units**: 5/4 for a new project, so a sector is 1280 editor units and the 1024-tall player is 0.8 of a sector. Rule of thumb:

* grid units: every `grid_*` tool, including planes and meshes;
* editor units: nodes (`move_node`, `add_light`, `place_node`), `screenshot` poses, `grid_height_at` (unless `grid_units: true`), `grid_plan` marker coordinates, and the player path `playtest` reports.

`metrics` and `grid_rooms` print the exact figures. The player is 188 wide and cannot step up more than 640 editor units; there is no jump.

## The loop: READ, ACT, VERIFY

**READ** before inventing a number: `metrics`, `grid_rooms` (the room indices every tool takes), `materials` (`palette_only: true` for the world palette), `scene_info`. After every edit read geometry back:

| Tool | What it answers |
| --- | --- |
| `grid_heights` | ASCII height map of a rectangle of sectors, world heights in clicks, `.` where there is nothing. The quickest read-back. |
| `grid_sector` | One sector: floor and ceiling corners, split, materials, authored walls, derived walls with owner, material and effective tiling. |
| `grid_walls` | Derived walls (risers, drops, closures) and authored walls of a room or rectangle, with heights, run length, flags. This is where you learn a derived wall's host cell, edge and kind. |
| `grid_height_at` | Floor height at a point, per room and for the world (the function the runtime stands on). |
| `grid_planes`, `grid_bands` | Free planes with what they cook to; the cut lines and bands of a wall run. |
| `grid_meshes` | Static mesh assets (recipe, faces, collision shapes, sockets, slots, how often placed) and the placed instances by stable id, with the sectors each covers. Also lists the recipe kinds. |
| `grid_budget` | The face budget against its cap and every `limits::validate` finding. |
| `cook_check` | The real cook in memory: its errors one per line, warnings, exact size. What would stop a playtest build. |
| `grid_walkability` | The walkability overlay as text, with the motor's numbers: too steep, low headroom, steps taller than the body climbs, drops, ghost floors and floors no player spawn reaches, a map and the flagged edges. |
| `grid_heatmap` | The performance heatmap as text: a labelled MODEL of world render cost per spot against the 30 fps budget, percentiles, a map in tenths of the budget, the hottest spots. `playtest` telemetry is the measurement. |
| `grid_plan` | A top-down PNG: heights as colour, wall lines by kind, numbered rooms, plane footprints, numbered node markers, axes in sector coordinates. The picture for layout. |

**ACT**. Every tool stages in memory and reports numbers (what changed, what snapped, what it skipped). An edit that introduces validation findings says so in its reply.

| Area | Tools |
| --- | --- |
| Rooms | `grid_add_room`, `grid_delete_room`, `grid_set_room` (name, raise, ambient, wall material, open wall height, clean overrides), `grid_convert_room`, `grid_set_shading`, `grid_set_stream` (draw distance, activation and prefetch radii in sectors, walked over the portals) |
| Surfaces | `grid_add_surface` (floor or ceiling over a rectangle at a height), `grid_remove_surface`, `grid_set_heights` (corners, one edge for a slope, absolute or relative, optional weld to neighbours), `grid_set_split` (flip, set, or `auto` to let surfaces choose their own diagonal from their heights), `grid_set_walkable` (ghost floors and ceilings) |
| Level verbs | `grid_raise` (the Raise tool: offset or level, ceilings follow), `grid_ramp` (the Ramp tool: `keep` a side, `rise` or `walkable`), `grid_stairs` (the Stairs tool: a path, riser option, ramp option) |
| Natural terrain | `grid_polygon` (a polygon or corridor of floors, optional roof, 45 degree corner cuts), `grid_chamfer`, `grid_crack` (a triangle at its own height), `grid_void` (half a cell empty), `grid_set_wall` (an authored wall's four heights, an open top crumbled), `grid_break`, `grid_rough`, `grid_paint_rules` |
| Walls | `grid_place_wall`, `grid_remove_wall` (authored runs), `grid_wall_override` (hide, ghost, repaint a derived wall; reset) |
| Look | `grid_paint`, `grid_paint_wall` (a wall or its whole run), `grid_tiling` (World density or Fit N x M, turns, flips, offset, scale), `grid_material_tiling` (a material's default) |
| Bands | `grid_band_cut`, `grid_band_move`, `grid_band_delete`, `grid_band_set` (material, tiling and V per band; scope `run` or `room`) |
| Planes | `grid_add_plane` (by centre or by the midpoints of its two ends), `grid_set_plane`, `grid_delete_plane` |
| Meshes | `grid_arch_span` (an arch between two floor points), `grid_place_mesh` (an asset or a recipe: arch, column, rubble, or an OBJ file), `grid_set_mesh` (move, turn, scale, retexture, or change the asset's recipe for every copy), `grid_delete_mesh` |
| Nodes | `place_node`, `set_node`, `move_node`, `delete_node`, `add_light`, `lights`, `get_node`, `entity_types` |
| Safety | `checkpoint`, `restore`, `checkpoints`, `save`, `revert`, `undo_last_edit`, `list_backups` |

**VERIFY**.

* `screenshot` renders the **staged** document (saved or not) with the editor's preview renderer. A pose is an eye `position` plus `look_at` (or `yaw_deg` and `pitch_deg`), or an orbit around a `target` point or a `node` with `distance`, `yaw`, `pitch`. Up to 8 shots in one call through `shots`. Heading: yaw 0 looks north (-Z), 90 degrees looks west.
* `playtest` cooks a real disc (about half a minute), boots it headlessly and plays it (roughly 8 seconds per thousand polls). With a `route` it plays a path and returns frames along it and a frame-cost summary.

### Playtest routes

```json
{ "route": [ {"polls": 600, "dir": "forward"}, {"polls": 750, "dir": "right"} ],
  "warmup_polls": 450, "frames": [0, 200, 560, 1000], "track_player": true }
```

A route becomes a poll-bound input tape: sample N is what the guest reads at its Nth controller poll, one per simulation tick, 60 a second, so a route plays the same on any build. It opens with `warmup_polls` (default 450) while the level loads, with an occasional CROSS tap that clears a screen waiting for one; the legs start after it. A leg has `polls`, a left `stick` `[x, y]` (-1 to 1, y forward) or a `dir` (`forward`, `back`, `left`, `right` and the diagonals), a right `camera` stick and held `buttons`. The default camera looks north and does not turn on its own: forward is north, right is east. A route far shorter than it needs is the usual surprise, so check the player path.

* `track_player` builds the disc with the guest's `camera-pose-log` and reports the player's position (editor units) at each returned frame, its start and end, its height range (a fall shows as a low minimum) and one point a second. It is how you check a bridge was crossed instead of reading it off a picture.
* With `track_player` the report also lists the hook runtime's own events by route poll (`hook events (route poll): arch:attached at 296, arch:drop at 410`): `arch:attached` (hanging from a hook point), `arch:drop` (let go), `arch:blocked` (the flight hit something), `arch:timeout` (the hold ran out). It is how a perch is checked.
* `telemetry` (on by default for a route) builds with `emulator-telemetry` and reports render cycles, world (room draw) cycles and frame work per presented frame: mean, p90, max, and how many exceeded the 1,128,960 cycle work budget of a 30 fps frame. It costs guest RAM, so a level near the cap may not link with it: unless you asked for it by name the build then falls back to a plain disc, and the summary is frame cadence from the emulator's route log (presented frames, fps, how many took more than two vblanks). `track_player` adds a little guest-side logging, so read cost from a run without it.
* `frames` are polls after the route starts; the nearest captured frame is returned.

Without a `route`, `playtest` runs the older fixed schedule (tap CROSS through the menus, optionally walk forward) for `polls` polls.

## Meshes: arches, columns, rubble, props

A mesh is not grid geometry. An **asset** is a recipe (or imported OBJ geometry) the world keeps once, and a room places **instances** of it; the cook draws an instance as a run of quads with the room's planes, collides it through the grid's own collision records and turns its sockets into hook points. `docs/grid/meshes.md` has the model, the cost (an arch is about 2.9 KB of chunk, a column 1.6 KB) and the audit of what existed before.

* **`grid_arch_span`**: `from` and `to` are the floor points under the two springings, grid units; the clear opening is their distance (256 to 8192) and the arch is turned to run along them, standing at the lower height. `profile` is `round` (a quadrant of an ellipse each side, a semicircle when the rise is half the span), `segmental` (rise under half the span), `pointed` (rise at least half the span) or `flat` (a lintel). `spring` is the pier height (keep it above 1,000 grid units at the default scale if a body is to walk through: the character is 48 runtime units tall), `rise` the crown above it, `thickness` the band and pier width, `depth` along the axis, `segments` the pieces per half (1 to 8). The reply gives the numbers, the sectors covered and where the hook grips.
* **`grid_place_mesh`**: an `asset` (index or name) or a `recipe` (`kind`: `arch`, `column` with radius, height, sides, plinth, capital; `rubble` with size, pieces, seed; `obj` with a `path`, `scale` grid units per OBJ unit and `collision`: `none`, `box` or `box_top`). Equal recipes share one asset. `position` is grid units (x and z on the world lattice, y relative to the room), `rotation` or `yaw` degrees, `scale` one number or three, `material` for every slot or `materials` per slot (arch slots: fascia, soffit, extrados, pier).
* **`grid_set_mesh`** by stable `id`: position, rotation, scale, materials, `collide`, `fullbright`, `enabled`, another asset, or `asset_recipe` to change the numbers of the asset in place so every copy follows. **`grid_delete_mesh`** removes by id; other ids never change.
* **Collision.** Piers and columns block (a box; exact at any yaw), the arch's top pieces are surfaces to stand on (the crown holds the player at its extrados), the haunches block as boxes and the cook warns once that this over-blocks the space under them. `collide: false` makes a mesh decoration.
* **Perch.** An arch with `perch` (the default) has a hook socket just under the crown. The cook spawns a hook point there, and the player can hang from it: Triangle for the ranged stance, hold L2, pitch the camera so the crown is at the reticle, press R2; Circle lets go. `playtest` with `track_player` reports `arch:attached` and `arch:drop`. The perch needs a ceiling clear of the arch's top, the body path under the crown clear, and the crown within 1,600 runtime units.

## Natural terrain: caves, canyons, rock

The tools above make axis-aligned rooms and slopes. These make rock. They write ordinary grid data, so everything stays hand-editable and every other tool still applies.

* **Diagonal geometry.** A surface whose two triangles sit at different heights is *split-level*; in a derived room its fold diagonal grows a derived wall (a riser, or a drop for a ceiling), seen from the lower triangle, listed by `grid_walls` as `diagonal NW-SE riser`. A step taller than the character climbs (40 runtime units, 3 clicks at the default scale) also cooks a one-sided cliff wall: rays, the camera arm and a body below the cliff stop at it, a body on the ledge can still walk off. A *void* triangle draws nothing; the other half of the cell closes itself with a solid diagonal wall, and the neighbours across the cardinal edges of the void half close themselves too. `grid_crack` makes the first, `grid_void` the second. `grid_sector` prints both.
* **Outlines.** `grid_polygon` takes points in *sector coordinates* (numbers; cell `[3, 2]` has its centre at `[3.5, 2.5]`), kind `room` (the closed polygon), `corridor` (the polyline, `width` sectors) or `region` (report only), `floor_height` and, for a cave, `ceiling_height`. With `chamfer: corners` (default) every convex staircase corner, a cell with exactly two adjacent neighbours missing, is cut: its outer triangle is voided, so a run of unit steps becomes one 45 degree wall. Steps wider than one sector stay steps. `grid_chamfer` does only the corner cut on floors that exist (`void`, `crack` for a plateau corner, `restore`).
* **Break.** `grid_break` (region: `from`/`to`, `brush {centre, radius}`, `path {points, width}` or `polygon`) adds detail to walls (plateau lips crumble, edges are bitten into diagonal cracks, open wall tops get ragged), floors (interior vertices tilt, tiles crack along a diagonal) and ceilings. `intensity` 0 to 100 (15 is the default and the Cathedral's sparse hand-made look, which measures at about 10 to 20; 40 and up reads as rubble, 70 and up rubble and gaps), `style: "cathedral"` sets that look's intensity, amplitude, snap and targets in one word, `seed` repeats a result, `amplitude` is the largest change at intensity 100 in fine steps of 64 grid units. With `keep_walkable` (default) each change is checked against the walkability analysis, the motor's own numbers: nothing that adds a floor too steep to stand on, a step the character cannot climb or too little headroom survives, and the reply lists what was refused and the walkability before and after. A checkpoint `before-break` is taken first; `reroll: true` restores it and breaks again with the next seed, `dry_run: true` only reports. Nothing about the break is stored but its result.
* **Rough.** `grid_rough` paints a level 0 to 3 over a region and sets the room's seed, amplitudes (`amp`, grid units at levels 1, 2, 3) and brightness `tint`. It moves the plan position of face corners by a hash of the lattice point, never the height, so the surface stays watertight and the preview and Play agree. Collision stays on the grid: the default amplitudes are 1, 2 and 4 runtime units, and anything above 4 (a third of the character's radius) is a `grid_budget` warning because the rock would clip the model.
* **Look rules.** `grid_paint_rules` sets floor material by slope class and by height, the wall material, strata (wall material by height, made the room's default band set with an absolute line per threshold) and the trim; with a region it applies the floor rules now. Rules run inside the verbs on floors not painted by hand.

## Recipes

* **A cave**: `grid_polygon` an irregular outline with `floor_height` 0 and `ceiling_height` 2048, `grid_raise` or `grid_ramp` a few areas, `grid_break` on the whole thing (walls, floor, ceiling), `grid_rough` level 2 over it, `grid_paint_rules` for scree low and rock high, then `grid_walkability`, `screenshot` from three poses and `playtest` a route.
* **A canyon**: the same polygon with no ceiling and `grid_set_room` `open_wall` for the rock walls (their tops are what `grid_break` ragges), raised rims, `grid_break` targets walls and floor.

* **An arch you can perch on**: `grid_set_heights` the ceiling above the arch (the starter's is 2,048, too low for a crown at 2,048), `grid_arch_span` between two floor points with the default proportions, `move_node` the player on its axis, `cook_check`, then `playtest` with a route (see the Playtest routes section and `crates/psxed-mcp/tests/grid_perch.rs`). Columns and rubble for the rest of the hall: `grid_place_mesh` with a recipe.
* **A platform**: `grid_set_heights` on its rectangle with `height: 512`. In a derived room the risers appear by themselves; `grid_walls` lists them.
* **A pit**: `grid_remove_surface` on the floor, leaving the ceiling. A floorless sector makes no wall at its edge, so cross it with a plane.
* **A bridge or ramp**: `grid_add_plane` with `from`, `to` (the midpoints of its two ends, grid units) and `width`, `collision: "walkable"`, `double_sided: true`. The reply says what it cooks to (a platform you stand on needs a normal leaning less than about 45 degrees).
* **A second room at another height**: `grid_add_room` with `origin` and `y_offset`, then `grid_add_surface` for its floor (and ceiling) at 0. Rooms may overlap in plan; that is how floors stack.
* **An opening in a wall**: each side of a room edge is a derived closure wall. `grid_wall_override` with `hidden: true, solid: false` on the closure of both rooms (address it by host cell, edge and `kind: "closure"`) leaves a one-sector gap you can walk through.
* **A banded wall**: `grid_band_cut` (`bottom`, `top` or `abs` anchor), then `grid_band_set` per band. Scope `room` makes the set the room default, `run` edits just that run.
* **A FIT patch**: `grid_paint` the material, then `grid_tiling` with `mode: "fit", repeats: [2, 2]`. The reply reads back "2 x 2 on this face" and warns when the cook has to cut the face.
* **The player**: `grid_height_at` for the floor under a point, then `move_node` the player entity (`Aletha (Player)` in a new project) a little above it.

## Staging and safety

Writes stage in memory. Nothing reaches `project.ron` until `save`, which refuses if the file changed on disk since the edits were staged (the editor saved over it) and snapshots the old file first. `screenshot` and `playtest` read a scratch copy of the staged document (a `.mcp-stage-<name>` directory beside the project, deleted afterwards), so they show unsaved work. `checkpoint` takes a cheap in-memory snapshot; `restore` puts the staged document back, and only a `save` writes anything. `revert` drops all staged edits.

An edit is atomic: it runs on a copy of the world and an error leaves the document untouched.

## What is not here yet

Added with the verbs they belong to, each as a wrapper over that verb's own edit function: Sculpt, Opening, Stamps, vaults, scatter (see `natural-terrain.md`). Free-standing arches are meshes now (above); arched openings in walls and vaults are not. Raise and Lower, Ramp and Stairs are here (`grid_raise`, `grid_ramp`, `grid_stairs`) with the two overlays as reads (`grid_walkability`, `grid_heatmap`). A region is a rectangle of sectors in the MCP (the editor also has a brush and a path; a path is `grid_stairs`' `path`).

## The proof

`editor/crates/psxed-mcp/tests/grid_level.rs` builds a small level from a fresh New Grid Project using only tool calls over the stdio protocol (a pit crossed by a plane bridge, a second room at another height joined by a ghosted opening and a plane ramp, a banded wall in three materials, a FIT 2 x 2 floor patch, two lights, the player), validates it, takes a checkpoint and restores it, saves, and cooks the saved file. The geometry runs in a few seconds with no extra setup. `PSXED_MCP_VERIFY=shots` also renders three screenshots; `PSXED_MCP_VERIFY=1` builds a disc and walks the route, asserting from the tracked path that the player crossed the bridge without falling and ended on the gallery floor. `PSXED_MCP_PNG_DIR` keeps the pictures.
