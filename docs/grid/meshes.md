# Static meshes on the grid (2026-10-17)

Branch `grid/meshes`, cut from `grid/stream-ready` 5469d7c3. Manny: "arches should not be grid geometry though, we need to introduce the concept of meshes, which we'll use as decorative elements, and also for arches." This is the audit of what existed, the model that was built, its format and cost, what the user can do, and the proof that Aletha perches on an arch made through the MCP.

Labels: **[M]** measured by a run or a test named here, **[D]** derived by arithmetic from [M] values or struct sizes, **[?]** not measured.

---

## 0. Summary

1. A **mesh** is an asset (a recipe such as an arch, or imported OBJ geometry) that the world keeps once, plus **instances** a room places (position, turn, scale, a material per slot). It is not grid geometry: no sector, wall or plane is involved.
2. **No runtime code and no format bump.** The cook turns an instance into one run of quads in the room's plane list (the same record a free plane uses), its collision proxies into the `ColPlane` records planes already have, and its hook socket into a hook-point entity. The chunk, the core and the guest draw path are unchanged, `PGL` stays version 4, and the eight reference levels and the gate fixture cook to the same bytes as before (`tools/grid-simgate/cookhash-golden.txt`) **[M]**.
3. **The arch generator** (`ArchSpec`): span, rise, spring, thickness, depth, segments and a profile (round, segmental, pointed, flat), with piers, a per-piece collision slab and a hook socket under the crown. `grid_arch_span` places one between two floor points.
4. **Perch proof [M]:** an arch made with `grid_arch_span`, the player south of it, a poll-bound route (Triangle, L2, camera up, R2, wait, Circle). The guest prints `hook:depart`, `arch:attached`, then `arch:drop`; the player hangs at y 77 under the crown and falls to the floor at y 0. Committed as `editor/crates/psxed-mcp/tests/grid_perch.rs`.
5. **Step 0** (the sim-state gate) is in `streaming-design.md`, last section: sim state is identical poll for poll between `d4f31d95` and `grid/stream-ready` on the Cathedral, Dungeon, Cave, starter and a new three-room fixture.

---

## 1. Audit: what existed

| Piece | What it is | Verdict for static meshes |
|---|---|---|
| `ModelRenderer`, Model resources, `MODEL_INSTANCES` | The character path: skinned models with joints and clips, GTE-lit per frame, 1:1 with the model cook (`psxed-gltf` builds rigged `.psxmdl` with a clip table) | Wrong tool. A column does not need joints, a clip table or a per-frame transform, and its light would be GTE lit, not the baked per-corner light the grid world uses, so it would not match the walls beside it. The model path is also the expensive one (the player alone is about 470k cycles on the Cathedral, `level-format-and-runtime.md` section 1). |
| `psxed-gltf`, `psxed-obj` | Importers | `psxed-gltf` is the rigged-model importer. `psxed-obj` converts to PSXM, a flat-colour format with no UVs and no materials. Neither yields textured quads with material slots, so a small OBJ reader was added (`grid/mesh_obj.rs`). A glTF static path is left for later (export OBJ from Blender, which is what the importer takes). |
| Box props, image props, cylinder props | Breakable boxes, billboards and cylinders with AABB collision, from scene nodes (`playtest/cook_props_lights.rs`) | Different jobs. Collision is axis-aligned boxes only. |
| `ArchProp` | A scene node with a tile-snapped round generator (span and depth 1 to 3 sectors, rise and leg and band in height quanta, four resource materials), one conservative AABB per profile segment, drawn through the world-object path | **Cooks in a grid world** (`playtest::tests::an_arch_prop_node_cooks_in_a_grid_world_with_segment_boxes`: one prop, its surfaces and at least six boxes) **[M]**; whether Play draws it in a grid world I did not run **[?]**. It cannot be reached by an agent: the grid MCP only clones nodes (`place_node`) and `set_node` keeps the variant, and the base project has none. It snaps to sectors, has one profile, rotates only about the axes the AABBs allow and takes its materials from node resources. The generator is the model for the new one (same profile idea, two polylines paired by index); the new one is not built on it because it needs arbitrary spans and yaws, profiles and a socket. |
| `HookPoint` nodes, ArchPerch | An entity kind. The runtime puts a grip marker `MARKER_HEIGHT` (96 runtime units) above the record; the body hangs below it (hand offset `[12, 81, -33]`); range 1,600 units; at most 32 hook points; it needs the ranged stance (Triangle), aim (L2 held 8 ticks) and fire (R2), a visible marker within 30 pixels of the reticle, a clear body path and a clear landing (`hook_runtime.rs`). | The perch target. A mesh socket cooks to an ordinary hook point, so none of this changed except two things below. |
| Sockets on models (`MODEL_SOCKETS`) | Hand and weapon attachment points on skinned models | Same word, different thing. A mesh socket is a point of the asset that becomes an entity. |
| Free planes | Quads off the lattice with a collision class; `GridFace`, the light bake, the preview, `PlaneRef` runs and `ColPlane` records | The mechanism reused (section 2). |

Two things in the hook code had to change to let a mesh socket be a hook point:

* **Sway by slot number.** `hooks::arch_offset(slot, tick)` moved the second and fourth hook point of a project sideways (the Graybox Reach prototype perches). A mesh socket in that position would have swayed away from its arch. The record now says it: `entity_flags::HOOK_SWAY`, set by the cook on exactly the node hooks that were slots 1 and 3, so existing projects behave identically.
* **The beacon's ruin.** The hook beacon draws a floating ruin of broken stones where nothing stands. Under a real arch it was a large dark slab across the opening. `entity_flags::HOOK_BARE` (set on socket hooks) draws only the crystal and the brackets.

---

## 2. The model

### 2.1 Why the grid's plane run

The first idea was a new `MESH_INSTANCES` section holding (asset, transform) records and an asset table in the core, drawn by transforming the asset's vertices per instance. It was rejected for this phase:

* **Light.** Baked per-corner light depends on where the instance stands (point lights, shadows, ambient). Sharing an asset's vertices means per-instance colours anyway, which is most of the data, so the sharing saves little: 16 B of colour per quad corner set against 40 B per quad.
* **Everything else is free in the grid path.** Back-face culling, near-plane clipping, texture-window repeat, the per-room material table, the room's frustum test, the cook's welding, and an identical result in the editor preview and in Play. A transform path would have to be written, tested and kept equal.
* **Streaming.** A run of quads in one chunk cannot reach into another chunk: chunk independence holds by construction (checked by `validate_full` in `grid/cook/mesh_tests.rs`).

Cost of that choice: an instance's quads are stored once per instance (section 4). When a level places the same piece dozens of times, an instanced section is worth building; the asset and instance records below are already the shape it would take.

### 2.2 Records

```
MeshAsset { name, kind: Arch(ArchSpec) | Column(ColumnSpec) | Rubble(RubbleSpec) | Imported(ImportedMesh) }
GridMesh  { id u16, asset u16, position [i32;3], rotation [i32;3] (yaw, pitch, roll degrees),
            scale [u16;3] (Q8), materials [Option<ResourceId>], fullbright, collide, enabled }
```

* The world keeps `mesh_assets`, a room keeps `meshes` (both serde-defaulted and omitted when empty, so an old project loads and saves unchanged).
* Frames: the asset's origin is the floor under the middle of its base, `+x` right, `+y` up, `+z` front, all grid units. An instance scales per axis, turns with the plane convention (roll about z, pitch about x, yaw about the vertical) and lifts to `position` (`x` and `z` on the world lattice, `y` relative to the room). Every corner is rounded once, so the corners of one instance weld exactly.
* `BuiltMesh` is what an asset builds: faces (rings of 3 or 4 corners, texels, a material slot), proxies, sockets, slot names. Generators are deterministic (the rubble hash is xorshift32) and checked by tests.
* An asset equal to one the world already has is reused (`add_asset`), so ten copies of a column are one asset.
* **Stable ids.** An instance id is `max + 1` over the world and is never reused while the instance lives; deleting never renumbers. The MCP, the editor and the sockets name an instance by it. A hook socket cooks to a hook entity in room-then-instance order after the node-authored hook points, so those keep their slots. (Entities have no stable-id table yet; the streaming design's per-unit spawn section is where it belongs.)

### 2.3 The cook

* **Render.** `mesh_faces_with` turns an instance into `GridFace`s of kind `Mesh`. The cook adds them after the room's planes with the `QUAD_PLANE` flag and one `PlaneRef` per instance (box: the instance's sectors clamped to the room, its height range). The runtime draws it through the plane list. Light uses the plane rule: a shadow ray starts 2 units off the surface; `fullbright` shows the tint unlit. Tiles and texels go through the same rebasing and tile cutting as every face (`uv_split`), so the editor preview draws what Play draws.
* **Collision.** A proxy becomes records in the room's plane collision table:
  * `Box` (piers, columns, imported boxes): one **blocker** when the instance is upright and a quarter turned, four vertical **walls** (exact at any yaw) plus a **platform** top when the box is standable or turned off a quarter, the bounding box (with a warning) when pitched or rolled;
  * `Slab` (the arch's extrados pieces, thickness the band's): a **platform** when it faces up within the walk slope, a **wall** when upright, else a **blocker** box. The two haunch pieces of a round arch land there; the cook says "N slab(s) lean too far for a wall and not flat enough for a platform and block as boxes, which over-blocks the space under them", once per instance. A body 48 units tall under an arch whose springing line is above that is unaffected **[M]** (`a_body_walks_through_the_opening_and_stops_at_a_pier`).
  * Record ids continue after the room's planes (`u8`, 255 per room including planes).
* **Sockets.** `Socket::Hook { anchor, yaw }` becomes a `HookPoint` entity 96 runtime units under the anchor (`HOOK_MARKER_HEIGHT_RU`, pinned to `hook_points::MARKER_HEIGHT` by a test in the runtime crate). The arch anchors 32 grid units (2.5 runtime units) under the soffit: the runtime traces the view to the grip and a point on the face of the crown slab counts as inside it, which made the hook unavailable at 0 **[M]** (the first perch attempt, before the sink).
* **Limits and checks** (`limits::validate`): at most 128 instances per room, 255 assets, 255 faces per instance, 8 material slots, unique ids, scale 1/8 to 8, a missing or unbuildable asset is an error, an instance reaching outside its room is a warning (an editor edit always grows the room, as for planes).

### 2.4 Editor and MCP

See `editor-guide.md` (Mesh tool, Shift+N, the Meshes palette and Inspector) and `mcp-guide.md` (`grid_meshes`, `grid_place_mesh`, `grid_set_mesh`, `grid_delete_mesh`, `grid_arch_span`). Both call `psxed_project::grid::edit::meshes`, so a mesh made with a mouse and one made through the MCP come from the same functions.

Key: every plain letter is taken or reserved (N Plane, L Link and R turn are in use, S B K I Y G reserved), so the Mesh tool is the chord **Shift+N**, the solid sibling of the Plane tool's N. `GridTool::shift()` carries it and the uniqueness test compares key and shift.

---

## 3. Fewest bits: records

Cooked, an instance has no record of its own (it is quads), so the rule applies to what is stored and to the quads it makes. For the stored records, with the widths the real ranges need (a room is at most 128 sectors, the lattice 64 grid units, 24 headings at 15 degrees):

| Field | Document (RON, the editor's file) | Minimum bits | If packed (chosen) |
|---|---|---|---|
| `GridMesh.id` | u16 | 7 (128 per room) but must stay unique world-wide | 16 |
| `asset` | u16 | 8 (255 assets) | 8 |
| position x, z | i32 | 11 each (128 sectors at the 64 lattice), 17 free | 11, 11 |
| position y | i32 | 11 (fine units) | 11 |
| yaw | i32 degrees | 5 (24 headings), 9 for whole degrees | 5, or 9 when off the 15 degree lattice |
| pitch, roll | i32 | 0 when upright, 5 each otherwise | 1 flag, then 5, 5 |
| scale | `[u16;3]` Q8 | 11 per axis, 1 flag when uniform | 1 + 11, or 33 |
| flags (collide, fullbright, enabled) | three bools | 3 | 3 |
| material per slot | `Option<ResourceId>` (u64) | 3 (the room's `MATERIAL_USE`) | 3 per slot |

A typical upright, uniformly scaled instance with two slots is 16 + 8 + 11 + 11 + 11 + 5 + 1 + 1 + 12 + 3 + 6 = **85 bits (11 bytes)** against about 56 bytes of RON-less struct and several hundred of RON text. Nothing reads this form at runtime today; it is what an instanced section would store.

Asset records (the recipe, minimum bits at the 64 grid unit lattice the recipes use):

| Asset | Fields | Bits |
|---|---|---|
| Arch | profile 2, span 7, rise 7, spring 7, thickness 5, depth 6, segments 3, perch 1 | **38** |
| Column | radius 4, height 8, sides 4, plinth 5, capital 5 | **26** |
| Rubble | size 6, pieces 3, seed 16 | **25** |
| Imported | per point 3 x 12 (bounds under 4,096), per face 4 indices at `ceil(log2 points)`, 1 count bit, 3 slot bits, 8 bits per texel pair per corner | stored today as 48 per point and 144 per face; packed about 36 and 60 |

Cooked cost per instance, measured by `cook::mesh_tests::what_a_mesh_costs_in_the_chunk` (bytes added to a room chunk, the instance in an otherwise empty hall) **[M]**:

| Asset | Quads | Total | Quads | Vertices | Collision copies | Sector exts | Plane ref |
|---|---|---|---|---|---|---|---|
| Arch (4 segments per half, 10 collision records) | 40 | **2,926 B** | 1,600 | 352 (44 x 8) | 896 (32 x 28) | 64 | 12 |
| Column (8 sides) | 19 | **1,622 B** | 760 | 256 | 560 | 32 | 12 |
| Rubble (4 stones) | 40 | **1,965 B** | 1,600 | 352 | 0 | 0 | 12 |
| Imported crate (6 faces, box top) | 6 | **685 B** | 240 | 64 | 336 | 32 | 12 |

Two things stand out. Collision is a third of an arch (a record is copied into every sector it touches: 32 copies of 28 bytes), which a shared per-room record list would fix, and the plane ref at 96 bits holds a quad index (16), a count (8), flags (8), four cell bytes (32) and a height range (32) where about 40 bits would do. Both belong to the plane format, not to meshes, and are worth doing together with the streaming layout.

Render cost is **[?]**: per frame the renderer tests one box per instance, then draws the visible quads at the grid's per-quad cost (741 cycles per quad measured on the Cathedral); no frame was timed for meshes, per the brief.

---

## 4. Constraints from the streaming design (section 4 of `streaming-design.md`)

| Rule | Status |
|---|---|
| Instances stay inside one stream unit | An instance belongs to the room that holds it; editing grows the room to hold it; the validator warns when an instance reaches outside |
| Chunk independence | A run references only its own vertices and the room's material table; `validate_full` passes in the tests |
| Stable ids | Instance ids as above; sockets cook in a fixed order |
| Fewest bits | Section 3 |
| Materials per unit | Slots resolve through the room's own `MATERIAL_USE` (the same path as every face) |
| `Room` field meaningless on entities | Socket entities are written with room 0 like every point entity |

---

## 5. Proof

* **Perch (headless Play).** `editor/crates/psxed-mcp/tests/grid_perch.rs`. The geometry stage always runs (cook, hook entity at the arch centre 96 under the grip, save and reload); with `PSXED_MCP_VERIFY=1` it plays the route and asserts the guest's `arch:attached` and `arch:drop` lines and the tracked path. The scenario, made with MCP calls only: ceiling to 4,096, `grid_arch_span` from `[2048, 0, 2048]` to `[4096, 0, 2048]` (a 2,048 span, rise 1,024, spring 1,024, band 256, depth 1,024), the player 240 runtime units south on the arch's axis. Route: Triangle (ranged stance, 72 ticks to swap), L2 held, the right stick up for 44 polls (camera pitch 205 to -191, which puts the crown at the reticle), R2, wait, Circle, wait. Result **[M]**: `hook:depart landing=000000FC 0000004D 0000007F`, `arch:attached`, the player at `(252, 77, 127)` hanging, `arch:drop` after Circle, the body back on the floor (`y 0`, grounded). Frames: `build/grid-editor/progress/mesh-*.png`.
* **Arch collision.** `cook/mesh_tests.rs`: a body walks through the opening along the arch's axis and out the far side at floor height, stops at the outer face of a pier, a body dropped over the crown stands on the extrados at 220 units, a quarter-turned arch collides where it is drawn.
* **Decor.** A column (blocks, standable capital at 200 units), rubble (draws, no collision) and an imported OBJ crate (blocker box with a standable top) cook and collide as listed.
* **Reference levels unchanged.** Cook hashes of the Cathedral, level008, Dungeon, Cave, sewers, west, starter and the gate fixture are identical before and after (`cookhash.sh`), and the sim-state gate matches on all of them.
* **Tests added:** 45 in `psxed-project` (generators, faces, edits, cook, collision, costs), 3 in `psxed-mcp` (tools) plus `tests/grid_tools.rs::meshes_are_generated_placed_read_changed_and_cook` and `tests/grid_perch.rs`, 10 in `psxed-ui` (`tests/grid_meshes.rs`), and one each for the camera pin, the hook marker height, hook events and the ArchProp audit.

---

## 6. Noticed, not fixed (separate issues)

* The third-person camera's solve and lift-check throttle is anchored to its own first update, so camera poses depend on load time (`streaming-design.md`, last section).
* `room_index` in the guest is constant 0 whatever room the player is in, and entity records carry room 0; hooks work because both are 0. A multi-room world needs the room of a hook point to mean something before hooks are authored outside room 0 (`hook.room != self.room_index` would skip them).
* `hook_signposting`'s beacon is drawn per hook point in the project with no culling by room or distance beyond 2 x range; with many arches it is the first thing to measure.
* `PlaneRef.count` is a `u8` and `ColPlane.id` a `u8`: an instance with more than 255 quads after tile cutting, or a room with more than 255 collision records (planes and proxies together), is a cook error. A shared record list would lift the second.
* The plane collision `blocker` over-blocks leaning pieces; a small oriented-box record would make haunches exact.
* `ArchProp` and the new arch are now two ways to make an arch. ArchProp should be retired for grid worlds once meshes cover its cases (tile-snapped and half arches are `span` and the halves are two instances).
