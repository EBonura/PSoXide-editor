# PSoXide editor architecture map (for the bonnie-32 grid port)

Read-only survey, 2026-10-16. Source tree: `/Users/ebonura/Desktop/repos/PSoXide-editor-wt/open-hall` (branch exp/open-hall-2026-10-16, base `ff9b1723` plus the experiment commits `0e64a92b`, `2a43766f`, `5b1126b4`, `92eada54`, `3eb2fb72`, `5d93e571`, `64faaa1f`, `29704ca5`). Nothing was built or run. Paths are relative to that root unless they start with `/`. Every claim carries file:line; where I only inferred something it is marked (inferred).

Conventions: "authored units" are the editor's historical scale (character 1024 tall). The cook divides every length by 16 (`editor/crates/psxed-project/src/units.rs:1-40`, `WORLD_UNIT_DIVISOR = 16`) to get engine (Quake-scale) units. One bonnie sector (1024) imports as 576 authored units (9/16, `bin/import_bonnie32_level.rs:20-29`), which is 36 engine units.

---

## 0. Headline findings

1. The editor is **egui 0.31** (`editor/crates/psxed-ui/Cargo.toml`), 40k lines in psxed-ui src alone. But the **3D viewport is not drawn by egui**. It is a texture produced by the frontend (`emu/crates/frontend/src/editor_preview.rs`, 2885 lines plus `editor_preview/*` ~4000) that runs the real PSX ordering-table/GP0 path through the emulator's software GPU. A grid world therefore needs a preview walker in the **frontend crate**, outside psxed-ui. The 2D ortho views and all overlays/gizmos are egui painter code in psxed-ui.
2. The project model has **no world document type**. The world is `Scene.brushes: Vec<Brush>` (convex half-space brushes) inside the one active scene (`scene_types.rs:902-914`). The cook hard-fails if there are no brushes: `playtest.rs:518-522` ("BSP is the sole world source"). The grid experiment never changed that. It reads a **bonnie-32 level file named by the env var `OPEN_HALL_GRID_LEVEL`** next to the BSP cook (`playtest.rs:250-287`, `brush_grid.rs:493`), so the project holds thin imported brushes and the grid pack is a side artifact.
3. The BSP is not only a renderer. At runtime it also provides **collision** (hull clipnodes), the **PVS used to activate entities/lights/logic/world objects**, **liquids** (brush contents), **doors/destructibles** (brush submodels) and **sky apertures**. Replacing the renderer is the easy 10%. See section 7.4.
4. The grid-room system was **fully deleted 2026-10-08** and is recoverable from git at `358728f2` (last tree that still had it). Only inert stubs remain in the live tree (section 8). The old model (`WorldGrid`, `GridSector`, `GridHorizontalFace`, `GridVerticalFace`, rooms, portals, floors_above) is a strong prior art for the new grid document, and bonnie-32's `Level/Room/Sector` shape is nearly the same.
5. Manny's live checkout `/Users/ebonura/Desktop/repos/PSoXide-editor` is **byte-identical to open-hall in `editor/crates/**`** except the experiment files (diff -rq, section 9). The 600+ uncommitted files are project data and artifacts, not editor source.

---

## 1. App structure

### 1.1 Crates and sizes

| Crate | Role | Size |
|---|---|---|
| `editor/crates/psxed-ui` | egui workspace: panels, tools, inspectors, 2D viewport, undo | 87,332 lines (src+tests); lib.rs 4184, `workspace/tools.rs` 6686, `model_animation_viewer.rs` 10,323, `inspector_transform_node.rs` 4850 |
| `editor/crates/psxed-project` | project model, brush kernel, BSP cook, playtest packager | 92,297 lines |
| `editor/crates/psxed-mcp` | stdio MCP server for agent authoring | 6802 lines |
| `emu/crates/frontend/src/editor_*.rs`, `playtest_disc.rs`, `embedded_playtest.rs` | window host, 3D preview renderer, texture upload, Play/disc build | `editor_preview.rs` 2885, `editor_textures.rs` 2360, `editor_assets.rs` 542, `playtest_disc.rs` 874 |
| `psxed-tex`, `psxed-gltf`, `psxed-obj`, `psxed-audio` | asset cookers (PNG/JPG to .psxt, glTF/FBX/OBJ models, WAV) | 1.9k / 6.7k / 0.7k / 1.1k |
| `engine/crates/psx-bsp` | no_std PXBSP format, renderer, collision, resident map | `render.rs` 5265, `collision.rs` 3200 |
| `engine/examples/editor-playtest` | the PS1 guest every Play builds | `bsp_runtime.rs` 2913 |

The host workspace is the repo-root `Cargo.toml` (members listed there); `sdk/`, `engine/`, `build/` are excluded nested workspaces. `psxed-ui` is embedded by `emu/crates/frontend` behind the `editor` feature (`emu/crates/frontend/src/main.rs:38-56`). The "editor" is therefore a mode of the emulator frontend, launched with `cargo run -p frontend -- --editor` (`editor/README.md`).

### 1.2 Toolkit and main layout

egui retained by one big struct: `EditorWorkspace` (`psxed-ui/src/lib.rs:601`, ~300 fields; project, selection, interaction, camera rig, caches, dialogs). One `draw()` per frame (`workspace/dialogs.rs:9-89`) draws in this order: global shortcuts, action bar (top: menus, project identity, Build/Play controls, `input.rs:819`), left dock (scene tree + filesystem split, `panels.rs:102`), right inspector (`panels.rs:1780`, or the guest performance column while Play runs), bottom content browser (resources), central viewport (`toolbars.rs:4`), then modals (New/Delete Project, unsaved changes, brush-overlap report, terrain editor, texture/model import).

Workspace tabs (`WorkspaceView`, `lib.rs:2630`): `Room` (labelled "3D", the world editor), `Ui` (HUD/menu canvas), `Animation`, `Material`. Only `Room` is world editing.

### 1.3 Viewports and camera

- 3D: `draw_viewport_3d_body` (`workspace/viewport.rs:55`). Paints the frontend-provided texture (`EditorViewport3dPresentation`, `play_mode.rs`), handles picking with CPU raycasts against solved brushes (`tools.rs:434-616`), draws gizmos and handle overlays on top. Camera rig `CameraRig` (`lib.rs:1947`) has Orbit and Free modes (`ViewportCameraMode`, `lib.rs:1875`); middle/right drag orbits, shift pans (`viewport.rs:78-91`).
- 2D: Top/Front/Side orthographic (`OrthographicView`, `viewport2d.rs:131`) with `ViewportTransform` (pan/zoom). Drawn directly with the egui painter. The 2D tool dispatch is a ~300-line inline `if` ladder inside `draw_viewport` (`toolbars.rs:100-410`), NOT behind the tool trait. The Top view is what a sector-grid UX would map onto.
- Preview parity: the preview camera, projection, packet limits mirror the playtest renderer (`editor_preview.rs:1-12` header). `editor_preview::build_phase1_frame_reusing` (`:367`) builds sky, far vista, then `walk_brushes` (`:436`, `:1377`), props, POI beacons, light gizmos, particles, model instances, bounds. It reuses `brush_compile` CSG and `brush_light` bake with caches keyed on geometry hashes (`:742-1145`, `PreviewCsgCache`, `PreviewLitCache`). Overlays (selection, grid lines, leak path) are returned as host-drawn lines so they get sub-pixel strokes (`:278-286`).

### 1.4 Modes, tools, selection, gizmos

- `ViewTool` (`lib.rs:2226`): Select, PaintMaterial, Place, Brush. The user-facing flat strip is `BspToolbarMode` (`lib.rs:1229`): Select, Draw, Paint, Face, Edge, Vertex, Clip, mapped to `BrushEditMode` (`lib.rs:1185`) and `ViewTool` by `set_bsp_toolbar_mode` (`toolbars.rs:578`); strip UI at `toolbars.rs:598`, per-mode option row at `toolbars.rs:673`.
- Place tool: `PlaceKind` (`lib.rs:2503`, 15 variants: PlayerSpawn, SpawnMarker, ModelInstance, Character, Image/Box/Cylinder/Arch props, PointLight, ParticleEmitter, Logic, Destructible, PointOfInterest, VitalityCircle, HookPoint). Clicking a surface creates the matching `NodeKind` node.
- 3D tools implement `ViewportTool3d` (`tools.rs:82`): `primary_pressed/dragged/released/clicked(&self, &mut EditorWorkspace, &ToolFrame3d)`. Dispatch is `tool_impl_3d(ViewTool)` (`tools.rs:96`) with `SelectTool`, `BrushTool`, and `PaintDispatchTool` as the catch-all.
- Selection: `SelectionState` (`lib.rs:1793`): primary node, node set, resource set, hovered brush handle, hovered entity. Brush selection is separate state on the workspace (`selected_brush`, `selected_brushes`, `selected_face`, element lists; `lib.rs:735-760`). Editing context supports TrenchBroom-style groups (`open_group`, `workspace/groups.rs`).
- Gizmos: `gizmo.rs` (214 lines) plus node gizmo in `workspace/editing.rs` (`pick_node_gizmo_handle :292`, `begin_node_gizmo_drag :552`, `apply_node_gizmo_drag :910`). `TransformGizmoMode` Move/Rotate/Scale (`lib.rs:1287`), world or local `GizmoSpace`. Brush element gizmos in `tools.rs:1247-1500`.
- In-flight strokes: one enum `Interaction` (`lib.rs:1674`: Idle, NodeGizmo, BoxSelect3d/2d, UiCanvas) so two strokes cannot coexist. Brush drags (`brush_drag`, `brush_move`, `brush_vertex_drag`, ...) are still individual `Option` fields.
- Input: `handle_global_shortcuts` (`input.rs:73`), toolbar-group cycling (`input.rs:298-455`), brush keyboard (`tools.rs:2230`). Shortcuts are suppressed while a text field owns keyboard focus (`widget_owns_keyboard_shortcuts`).

### 1.5 Undo/redo

Snapshot-based. `UndoStack` (`history.rs`) holds whole `ProjectDocument` clones, capacity 64 (`history.rs:17`), in-session only. Call `push_undo()` (`workspace/geometry.rs:1293`) once per author action, `mark_dirty()` (`geometry.rs:1229`) after any change (clears cook status, schedules the BSP leak recheck). Inspector edits coalesce through `InspectorUndoTransaction`. Consequence for a grid doc: every undo step clones the whole project (the Cathedral import is a 1.0 MB project.ron, 1039 brushes: `editor/projects/bonnie-cathedral/project.ron`), so a grid document held inside `ProjectDocument` gets undo for free; a separate large grid structure would make each snapshot copy it too (fine at these sizes, a smell at much larger ones).

### 1.6 How a tool is added (observed pattern)

1. Add a `ViewTool` variant and label (`lib.rs:2226`, `impl ViewTool` ~2480).
2. Add the strip entry in `BspToolbarMode` (`lib.rs:1229`: `ALL`, `label`, `icon`, `brush_edit_mode`) and its mapping in `set_bsp_toolbar_mode`/`active_bsp_toolbar_mode` (`toolbars.rs:563-598`), plus option widgets in `draw_bsp_context_controls` (`toolbars.rs:673`).
3. Implement `ViewportTool3d` and add an arm to `tool_impl_3d` (`tools.rs:96`).
4. For 2D, add branches inside the inline ladder at `toolbars.rs:100-410` (no trait exists); draw previews via the painter functions in `viewport2d.rs`.
5. Inspector panel for the selection: priority chain at `panels.rs:~1830` (brush, UI, resource, node). A new selectable kind needs its own early branch like `draw_brush_inspector` (`tools.rs:2327`).
6. Overlay lines for the 3D view go through `EditorViewportOverlayLine` (`play_mode.rs`), produced either in psxed-ui or in `editor_preview/overlays.rs` (1831 lines).
7. Test it through `MouseRig` (section 6).

---

## 2. Project model (psxed-project)

### 2.1 On-disk layout and format

A project is a directory (`editor/projects/<name>/`): `project.ron` (pretty RON via `ron::ser::to_string_pretty`, `document_types.rs:1128-1136`), `assets/` (textures `.psxt`, models, animations, audio), `.psxed-view.ron` sidecar for camera/visibility/layout state (`EDITOR_VIEW_STATE_FILE`, `document_types.rs:225`), `baked/` (exported discs), `.mcp-backups/` (pre-save snapshots from the MCP). There is **no schema version field**. Compatibility is by serde defaults (`#[serde(default)]` on nearly every field) plus textual migrations in `from_ron_str` (`document_types.rs:1138-1156`, `migrate_legacy_project_ron` `:1813`). `save_to_path` re-normalizes before writing (`:1159`). A load that contains grid-era node kinds is rejected: `legacy_grid_node_count` looks for `kind: Section(`, `Room(`, `Map(`, `WaterVolume(`, `Portal(` and returns `ProjectIoError::LegacyGridWorld` (`document_types.rs:1806-1811`, error at `lib.rs:495`, tests `tests.rs:3644-3682`). **A reintroduced Room kind must change that guard first.**

### 2.2 Schema (top level)

`ProjectDocument` (`document_types.rs:272`): name; editor camera/visibility/workspace/viewport starting state; BSP cook settings (`bsp_cook_mode` Draft/Release, `collision_hull_bsp`, `bsp_render_edge_repair`, `bsp_collision_leaf_target`, `bsp_solid_cell_proofs`); animation budget knobs; `scenes: Vec<Scene>` (first is active); `ui_scenes`, `scene_states`, `options`, `boot: BootTarget`, `screen_transition`; `resources: Vec<Resource>`; private `next_resource_id`. Only `active_scene()` is cooked (`playtest.rs:~509`).

### 2.3 Scene and node tree

`Scene` (`scene_types.rs:902`): `root`, private `next_node_id`, `nodes: Vec<SceneNode>` (flat arena with `parent`/`children` ids, linear `node()` lookup, `scene_types.rs:~1000`), and **`brushes: Vec<Brush>`** (`:913`, "a parallel collection for now; folds into the node tree when brushes need hierarchy"). The root is always a `NodeKind::World` node (`:35-58`) carrying `sector_size` (default 1024, `scene_grid_types.rs:1808`), `sky`, `far_vista`, `camera`, `culling` (`bsp_patch_extent`, `draw_distance`), `physics`, `world_message`. `normalize_world_root` repairs legacy `Root -> World -> Room` trees (`scene_types.rs:931`).

`NodeKind` (`scene_types.rs:16`), 25-odd variants. Spatial/gameplay ones that a grid port keeps:

| Kind | Notes |
|---|---|
| `Node`, `Group`, `Node3D` | organisation; `Group` is editor-only and flattened by cook; brushes reference a group by `Brush.group` |
| `Entity` + component children `ModelRenderer`, `Animator`, `Collider`, `CharacterController`, `Camera`, `Equipment`, `PhysicsBody`, `Interactable` | composed object. Enemies are `Entity` + `CharacterController{player:false}` referencing a `Character` resource (`scene_types.rs:178-270`; enemy behaviour is read from the Character resource at cook, `playtest.rs:479 controller_enemy_behavior`) |
| `SpawnPoint{player, character}` | player spawn (one allowed; place tool enforces) |
| `PointLight{color,intensity,radius}` | radius is **in sectors**, scaled by `World.sector_size` at cook (`brush_world.rs:2034 scene_lights`) |
| `HookPoint` | the "perch": an elevated teleport landing; the importer places Graybox Reach arch perches as `HookPoint` nodes (`bin/import_bonnie32_level.rs:388-414`) |
| `Logic{kind: TriggerVolume/Relay/Multisource/Door,...}` | triggers; linked by **node name strings** (`target`, `killtarget`, `master`), not ids (`scene_types.rs:397-424`) |
| `Destructible`, `PointOfInterest`, `VitalityCircle`, `ParticleEmitter`, `ImageProp`, `BoxProp`, `CylinderProp`, `ArchProp`, `MeshInstance` | props/gameplay. `ArchProp` horizontal size is "inherited from the enclosing room's sector size" (`scene_types.rs:155-160`) |

Every node has `transform: Transform3` (translation `[f32;3]` in authored units, euler degrees, scale; `scene_grid_types.rs:135`). Nodes sit in world space under the root; there is no per-room local space any more (rooms are gone). References to the world: positions only (no cell ids). Brush-bound doors/destructibles are the one exception: `Brush.mover: Option<NodeId>` points brushes at a `Logic Door` or `Destructible` node (`brush.rs:229-244`).

### 2.4 Resources (`ResourceData`, `resource_types.rs:3012`)

`Texture{psxt_path}` (legacy; folded into Material by `migrate_legacy_texture_resources`), **`Material(MaterialResource)`** (psxt path, blend mode, sidedness, tint, UV motion, flipbook, generated/procedural textures, sky aperture flag, secondary layer; `scene_grid_types.rs:832`), `Model`, `Skeleton`, `AnimationSource`, `AnimationClip`, `AnimationSet`, `Mesh`, `Scene`, `Script`, `Audio`, **`Character`** (movement, loadouts, enemy behaviour), `Weapon`, `Projectile`, `BoostModule`. Resources are referenced by `ResourceId` and `Resource{id,name,data}` (`:3105`). Face materials are `Option<ResourceId>` on each brush face. Materials are the texture-page unit: the cook interns `.psxt` images, applies page-local tiling promotion within an 8 KiB budget (`brush_world.rs:2124-2185`), and registers texture assets in order so PXBSP material slot N equals asset N (`playtest.rs:~690-705`).

### 2.5 World storage and versioning

Brush (`brush.rs:229`): `faces: Vec<BrushFace>` (three integer plane points + `material` + `FaceUv` + optional `edge_repair`, `:146`), `contents` (Solid/Water/Slime/Lava, `:182`), `mover`, `group`. Plane math is exact i64. Face UV is a paraxial projection with offset/rotation/Q8 scale (`:16`). Everything is integer coordinates in authored units. The kernel (`brush.rs`, 2446 lines) supplies clip, subtract, hollow, extrude, inset, vertex/face/edge move with texture lock, snap, raycast, convex-prism builders; `brush_primitives.rs` (671) the ramp/pillar/arch/stairs generators; `brush_convex.rs` convex-from-polyhedron; `brush_transform.rs` exact mirror/rotate; `terrain.rs` heightfield to closed solids. There is **no version number for the world**; "versioning" is serde-default additivity plus normalize-on-load (`normalize_loaded`, `Scene::normalize_brush_groups`).

---

## 3. World authoring today (brush/BSP pipeline)

Authoring (psxed-ui):
- Draw: two-gesture creation (footprint then height) in 3D or Top/Front/Side (`BrushCreateStage`, `lib.rs:2245`; `tools.rs:3777-3953`), shapes from `brush_primitives` (box, ramp, pillar, arch, curved wall, stairs).
- Edit: Select/Move, Face, Edge, Vertex handles (`tools.rs:1247-1500`, 2D equivalents `:4419-4980`), Clip (`tools.rs:4109`), CSG Subtract (`:4162`), Hollow (`:4225`), extrude, nudge, duplicate, snap-to-grid, repeat chain, vertex snap, groups with open/close editing.
- Surface: Paint tool, eyedropper, face UV controls with texture lock and a UV canvas (`tools.rs:2971-3453`, `workspace/uv_canvas.rs` 568 lines), justify/flip, replace material uses.
- Inspector: `draw_brush_inspector` (`tools.rs:2327`): origin, size, face plane, contents, mover, material apply.
- Other: terrain editor window (`workspace/terrain.rs` 726), overlap audit (`brush_overlaps.rs`), BSP leak pointfile overlay (`viewport.rs:345-550`, background job `lib.rs:142`).
- Prefabs: the memory note describes a "Tools > Prefabs" geometry-clipboard-with-serde system (2026-08-03), but a case-insensitive grep of `editor/crates/**.rs` finds no prefab code in either checkout (only an unrelated comment and a `res://prefabs/` test string). What exists is the **portable geometry clipboard** (`PortableGeometryClipboard`, `BrushGeometryClipboard` at `lib.rs:1506-1540`, tests `workspace/groups.rs:588`) which remaps material names across projects. Treat prefab stamping as absent; the copy/paste substrate is the nearest equivalent.

Cook (psxed-project, `brush_world::compile_brush_world`, `brush_world.rs:479`, 3900 lines; streamed variant `brush_world_stream.rs` 2137):
1. Scale authored scene to engine units (`units::scale_project_to_engine_units`, called at `playtest.rs:~505`).
2. CSG: `brush_compile.rs` (2140) turns brushes into a surface BSP (union as chained plane tests), exterior surfaces, portals (`brush_portal.rs`), leaf classification.
3. Render surface prep: rectangle merge (`brush_world.rs:1158`), subdivision to a patch extent (`bsp_patch_extent` clamp now 32..256 engine units, 576..4096 authored, per the open-hall edit), UV window split (255 texel limit, `brush_world.rs:878-1010`), seam audit (`brush_seams.rs`).
4. **Light bake** `brush_light.rs` (476): per-vertex occluded point-light bake. Inputs: `CompiledSurface`s, solid brushes as occluders, ambient, `BrushPointLight{position,radius,intensity_q8,color}` from `PointLight` nodes, `BrushMaterialTint` per material (`brush_light.rs:34-43`). Output: packed RGB per vertex. Plane-based occlusion (`brush_occluder_planes`), independent of the BSP. Reusable verbatim for any quad soup (the grid cook already does this: `brush_grid.rs:~540-565`).
5. **PVS** `brush_vis.rs` (922 + `brush_vis/cluster.rs`): Draft uses connected-component visibility, Release uses Quake portal-flow (`brush_vis.rs:1-8`). Open-hall adds an env-gated sampled-ray refinement (`OPEN_HALL_SAMPLED_VIS`, `:187-330`, flagged non-conservative in its own comment).
6. Collision: `brush_collision_hulls.rs` (1032) and `brush_region_hulls.rs` (1397, solid-leaf hull BSP) build Minkowski-expanded clipnode trees for two cooked body hulls derived from the player's Character capsule (`brush_world.rs:342-470`). Optional `solid_cell_proofs.rs`. Collision is brushes only; there is no height-query collision.
7. Pack: `brush_pack.rs` (1373) packs render records, `brush_pxbsp.rs` (1249) assembles the PXBSP container with materials, entities, brush models (movers/destructibles), liquids, sky apertures. Face cap `MAX_RESIDENT_WORLD_FACES = 6*1024` (`brush_world.rs:49`).
8. Streaming: `brush_region/` partitioner + `brush_world_stream.rs` can split a world into regions on disc (env `PSXED_STREAM_WORLD`, `playtest.rs:~688`).

Other brush-side tools: `brush_walk.rs` (player-hull traversal proof against the cooked world), `quake_map_import.rs` (geometry-only), `stream_world.rs` (stress world generator), `bin/gen_*` project generators.

---

## 4. Cook and playtest pipeline

### 4.1 Play

Play/Rebuild in the editor sets `pending_playtest_request` (`input.rs:801`); the frontend (`app.rs:3432 handle_editor_playtest_request`, `:2932 start_embedded_playtest`) then: (1) `cook_playtest_to_disk` (`psxed-ui/src/lib.rs:4004-4140`) which runs `playtest::build_package` (`playtest.rs:495`) and `write_cook_result` into `engine/examples/editor-playtest/generated/`; (2) spawns `make build-editor-playtest` (`app.rs:3083`; Makefile:935 runs `tools/build_guest_staged.sh`, cargo for the mipsel target) with features `cd-stream-bench` plus `DEFAULT_EMBEDDED_PLAYTEST_FEATURES`; (3) wraps the EXE and packs into a CUE/BIN (`playtest_disc.rs:85`, world pack/UI pack/XA music); (4) boots it in the in-process emulator in the viewport. Project Build exports the same disc (`playtest_disc.rs:511`).

### 4.2 Manifest and generated files

`write_package` (`playtest/manifest.rs:41`) writes into `generated/`: `brush_world.pxbsp`, `brush_world.pts` (leak pointfile), `grid_world.bin` (open-hall: empty unless cooked, `manifest.rs:100`), `level_manifest.cooked.rs` and `level_manifest.rs` (Rust source with `include_bytes!` statics, per the design in `playtest.rs:1-30`), `textures/`, `models/`, `sky/`, `ui_sfx/`, `xa_music/`, world/UI pack order files, stream reports. Record schemas live in `engine/crates/psx-level` (shared writer/reader types) and `playtest/schema.rs` (2055 lines; `PlaytestPxbspWorld` at `:131` with the open-hall `grid_pack` field). The guest `include!`s the manifest; `main.rs` asserts "psx-bsp is the only spatial authority" (`main.rs:~169`).

### 4.3 How the runtime picks the world renderer

There is one world path: `BspRuntime::draw` (`bsp_runtime.rs:1976`) calls `renderer.draw_pxbsp_world_from_visibility_origin`. The open-hall prototype swaps that call at compile time with Cargo feature `grid-world` (`editor-playtest/Cargo.toml:134`; `bsp_runtime.rs` diff: `mod grid_world` ~60, field `grid` ~664, load ~867, draw branch ~2039-2092). There is no runtime or manifest switch. The PXBSP is still loaded and still drives everything else.

### 4.4 The grid cook, renderer, importer (open-hall)

- Importer `bin/import_bonnie32_level.rs` (790): reads a decompressed bonnie-32 level (`Level{rooms[Room{position,width,depth,sectors[x][z]: Option<Sector>}]}`, floors/ceilings with 4 corner heights + optional second triangle, walls N/E/S/W/diagonals, per-face normal mode), emits one thin convex brush per floor/ceiling/wall surface (slab 144, wall 72 thick), a sky-aperture lid, a spawn, optional `--perch` HookPoints, Cathedral lights (hand-coded for that map), materials from a texture pack dir (`psxed_tex::convert`). Output is an ordinary brush project. Known gaps (from `build/oh-runs/grid/other-maps-survey.md`): custom UVs, per-vertex colours, props/meshes, double-sided, fog, texture packs not auto-resolved.
- `brush_grid.rs` (857): **re-reads the bonnie level file** (not the project), rebuilds faces, snaps corners to the world lattice (editor/16), dedupes shared corners, bakes corner light with `bake_brush_vertex_lighting` against the project's PointLights and solid brushes, writes pack `GRD1` v1 (header 16 words, material asset ids, vertices 8 B, cells 8 B, macro cells 8 B, quads 44 B, `brush_grid.rs:41-48`). Only faces whose texture name maps to a project Material that the BSP cook registered survive (`:513-526`); others are counted as `dropped_unmapped`.
- `psx-bsp/src/render/grid.rs` (820 new + header): the reader and `GridRenderer`: 4x4 macro-cell then cell frustum cull, backface cull before projection, corner stamp table, GTE-batched projection, textured Gouraud quad packets, near-plane fallback to the shared clipper. No portals, no PVS. Documented as "mirrored by hand" with the cooker layout (`grid.rs:~25`).
- `editor-playtest/src/bsp_runtime/grid_world.rs` (121): static `include_bytes!("../../generated/grid_world.bin")`, binds pack material slots to the VRAM bindings the PXBSP already made for the same texture asset (`:62-90`).
- Trigger: env `OPEN_HALL_GRID_LEVEL` at cook time plus `--features grid-world` at guest build time. Not in the editor UI. Not in the manifest as a typed record.

### 4.5 What collision uses

Brush hulls in the PXBSP (`psx_bsp::collision`, `CollisionHullTraceProvider`/`PxbspCollisionProvider` implementing `psx_engine::CollisionTraceProvider`, `collision_provider.rs:155,222`; trait at `engine/crates/psx-engine/src/collision_query.rs:110`). For the imported Cathedral the hulls are the thin slab brushes. The trait is the clean seam for a grid-native collision provider (height field plus wall edges); the character motor and third-person camera already consume only the trait (grid backends were deleted from both on 2026-10-08, commit `22894367`).

---

## 5. psxed-mcp

Offline stdio server (rmcp) over a project file; edits stage in memory and reach disk only on `save`, with pre-save backups in `.mcp-backups/` (`main.rs:1-12,442`). Tools (all `#[rmcp::tool]` in `main.rs`): `metrics`, `scene_info`, `plan_view` (PNG section of solid brushes to scale), geometry: `add_shape`, `add_convex_brushes`, `transform_brushes`, `make_room`, `array`, `carve`, `set_material`, `delete`, `get_brush`, `set_face_uv`, `group_brushes`, `groups`; nodes: `entity_types`, `get_node`, `place_node`, `set_node`, `move_node`, `delete_node`, `add_light`, `lights`; analysis: `audit`, `regions`, `area_budget`, `walk_test`, `materials`; build/verify: `playtest`, `screenshot` (shells out to `frontend dump-editor-preview`, `shot.rs`), `set_cook_mode`, `combat_flow`, `combat_duel`, `combat_timeline`, `set_combat_windows`; session: `status`, `save`, `revert`, `undo_last_edit`, `list_backups`. Tool bodies are pure functions of a `ProjectDocument` (`lib.rs:1-12`), so the same code could move into a live bridge.

A grid world would need: `grid_info` (extents, rooms, per-cell summary), `set_cell`/`paint_rect` (floor/ceiling heights, split direction, texture), `set_wall` (side, heights, texture, normal mode), `fill`/`carve` rectangle ops, `grid_plan_view` (the existing `plan_view` sections brushes and would be replaced by an exact cell render), `add_room`/portal-like links if rooms exist, a cell-addressed variant of `walk_test` and `audit` (open edges, unreachable cells, tile counts per view, face budget), and `place_node` that accepts a cell + height instead of world xyz. The `screenshot` and `playtest` tools only work once the frontend preview and cook understand the grid document.

---

## 6. Tests and harnesses

- psxed-ui: 513 `#[test]`, psxed-project 540, psxed-mcp 21 (grep count). Headless egui, no GPU or window.
- `ViewportHarness` (`psxed-ui/src/tests.rs:215`): builds an `EditorWorkspace` with an optional floor brush and a fixed camera/viewport, then runs the real `resolve_viewport_3d_pointer_target` so gizmo-vs-brush-vs-entity picking regressions become deterministic. Used by `tests/viewport_gizmo.rs`, `tests/brush_tools.rs`, `tests/orthographic_brush.rs`.
- `MouseRig` (`tests/interaction_rig.rs:5`): feeds real `egui::RawInput` pointer events through the real `draw_viewport_3d_body`; ~35 end-to-end gesture tests (select, handles, marquee, clip, extrude, undo). 2D equivalent in `tests/orthographic_brush.rs`.
- Direct tool-trait tests call `tool_impl_3d(...).primary_pressed(...)` (`tests/brush_tools.rs:38,1426`).
- Project crate tests: round-trip, normalization, legacy loads, cook budgets (`tests.rs` 4260 lines, `playtest/tests.rs`, `brush_region/tests*.rs`).
- Acceptance: `make editor-blank-playtest-check` (Makefile:~950) cooks a generated project, builds a disc and replays two headless emulator runs comparing GPU census byte for byte.
- CI (`.github/workflows/ci.yml`): `cargo fmt --all --check`, an explicit rustfmt check of five frontend files (`editor_assets.rs`, `editor_preview.rs`, `editor_textures.rs`, `embedded_playtest.rs`, `playtest_disc.rs`, line 17), engine fmt, `cargo check --locked --workspace --all-features`, `cargo test --locked --workspace --features frontend/editor`, engine and editor-playtest tests, then `make lint` (Makefile:277): `lint-policy-guard`, `runtime-numeric-guard`, `cargo clippy --workspace --all-targets --all-features -D warnings` for host, engine and sdk workspaces. Workspace lints: `missing_docs = warn`, `dbg_macro`/`todo` deny, `unsafe_op_in_unsafe_fn` deny (`Cargo.toml:50-59`). `psxed-project` denies `clippy::print_stdout` because the cook runs inside the MCP's stdout stream (`lib.rs:10`). `runtime-numeric-guard` (`tools/psoxide-dev/src/main.rs:254`) scans `psx-engine`, `psx-game-runtime`, `psx-level`, `engine/examples/*/src`, `sdk/crates/*/src` for float and 64-bit math; `engine/crates/psx-bsp` is not in that root list as I read it (inferred from lines 256-273), but PS1 32-bit-only is still house policy.

---

## 7. Seams for a grid world and grid editing mode

### 7.1 Document type

Recommended shape (proposal, not existing code): add a world-kind discriminator to `Scene` (for example `Scene.world: SceneWorld::{Brushes, Grid(GridWorld)}`, serde-default `Brushes` so old files are unchanged), mirroring how the prior system worked (`358728f2:editor/crates/psxed-project/src/world_types.rs:867 WorldGrid`, `scene_grid_types.rs:2830 GridSector`, `:2166 GridHorizontalFace`, `:2484 GridVerticalFace`). The data attaches to `Scene` (not `Resource`) because everything that positions into the world (nodes, lights, spawns) already lives there. `Scene.brushes` stays for non-grid props/blockers and for the interim "thin brush" collision path. Also needed: relax `legacy_grid_node_count` (`document_types.rs:1806`) if any old kind names are reused, or use new names (`GridRoom`) to avoid the guard.

### 7.2 Editing mode

- Add `WorkspaceView`-level or per-scene mode switch so the toolbar strip (`BspToolbarMode`) is replaced by a grid strip when the scene is a grid scene. Tools needed (bonnie-32 parity): select cells, paint floor/ceiling, raise/lower corners, split direction, wall draw per edge, texture paint, room extents, entity place (existing `PlaceKind` stays).
- 2D: the Top ortho view becomes the primary cell editor. The inline ladder in `toolbars.rs:100-410` should be refactored into a `ViewportTool2d` trait first (mirrors `ViewportTool3d`) so the grid tools do not extend the ladder.
- 3D: new `ViewportTool3d` impls plus CPU picking against cells/faces (a ray-grid DDA is simpler than the brush raycast in `tools.rs:434-616`).
- Inspector: new early branch in `panels.rs:~1830` for a selected cell/face, analogous to `draw_brush_inspector`. World node inspector (`inspector_transform_node.rs`) exposes `sector_size`; revisit the 1024 vs 576 decision (section 10).

### 7.3 What stays, what changes

Stays as is: asset pipeline (`psxed-tex`, `-gltf`, `-obj`, `-audio`, texture import dialog, resource browser, Material lab), Character/Weapon/Animation resources and the Animation workspace, UI workspace, scene tree, node gizmos, inspectors for all entity kinds, undo (as long as the grid lives in `ProjectDocument`), project IO, view-state sidecar, Play/disc build flow, MCP node/light/material tools, the `MouseRig`/`ViewportHarness` test style.

Must change: viewport tools and 2D painting for the world, the world inspector, scene tree rows for cells/rooms (if any), the frontend 3D preview (`editor_preview.rs walk_brushes` replaced by a grid walker; reuse the new `psx_bsp::render::grid` logic only if the preview is allowed to link guest-side code; today it links `psx_bsp::sky` etc., so linking is plausible, inferred), the cook entry (`build_package` brush-empty guard, `playtest.rs:518`), and the guest's `BspRuntime` consumers (7.4).

### 7.4 Reusable parts of the BSP pipeline for a grid

| Part | Reuse | Notes |
|---|---|---|
| Light bake `brush_light.rs` | Yes, already reused by `brush_grid.rs` | Occluders are brush planes; a grid cook needs either generated occluder brushes or a new occluder source (cell/wall slabs). Corner dedupe means one colour per shared corner: a visible lighting quality trade vs per-face lighting |
| Texture pages / materials | Yes | `resolve_materials`, `intern_texture`, page-local promotion, asset-id ordering are keyed by Material resource, independent of geometry. The grid pack just indexes asset ids (`brush_grid.rs:513`). Need the cook to register textures even when no brush uses them |
| Collision | Partly | Option A: keep generating thin brushes (works today, runs the full 3.9k-line BSP cook, hull compile cost, face budget irrelevant to render). Option B: implement `CollisionTraceProvider` for a height/wall structure (clean engine seam, `collision_query.rs:110`) and drop the hull cook for grid scenes. B is the "pure grid engine" answer but must reproduce step-up, slope, ledge, camera-trace and melee-segment behaviour that `BspRuntime` exposes (`bsp_runtime.rs:1737-1911`) |
| PVS / activation | Needs replacement | `BspRuntime` owns `activation_visibility`, `visible_points_mask`, `visible_bounds_mask`, `visible_world_objects` (`bsp_runtime.rs:625-660, 918-1130`) and the cook emits `world_object_leaf_links` (`playtest/world_object_leaf_links.rs`) and solid-cell proofs. For a grid use cell/room adjacency with simple portal flood or sampled rays; the open-hall sampled PVS (`brush_vis.rs:187`) is a stopgap |
| Entities/doors/liquids/destructibles/sky apertures | Need grid equivalents | Doors and destructibles are BSP brush submodels (`Brush.mover`), liquids are brush contents, sky is aperture-material surfaces. A grid needs per-cell/edge door and liquid flags, and a sky-open flag per cell (the grid pack already has a macro-cell sky flag, `grid.rs:82`) |
| `Scene.nodes` placement, lights, spawns, triggers | Yes unchanged | positions only |
| Streaming (`brush_region`, region pack) | Not reusable as is | grid streaming would be by room/macro-cell; the old grid-room CD streamer is in git history (salvage commit `358728f2` kept the page pool and scheduler policy in `psx-game-runtime/src/streaming.rs:5`) |

---

## 8. Remnants of the old grid room system (decision D14, deleted 2026-10-08)

Removal series (all 2026-10-08): `637cb4a4` delete room window/visibility/streaming runtime, `c81f1235` drop grid tables from cooker/manifest/psx-level, `76272822` remove grid world from project model, `22894367` delete grid collision backends from motor and camera, `57dd4303` remove grid authoring from editor UI and frontend, `5c67b01b`/`a6f4b01f` knobs and telemetry. `git diff 358728f2 a6f4b01f` removes about 22.9k lines from `editor/` and `emu/crates/frontend` and 30.4k from `engine/`, `crates/`, `sdk/`. **`358728f2` is the last tree with the grid editor, project model and runtime**; the prior doc retirement (2026-08-25) is recorded in `docs/legacy-grid-boundary.md`.

Still present in the current tree (grep for GridRoom, grid room, psxw, room streamer, WorldGrid, SectorGrid, LegacyGrid, sector):

| Path | What it is |
|---|---|
| `editor/crates/psxed-project/src/scene_grid_types.rs` (2007 lines) | misnamed survivor: live materials, UV, `Transform3`, prop sizes, `snap_height`, `snap_world_sector_size`, `DEFAULT_WORLD_SECTOR_SIZE`, `HEIGHT_QUANTUM`. Only genuinely dead item: `GridTriangleMaterialOverride` (`:109`, no uses elsewhere) |
| `psxed-project/src/document_types.rs:1806-1811`, `lib.rs:495-524`, `tests.rs:3644-3682` | `LegacyGridWorld` rejection of old kinds |
| `psxed-project/src/scene_types.rs:35-58` | `NodeKind::World{sector_size,...}` still carries the grid-era "descendant room grids" docs and `sector_size`; used for light radius and arch prop sizing |
| `psxed-project/src/playtest.rs:524,560,753` (comments), `schema.rs:64,217,1595`, `PlaytestRoom` rows (`playtest.rs:~560`) | the "PXBSP World" metadata room record that the guest still reads (`sector_size`, sky, camera, physics); `.psxw` mentioned in docs |
| `crates/psxed-format/src/world.rs` (508 lines) | the `.psxw` v5 format spec, **still compiled** (`psxed-format/src/lib.rs:52 pub mod world`) |
| `sdk/crates/psx-asset/src/lib.rs:2447` (`World<'a>` parser) and `sdk/crates/psx-asset/src/tests.rs:32,51` | still-compiled PSXW parser and pinned-version tests |
| `engine/crates/psx-level/src/lib.rs:717,3003,3519` | `.psxw`/`World::from_bytes` doc references; `MAX_ROOM_MATERIALS`, `RoomPoint`, `ROOM_*` telemetry names reused by BSP and the grid prototype |
| `engine/crates/psx-game-runtime/src/cd_stream.rs:80`, `streaming.rs:5` | salvaged page pool and scheduler from the room streamer |
| `engine/crates/psx-engine/src/world_render.rs:3`, `:234` | residual comment and vertex-colour helper |
| `editor/crates/psxed-ui/src/tests/project_workspace.rs:2101` | test named "...without_grid_rooms" |
| `tools/mkisopsx/src/main.rs:799,823,892`, `emu/crates/frontend/src/playtest_disc.rs:426,737` | disc packer still understands `room_NNN.psxw/.psxc` chunk names and writes "no PSXW chunks" placeholders |
| `editor/projects/cortex-two-area-level/project.ron` | still holds dead keys `runtime_depth_sort_mode`, `runtime_texture_split_mode`, `runtime_room_draw_order_mode`; a test confirms they are dropped on resave (`tests.rs:2713-2724`) |
| `docs/legacy-grid-boundary.md`, `world-grid-architecture.md`, `floors-plan.md`, `floors-editor-architecture.md`, `world-format-roadmap.md` | marked retired, kept as history |
| No `GridRoom`, `room_streamer`, `SectorGrid` identifiers remain (grep). No grid editing tool code remains in psxed-ui |

Related new (open-hall only) grid code: `brush_grid.rs`, `bin/import_bonnie32_level.rs`, `psx-bsp/src/render/grid.rs`, `editor-playtest/src/bsp_runtime/grid_world.rs`, `grid-world` feature, `grid_pack` in `PlaytestPxbspWorld`, `OPEN_HALL_*` env knobs (`OPEN_HALL_GRID_LEVEL`, `OPEN_HALL_SAMPLED_VIS`, `OPEN_HALL_SPLIT_BANDS`, `OPEN_HALL_FAR_WORLD`).

---

## 9. Manny's live checkout: differences and collision risks

`/Users/ebonura/Desktop/repos/PSoXide-editor` (read only, compared with `diff -rq`, no git):

- `editor/crates/**` and `docs/**`: byte-identical to open-hall, except the experiment files: `psxed-project/Cargo.toml`, `src/brush_grid.rs` (open-hall only), `src/bin/import_bonnie32_level.rs` (open-hall only), `src/brush_vis.rs`, `src/brush_world.rs`, `src/brush_world_stream.rs`, `src/lib.rs`, `src/playtest.rs`, `src/playtest/manifest.rs`, `src/playtest/schema.rs`, `src/world_types.rs`, and `psxed-ui/src/inspector_transform_node.rs`. So there is **no newer UI work** in the live checkout's editor crates; porting into psxed-ui will not collide with uncommitted UI edits.
- Engine: live differs only in `psx-bsp/src/render.rs`, `psx-engine/src/affine_surface.rs`, `editor-playtest/src/bsp_runtime.rs` (and open-hall-only `render/grid.rs`, `bsp_runtime/grid_world.rs`); live has one extra file `engine/crates/psx-bsp/examples/collision_blob_visits.rs`.
- `emu/crates/frontend/src` in live holds only the editor-owned files (`editor_assets.rs`, `editor_preview.rs` + dir, `editor_textures.rs`, `embedded_playtest.rs`, `playtest_disc.rs`), identical to open-hall's copies. The rest of `emu/`, `sdk/`, `crates/`, `tools/` in live is a different materialization (older/newer than open-hall's): many files differ or are absent, which is not editor work.
- The 600+ dirty/untracked entries are in `editor/projects/`: `graybox-reach` (626 differing entries, mostly `validation/**` (533), `validation/enemy-behavior`, `validation/zenith-*`, `source_assets/animations/player`, `baked`, `logs`, `.mcp-backups`, `.psxed-view.ron`), `cortex-two-area-level` (128), `default` (41), `graybox-valley` (`baked`, `.mcp-backups`, `.psxed-view.ron`), and untracked projects `buried-grid-blockout` (a Blender-driven BSP blockout, 281 brushes, not a grid doc), `cortex-scenery-bank`, `cortex-scenery-kit`, `palermo-mondello`, `palermo-mondello2`. The three tracked `project.ron` files have identical byte sizes in both trees (749137 / 606079 / 446451), so their differences are sidecar artifacts, not edits to those files (I did not diff contents).
- What the port must not collide with: never write into live `editor/projects/**`; do not change `Scene`/`ProjectDocument` serialization in a way that breaks loading those project.ron files (serde-default only); do not reuse the node kind names `Section/Room/Map/WaterVolume/Portal` (rejected on load); keep the open-hall bonnie projects (`bonnie-cathedral*`, `graybox-reach-live`) out of the live tree. `editor/projects/.gitignore` is tracked, so project-dir ignore rules matter when adding a grid project folder.

---

## 10. Risks and open questions

1. **Scope of "pure grid"**: replacing only the renderer (what the prototype did) leaves BSP collision, PVS activation, movers, liquids, sky as hard dependencies; a grid-native engine must re-provide the whole `BspRuntime` surface (`bsp_runtime.rs:670-2300`, ~20 methods used by playtest_runtime/update/scene/game_logic). Mitigation: introduce a `WorldRuntime` trait first, with BSP as impl one.
2. **Two sources of truth today**: the BSP cook (from thin brushes) and `brush_grid` (from the raw bonnie file) can drift; `dropped_unmapped` already shows silent divergence. The project must own the grid data, and thin collision brushes should be derived from it, never authored.
3. **Frontend preview is outside psxed-ui and has its own CI formatting rule**; the 3D grid preview and the headless `dump-editor-preview` used by MCP screenshots both need it. Without it the grid editor has no truthful 3D view.
4. **Unit and lattice choice**: bonnie sector 1024 vs imported 576 vs World.sector_size default 1024 vs 36 engine units per cell. `PointLight.radius` and `ArchProp` scale by sector_size. Decide the canonical cell size before the schema ships (Manny's earlier rule: never guess numbers; this one needs his call).
5. **Per-corner shared-vertex lighting** (grid) vs per-face lighting (BSP) is a visual change; `brush_light` occlusion needs a non-brush occluder source.
6. **Undo cost** is whole-document clone; fine at Cathedral size, revisit for big maps.
7. **2D tool ladder** has no abstraction; adding grid tools without the refactor will add another 300+ lines to `toolbars.rs`.
8. **No world version field**: shipping a second world kind via serde defaults works, but a real `world_format` marker would make tooling (MCP, importers, `cook`) fail clearly. (A `ProjectWorldFormat` with `Bsp|LegacyGrid` existed once and was removed; grep finds none now.)
9. **Prefab/clipboard**: the memory note about prefab stamping does not match the code; do not plan on reusing it.
10. **Experiment code is WIP**: commit `5d93e571` is labelled "unverified, may not build"; `cook_grid_pack_if_requested` is env-driven; the sampled PVS comment says it is not provably conservative.
11. **Bonnie-32 side**: its editor is macroquad with its own software rasterizer and custom widgets (`/Users/ebonura/Desktop/repos/bonnie-32`: `src/editor/viewport_3d.rs` 7756, `layout.rs` 6585, `grid_view.rs` 1622, `src/world/geometry.rs` 4037, `src/ui/widgets.rs` 2282; Cargo.toml macroquad 0.4). A port is a re-implementation in egui, not a lift; the data model (`Level/Room/Sector/HFace/VFace`) is the transferable part. Levels are brotli RON and must be decompressed for the importer.
