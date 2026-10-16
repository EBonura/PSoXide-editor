# bonnie-32 world/level editor: porting inventory

Source: `/Users/ebonura/Desktop/repos/bonnie-32` on branch `main` (HEAD 2f1607b, "Mark BONNIE-32 as discontinued", 2026-05-18). All paths below are relative to that repo unless absolute. Read-only investigation: nothing was built or run. Anything marked "inferred" was not executed.

Which code is "the editor": `main` is the finished v1 (macroquad, custom immediate-mode UI, v0.1.12). The `v2` branch (wgpu + egui, 2330-line `src/editor/world_editor.rs`, last commit 2026-03-23) is an unreleased partial rewrite; `CLAUDE.md` on main describes v2 (stale for main). Everything here describes v1 on main, which is the complete one.

Size: `src/editor/viewport_3d.rs` 7756 lines, `layout.rs` 6585, `texture_palette.rs` 2036, `grid_view.rs` 1622, `state.rs` 1393, `level_browser.rs` 846, `src/world/geometry.rs` 4037, `world/level.rs` 486, `src/ui/*` about 6400 (widgets.rs 2282). Editor + world history: 289 commits touching `src/editor src/world` between 2025-12-04 and 2026-02-10 (about 10 weeks).

---

## 1. Data model

All of it is in `src/world/geometry.rs` unless noted. Level files are RON (serde). Coordinates: X/Z horizontal, Y up. `SECTOR_SIZE = 1024.0` (geometry.rs:10). Height unit "click" `CLICK_HEIGHT = 256.0` (editor/state.rs:105), default ceiling `CEILING_HEIGHT = 3072.0` (state.rs:107). All heights are `f32` world units, room-relative (the room has a world `position`).

### Level (geometry.rs:3443)
```
Level {
  rooms: Vec<Room>,
  editor_layout: EditorLayoutConfig,   // serde(default): panel split ratios, 2D zoom/pan, orbit cam (3357)
  player_settings: PlayerSettings,     // serde(default)
  skybox: Option<Skybox>,              // serde(default)
}
```
There is no `version`, `name`, `spawn`, or `lights` field at level scope. A level is a Vec of rooms. The player start is an object instance whose asset has a `SpawnPoint{is_player:true}` component (`get_player_start`, geometry.rs:3473).

### Room (geometry.rs:2437)
```
Room {
  id: usize,
  position: Vec3,                       // world origin of the sector grid (Y is a per-room vertical offset)
  width: usize, depth: usize,           // grid dims (X count, Z count)
  sectors: Vec<Vec<Option<Sector>>>,    // indexed [x][z] (column-major), None = no sector
  portals: Vec<Portal>,                 // serde(default); auto-computed, but saved
  bounds: Aabb,                         // serde(skip), recomputed on load
  ambient: f32,                         // default 0.5, 0..1, UI shows 0-31
  objects: Vec<AssetInstance>,          // per-room placed objects (spawns, lights, props...)
  fog: RoomFog,
}
```
Validation limits (world/level.rs:15-26): MAX_ROOMS 256, MAX_ROOM_SIZE 128, MAX_WALLS_PER_EDGE 16, MAX_COORD 1e6, MAX_STRING_LEN 256. Diagonal wall vectors are not count-checked in `validate_sector` (level.rs:117-146 only checks the four cardinal edges). Rooms are sparse grids with auto-grow in all four directions (see 2.x) and `Room::trim_empty_edges` (geometry.rs:2700) / `compact` (2690) shrink them while adjusting `position` and object `sector_x/z`.

### Sector (geometry.rs:1499)
```
Sector {
  floor:   Option<HorizontalFace>,      // None = pit/no floor
  ceiling: Option<HorizontalFace>,      // None = open sky
  walls_north: Vec<VerticalFace>,       // stacked: several segments per edge (-Z)
  walls_east:  Vec<VerticalFace>,       // +X
  walls_south: Vec<VerticalFace>,       // +Z
  walls_west:  Vec<VerticalFace>,       // -X
  walls_nwse:  Vec<VerticalFace>,       // diagonal wall NW corner to SE corner
  walls_nesw:  Vec<VerticalFace>,       // diagonal wall NE corner to SW corner
}
```
So: floor and ceiling are independent faces (a sector may have either or neither); walls are explicit per-edge stacks (not derived from height differences); there are two diagonal wall lists. The wall tool places at most 3 per edge (`next_wall_position`, geometry.rs:1630, `if walls.len() >= 3 return None`), validation allows 16. Edges are not shared: the wall between two adjacent sectors is stored on one or both sides independently (portal detection treats a wall on either side as blocking, geometry.rs ~3740).

### HorizontalFace (floor/ceiling, geometry.rs:1104)
```
heights: [f32;4]                 // corners [NW, NE, SE, SW]: this is the per-corner slope data
split_direction: SplitDirection  // NwSe | NeSw  (which diagonal splits the quad into 2 triangles)
texture: TextureRef              // triangle 1 (and 2 unless overridden)
uv: Option<[Vec2;4]>             // None = world-aligned default
colors: [Color;4]                // per-corner vertex colour (r,g,b,blend); 128 = neutral
texture_2: Option<TextureRef>    // triangle-2 overrides, all skip_serializing_if None
uv_2: Option<[Vec2;4]>
colors_2: Option<[Color;4]>
heights_2: Option<[f32;4]>       // independent heights for tri 2 ("split-level" faces)
walkable: bool                   // default true
blend_mode: BlendMode            // Opaque|Average|Add|Subtract|AddQuarter|Erase (rasterizer/types.rs:1380)
normal_mode: FaceNormalMode      // Front | Both | Back  (double-sided = Both)  (geometry.rs:1040)
black_transparent: bool          // default true: RGB(0,0,0) texels are transparent (PS1 CLUT style)
```
Triangulation: NwSe gives tri1 = NW,NE,SE and tri2 = NW,SE,SW; NeSw gives tri1 = NW,NE,SW and tri2 = NE,SE,SW (`SplitDirection`, geometry.rs:1059-1100). Height sampling for collision uses the same triangle split (`interpolate_height`, geometry.rs:1283). Slopes are therefore not a separate concept: a slope is just unequal corner heights. `diagonal_matters()` (1232) decides when the 2D view draws the split line.

### VerticalFace (wall, geometry.rs:1355)
```
heights: [f32;4]   // [bottom-left, bottom-right, top-right, top-left], per-corner so walls can be trapezoids/triangles (slanted)
texture, uv: Option<[Vec2;4]>, solid: bool (default true), blend_mode,
colors: [Color;4], normal_mode, black_transparent,
uv_projection: UvProjection   // Default | Projected (world-Y-aligned V, for sloped walls; new walls default to Projected)
```
Left/right is as seen from inside the room, so corner order differs per edge (render.rs mapping at geometry.rs:3051-3235).

### Other leaf types
- `TextureRef { pack: String, name: String }` (geometry.rs:22). Pack `"_USER"` = RON paint textures (`USER_TEXTURE_PACK`, geometry.rs:18). Empty = checkerboard fallback. Texture refs are by name strings, not ids. 64x64 PNG packs in `assets/samples/texture-packs/*` (9 packs); user textures are indexed-colour RON (`assets/samples/textures/texture_00N.ron`, `src/texture/user_texture.rs`).
- `Color { r,g,b: u8, blend: BlendMode }` (rasterizer/types.rs:721). Custom `Deserialize` accepts the old `a:` alpha field (types.rs:728-760). `Color::NEUTRAL = 128,128,128` (types.rs:771).
- `Vec2/Vec3` rasterizer math types, serialized as `(x:..,y:..)` structs.
- `Direction { North, East, South, West, NwSe, NeSw }` (geometry.rs:2067) with `rotate_cw` cycling N,E,S,W,NwSe,NeSw.
- `Portal { target_room, vertices:[Vec3;4] (room-relative), normal }` (geometry.rs:2369). Auto-generated only (see below).
- `RoomFog { enabled, color:(f32,f32,f32), start, falloff (alias "end"), cull_offset }` (geometry.rs:2403). Default start 8192, falloff 30000, cull_offset 9000. In saved samples falloff/start are sometimes small (West.ron: 500/1500).
- `PlayerSettings` (geometry.rs:2177): radius 300, height 1800, step_height 384, walk 3000, run 5000, gravity 2400, jump_velocity 1200, sprint_jump_multiplier 1.15, camera_distance 6000, camera_vertical_offset 2000, pitch min/max +-0.8, legacy camera_height. Edited in the editor when a player-start object is selected (layout.rs ~5890-5990) and used by the Test tab.
- `Skybox` (geometry.rs:321): 4 gradient colours (zenith, horizon sky, horizon ground, nadir), `horizon`, optional horizontal tint (direction/intensity/spread), sun+moon (`CelestialBody`), 2 cloud layers, 2 mountain ranges, star field, horizon haze, presets (sunset, twilight, arctic, night; geometry.rs:749-1020). All `#[serde(default)]`. Procedurally rendered by `Skybox::generate_mesh` / `Framebuffer::render_skybox`. Editor UI is the left "Skybox" collapsible panel (layout.rs:1880-2720, ~840 lines). BACKLOG says it needs polish.

### Objects, lights, spawns, triggers: asset instances
Objects are not typed in the level any more. `AssetInstance` (geometry.rs:2289):
```
AssetInstance { sector_x, sector_z: usize, height: f32 (offset above sector floor avg), facing: f32 (yaw radians, 0 = +Z),
                asset_id: u64, name: String, enabled: bool, overrides: ComponentOverrides (skip if empty) }
ComponentOverrides { light: Option<LightOverride{ color, intensity, radius, offset: all Option }> }
```
What an object *is* comes from `AssetLibrary` lookup by `asset_id` (`src/asset/*`; assets are authored in the Modeler tab). `AssetComponent` (asset/component.rs:17-175) variants: Mesh, Collision (shape/trigger/collision_mesh), Light{color [u8;3], intensity, radius, offset}, Trigger{trigger_id,on_enter,on_exit}, Pickup, Enemy, Door, Audio, Particle, CharacterController, SpawnPoint{is_player,respawns}, Skeleton, and more. So lights, spawns, props, enemies, triggers are all "an asset with a component", positioned on a sector with a height offset and yaw; position is sector-centre only (no sub-sector XZ). World position = room origin + sector centre + (floor avg height + `height`) (`world_position`, geometry.rs:2353). Exactly one player start is enforced in the placement tool (viewport_3d.rs ~1086-1100, grid_view.rs ~1473-1483).

Lights are not baked: `scene::collect_scene_lights` (scene.rs:33) turns enabled instances with a Light component into runtime point lights and rasterizer shading evaluates them per vertex (`shade_multi_light_color`, rasterizer/render.rs:1013). There is only one fixed directional light in default `RasterSettings` plus per-room ambient.

### Portals
`Level::recalculate_portals` (geometry.rs:3657) clears and recomputes portals between every room pair (O(rooms^2 x cells x 4)). Vertical portals: adjacent sectors at matching grid alignment, no wall on either side, both have floor and ceiling; portal quad is max(floors) to min(ceilings) per corner (trapezoids for slopes). Horizontal portals: one room's ceiling exactly equal (within 1.0) to the other's floor at all 4 corners (geometry.rs:3877). Called from `main.rs:417-419` whenever `portals_dirty` is set (geometry edits call `mark_portals_dirty`). Manual portal creation was removed (`git log`: "remove Portal button", 2025-12-21); the `portal.create` action in editor/actions.rs:149 is registered but never wired. IMPORTANT: portals are never read by the game runtime or renderer (grep for `portal` finds only editor/world/main/landing); visibility is not portal-culled, rooms are all rendered with fog distance culling (`scene::render_scene`, scene.rs:158-200) and room membership uses `Room::bounds` AABB containment (`Level::find_room_at`, geometry.rs:3566). Portals are drawn in the 2D and 3D editor views and serialized (Dungeon.ron has 90).

### On-disk format and versioning
- RON via `ron::ser::to_string_pretty` with `depth_limit(4)`, then Brotli quality 6, lgwin 22 (`save_level`/`serialize_level`, world/level.rs:194-215, 420-441). Loader auto-detects plain RON (first byte `(`, space, newline, tab) vs Brotli (level.rs:127-135).
- Typical shape (from `assets/samples/levels/West.ron` after decompress): `(rooms: [(id:0, position:(x:-2048.0,y:0.0,z:-5120.0), width:8, depth:8, sectors:[[None,None,None,Some((floor: Some((heights:(0.0,0.0,0.0,0.0), split_direction: NwSe, texture:(pack:"dark-military-fantasy",name:"city_bank_brick"), uv:None, colors:((r:128,g:128,b:128,blend:Opaque),...x4), walkable:true, blend_mode:Opaque, normal_mode:Front, black_transparent:true)), ceiling:None, walls_north:[(heights:(0.0,0.0,2048.0,2048.0), texture:(...), uv:Some(((x:0.0,y:0.0),...)), solid:true, blend_mode:Opaque, colors:(...), normal_mode:Front, black_transparent:true, uv_projection:Projected)], walls_east:[], walls_south:[], walls_west:[], walls_nwse:[], walls_nesw:[])), ...]], portals:[], ambient:0.5, objects:[...], fog:(enabled:false,color:(0.5,0.5,0.5),start:500.0,falloff:1500.0,cull_offset:0.0))], editor_layout:(...), player_settings:(...), skybox: None)`. Sectors are a column-major `Vec<Vec<Option<Sector>>>`, so files are verbose: decompressed 39 KB to 493 KB, Brotli 1.3 KB to 4.2 KB (assets/samples/levels, sizes checked by `brotli -d`).
- Versioning: none. No version field anywhere in `world/`. Compatibility is done ad hoc with `#[serde(default)]` on later-added fields, `alias = "end"` on `RoomFog.falloff`, the `Color` deserialize shim (old alpha), and `retain(|obj| obj.asset_id != 0)` after load to drop pre-asset-system objects (level.rs:297-299, 340-343, 397-400, 442-444; geometry.rs:2304). Consequence found in the samples: `Cave.ron` and `West.ron` still contain old `object_type: Light(...)` / `Spawn(PlayerStart)` objects (3 each, no `asset_id`); the current struct ignores the unknown field and the loader strips them, so those two samples load with no lights and no player start (inferred from the code, not run). Only `Sewers.ron` and `level_008.ron` have `asset_id` objects. Both the editor sample set and the scratch copies in `PSoXide-editor-wt/.../scratch/levels/` are byte-for-byte the same size as a fresh decompress, so they are the same data.
- `-0.0` values appear in saved heights (e.g. West.ron `heights: (-0.0, -0.0, ...)`), a harmless float quirk to normalise on import.
- The loader code is copy-pasted four times (load_level, load_level_from_str, load_level_with_storage, parse_level_data) with the same validation tail. Worth collapsing if ported.

---

## 2. Editing UX

### 2.1 Overall layout (screenshot: `docs/screenshot-world-editor.png`)
Single window. Top: one 36 px toolbar. Left sidebar (25% default, draggable `SplitPanel`): four collapsible panels stacked, **Skybox**, **2D Grid** (with T/F/S view buttons + "center on room"), **Rooms** (list with show/hide eye, delete, "+ Add Room", then room properties: pos, size, sectors, portals, lights, Ambient slider 0-31, Fog checkbox + colour RGB, Start, Falloff, Cull+ sliders), **Debug** (frame timings, memory; collapsed by default). Centre: **3D Viewport** (software-rasterised framebuffer drawn as a texture, title turns cyan when focused). Right: **Textures** and **Properties** collapsible sections sharing height. Bottom: 22 px status bar with transient message (green) on the left and context-sensitive shortcut hints on the right (layout.rs:6243). Layout split ratios, 2D pan/zoom and the orbit camera are saved inside the level file (`EditorLayoutConfig`, geometry.rs:3357; `EditorLayout::to_config`, layout.rs:84).
Entry: `draw_editor` layout.rs:231.

### 2.2 Tools and modes
Tool enum `EditorTool { Select, DrawFloor, DrawWall, DrawCeiling, PlaceObject }` (state.rs:126). Keys 1 to 5 (in 3D viewport and in 2D view, viewport_3d.rs:388-398, grid_view.rs:1603). Toolbar order: Select (move icon), Floor, Wall, Ceiling, Object, then an asset picker "< asset >" with arrows and a browse button (opens a modal Asset Browser, layout.rs:~660-700).
There are no modal "geometry vs texture vs object" modes; selection kind drives what Properties shows. The BACKLOG still lists an "Environment/Objects switch" as unbuilt.

Selection kinds (`Selection`, state.rs:188): `None`, `Room`, `Sector{room,x,z}` (only via 2D view rect select), `SectorFace{room,x,z,face}` (3D face click), `Vertex{...,corner_idx}`, `Edge{..., face_idx, edge_idx, wall_face}`, `Portal` (type exists, not pickable in practice), `Object{room,index}`. `SectorFace` is `Floor | Ceiling | WallNorth(i) | ... | WallNeSw(i)` where `i` is an index into the wall Vec (state.rs:154), so deleting a wall renumbers the others (design smell). `multi_selection: Vec<Selection>` holds extras; primary + multi is the working set.

### 2.3 3D viewport behaviour (all in `draw_viewport_3d`, one 5360-line function, viewport_3d.rs:294-5650)
Picking: purely screen-space, in framebuffer pixels. Candidates are vertices (6 px threshold), edges (4 px), faces (quad hit with depth), objects (12 px); depth-based tie breaking, priority vertex > edge > face (`find_hovered_elements`, viewport_3d.rs:7028-7410; thresholds at 7038-7039, 7341). Only the **current room** is pickable for vertices/edges (`state.current_room`); rooms are switched through the Rooms panel list or by objects/other panel clicks. Hover highlights faces/edges/vertices drawn straight into the framebuffer.
Select tool gestures:
- Click vertex / edge / face: select; click selected one then drag: edit. Left-click empty space: box select (rectangle in framebuffer coordinates with live preview highlight; `find_selections_in_rect` 7512).
- Drag selected vertex/edge/face while **Shift held at press**: raise/lower in Y only. Mouse delta times 5.0, snapped to `CLICK_HEIGHT` (256) per vertex (viewport_3d.rs:2195-2260). Moves every vertex of every selected face/edge/vertex, and (with Link on, default) every coincident vertex in **any room** at the same world position (epsilon 0.1), so room seams and wall/floor corners stay welded.
- Drag selected **face without Shift**: relocate in X/Z, snapped to whole sectors, with ghost preview; destination collision check, room auto-expands, empties trimmed (`relocate_faces` 6768, `is_destination_occupied` 6902; viewport_3d.rs:2155-2193, 2680-2790).
- Shift+click (range select): vertices/edges on a line between two picks; floors/ceilings select the bounding rectangle of existing selection plus the click; walls select a connected wall path via BFS (`find_wall_path`, viewport_3d.rs:109). Ctrl/Cmd+click toggles one item. Click on unselected item replaces selection; click on already-selected keeps multi-selection so it can be dragged.
- Selecting a face auto-scrolls the texture palette to that face's texture (`scroll_to_texture`, state.rs:1262).
- Delete / Backspace: delete selected faces or objects; then `Room::compact` and portals dirty (viewport_3d.rs:513-600; `delete_face` 5654).
- `.` centres camera on selection. `Esc` clears selection and geometry clipboard. `Ctrl/Cmd+A` selects every floor/ceiling/cardinal wall in the room (note: diagonal walls are omitted, viewport_3d.rs:441-505).
Floor / Ceiling tools: ray-plane hit against the room's floor plane picks the sector (`pick_plane`), a teal hover grid is drawn; left-drag defines a rectangle of cells; release creates floor (default Y 0) or ceiling (default Y 3072) for every cell that does not already have one, texture = currently selected texture. **Hold Shift** to enter height-adjust: position locks, mouse Y changes `placement_target_y` in 256 steps (state field persists between placements). The room grid grows in -X/-Z/+X/+Z as needed, shifting `position` and object `sector_x/z` (viewport_3d.rs:2494-2620). Placement completes with `mark_portals_dirty`. Single undo snapshot per placement.
Wall tool: `R` cycles direction N,E,S,W,NW-SE,NE-SW (viewport_3d.rs:375-379, status bar shows it); `F` toggles "prefer high/low gap" (381-385). Preview: the sector under the cursor with the chosen edge highlighted; the wall's vertical extent comes from `Sector::next_wall_position` (geometry.rs:1630-1820), which fills the largest or mouse-selected gap between floor, ceiling and existing walls on that edge, following slanted floors/ceilings (it offers triangular gaps), max 3 per edge. Left-drag along an edge line places a run of walls (axis-locked; diagonals lock to 45 degrees via grid dx/dz, viewport_3d.rs:2470-2540). Diagonal walls can only be placed from the 3D view (2D shows "Diagonal walls: use 3D viewport"). Fully covered edges give "Edge is fully covered".
Object tool: hover shows a wireframe of the asset snapped to the sector centre at floor height; click places, with unique-player-start enforcement. Click an existing object to select and drag it in XZ (snapped to sectors, click offset preserved) or Shift+drag for Y (`height`, 256 snap); Shift can be pressed mid-drag to switch modes (viewport_3d.rs:1226-1370).
Copy/paste:
- `Ctrl/Cmd+C` on an object copies it; on one face copies its properties (texture, uv, colours, normal mode, blend, walkable/solid, split dir, tri2 data; **not** heights) into `FaceClipboard` (state.rs:221); paste applies to all selected compatible faces (layout.rs:912-1128, `paste_face_properties` 1164).
- With multiple faces/sectors selected, `Ctrl+C` makes a `GeometryClipboard` (relative offsets, heights and all). Paste shows a wireframe preview following the cursor on the floor plane, `H` flips horizontally, `V` flips vertically, `R` rotates 90 degrees (state toggles, flips mapped on walls/heights/colours), click commits, room grows to fit (layout.rs:1311-1880, viewport_3d.rs:403-424, 1617-1630).
Camera (viewport_3d.rs:6365-6570), two modes toggled by one toolbar button: **Free**: right-drag look, W/A/S/D move, Q/E down/up, Shift = 4x speed, wheel = dolly; there is a macOS stuck-key workaround for Cmd+A (state.rs:462-465, viewport_3d.rs:6415-6455). **Orbit**: right-drag rotates around target, Shift+right-drag pans, wheel zooms (100 to 20000), target follows selection centre. Gamepad sticks also drive the camera. BACKLOG: no minimum camera height.
View toggles (toolbar): Room bounds, Wireframe overlay, Backface mode cycle (both sides / back as wireframe / hidden), and the PS1 effect toggles listed in section 3.

### 2.4 2D grid view (`grid_view.rs`, left panel)
Three projections: T (X-Z top, default), F (X-Y), S (Z-Y) via toolbar letters (state.rs:136, layout.rs:~290-310). Scroll zoom (0.002 to 2.0), right-drag pan. Shows every room: current room bright, others dimmed, hidden rooms skipped; sector tint by contents (full / floor only / ceiling only / walls only); NW-SE or NE-SW split diagonals if `diagonal_matters`; wall edges and diagonal walls; portals (filled quad in top view if horizontal); objects as lettered circles with a facing arrow for spawns; a yellow/red room-centre crosshair handle with index.
Interactions in Select: left-drag empty = rectangle select of sectors (`Selection::Sector`), click sector, click-drag selected sectors to move them within the room grid in X-Z (or in Front/Side view move Y for objects), drag the room centre handle to move the entire room (snaps to sectors), drag objects (snapped to sector grid, 256 in Y views), Delete clears all geometry in selected sectors or deletes objects. Floor / Ceiling / Wall / Object tools place single cells by click (floor at Y 0, ceiling at 3072; "Sector already has a floor"); wall tool uses edge nearest the cursor (`closest_edge_top_view`, grid_view.rs:13) in Top view only. Shift-click toggles multi-select.
2D is a rougher editor than 3D: it lacks height editing, Shift range select and multi-wall paths.

### 2.5 Properties panel (right, `draw_properties` layout.rs:5204-6074)
Content depends on selection:
- Floor/Ceiling face "container" (`draw_horizontal_face_container`, layout.rs:3679-4415): split-direction toggle with diagram (NW-SE / NE-SW); two triangle texture slots with link/unlink; heights link/unlink toggle and +/- 256 buttons for tri 1 / tri 2 (single "Height" when linked); Walkable toggle; UV readout, Reset / Flip H / Flip V / Rotate 90 CW; UV drag fields (offset x/y, scale x/y, angle, with link toggles, double-click to type, pixel-based offsets); per-corner vertex colour with PS1 5-bit RGB sliders, presets and a colour picker (`draw_ps1_color_picker`); Normal three-way Front / Both / Back; Black-transparent toggle; blend mode picker (widgets.rs ~1867); **Extrude** button (floors only: `Sector::extrude_floor(256, wall_tex)` raises floor and builds back-facing perimeter walls, geometry.rs:1986; layout.rs:4364-4410).
- Wall container (`draw_wall_face_container`, 4764-5204): same texture/UV/colour/normal/blend/black controls plus `solid` toggle and **UV projection** toggle (flat texture on slopes).
- Sector: shows floor, ceiling and every wall in that sector. Vertex / Edge: heights per vertex plus colour for the picked vertices ("Vertex 1/2" labels).
- Object: location, component panels (Light: colour RGB sliders 5-bit, intensity, radius as **per-instance overrides**; Collision; "Preview" / "Collision" / "Movement" / "Camera" fields when it is a player start, including a small player-camera preview with wireframe cylinder, layout.rs:6311-6590), Enabled toggle, Delete Object.
- Nothing selected: "Nothing selected".
Multi-selection applies texture, normal mode, black-transparent, vertex colours, UV buttons to all selected faces (commits "Apply texture to all multi-selected faces", "Multi-selection property support"). BACKLOG: slope/height edits do not apply proportionally to multi-selection.

### 2.6 Texture palette and UV editing
`texture_palette.rs`. Two tabs: Source (PNG packs, folder selector, grid of thumbnails with zoom buttons 32/48/64/96) and Paint (user RON textures: New, Edit, delete with confirm, import PNG dialog). Click a thumbnail: sets `selected_texture` and applies to the current selection (`apply_texture_to_selection`, 471). Applies to tri 1 or tri 2 depending on `selected_triangle`. Editing a user texture swaps the right panel for the embedded **texture paint editor** (`src/texture`, 7170 lines; brush/fill/eyedropper/line/rect/ellipse/select tools, indexed palette, undo integrated into the global stack via `UndoEvent::Texture`). The paint editor has a **UV mode** that draws the selected face's UVs as a wireframe over the texture canvas and supports vertex dragging, box select, G / T / R modal grab/scale/rotate, bounding box scale handles, flip/rotate/reset buttons (`texture_editor.rs:15-75, 3900-4100`; `build_uv_overlay_from_selection`, texture_palette.rs:1389; `apply_uv_*` 1511-1900). Keyboard focus is routed by `ActivePanel` (viewport vs texture panel) so WASD does not fly the camera while painting (state.rs:75-80).
UV math: `UV_SCALE = 0.5` is "UV span of one sector" in the UI helpers (geometry.rs:15; layout.rs:4418-4580), but the renderer's default UVs use `32 / tex_width` per sector so UV_SCALE=0.5 is only right for 64 px textures (geometry.rs:2884-2897); an inconsistency to resolve in a port.

### 2.7 Undo / redo
Unified `undo_stack: Vec<UndoEvent>` (cap 100) (state.rs:316, 938-1068). `UndoEvent` is `Level(Level)` (a **full level clone**), `Selection(SelectionSnapshot)`, or `Texture{name,indices,palette}`. `save_undo()` is called before each mutating gesture (at first real movement for drags). Selection changes are separately undoable (dedup vs last snapshot). Redo = Ctrl/Cmd+Shift+Z (`edit.redo`, actions.rs:86; no Ctrl+Y binding; the commit "Ctrl+Z/Ctrl+Y" is superseded). Toolbar has undo/redo buttons. Cost: clone of whole level per edit; fine for small levels, large for Cathedral (493 KB RON). Also `main.rs:405` clones the whole `Level` into `ProjectData` every frame for the Test tab.

### 2.8 Rooms
Rooms panel: list (eye toggles `hidden_rooms`, not saved), trash deletes the room (updates hidden set and selection, `save_undo`), "+ Add Room" creates a 1x1 empty room east of the last one at Y 0 (layout.rs:3108-3135). Room position is edited by dragging the 2D room-centre handle. Rooms stack vertically only by giving `position.y` a value (no direct UI field; BACKLOG: "We need a way to place rooms on top of each other, maybe a side view"; the F and S 2D views exist partly for that). Per-room ambient slider and fog controls in the same panel. `room.duplicate`, `room.add`, `sector.raise_floor` etc. exist as registered actions (actions.rs:117-222) but are not connected to anything; only `file.*`, `edit.undo/redo/copy/paste` are dispatched (layout.rs:877-1054).

### 2.9 Files and play-test loop
Toolbar: New, Open (native file dialog, default `assets/userdata/levels`), Save, Save As, Level Browser (modal with sample + user levels and a **3D orbit preview** of each, `level_browser.rs`), plus WASM Upload/Download/cloud save. Ctrl/Cmd+N/O/S/Shift+S wired (actions.rs:37-72). The editor's own `Play` action is a stub ("Game preview coming soon", main.rs:2539) and the toolbar Play button was removed (commit 8e34f27). The real loop is the separate **Test tab** (`Tool::Test`, app.rs:70): it reads the same in-memory level (cloned each frame), spawns the player at the player-start object, and runs a TR-style cylinder controller (Elden Ring-style third person orbit camera, sprint/jump, free-fly camera option) with debug overlay (main.rs:810-840, game/runtime.rs). Switching to the World tab hot-reloads assets from disk (app.rs:228-245).

### 2.10 Operation table

Complexity is to port into the PSoXide Rust editor, assuming the destination already has a viewport, picking and a UI toolkit. S = under a day of logic, M = a few days, L = a week or more or a design change.

| Operation | How invoked | Code location | Complexity |
|---|---|---|---|
| Data model (Level/Room/Sector/Face structs, serde, load/save) | n/a | world/geometry.rs, world/level.rs | S (as data) / M (mapping to PS1 fixed-point and a TR-style collision model) |
| Floor/ceiling rect placement (grows room grid) | Tool 2/4 + left-drag; Shift = height | viewport_3d.rs:700-800, 2494-2620 | M |
| Single sector placement in 2D | Floor/Ceiling tool click in 2D | grid_view.rs:1257-1405 | S |
| Wall placement with gap detection (slanted aware) | Tool 3, R/F keys, drag | geometry.rs:1630-1960; viewport_3d.rs:801-1040, 2625-2780 | L (the gap logic took many commits and reverts) |
| Diagonal walls | Tool 3 + R to NW-SE/NE-SW, 3D only | geometry.rs:3235-3355; viewport_3d.rs:939-1040, 2423-2540 | M |
| Vertex / edge / face height drag, 256 snap, cross-room linked vertices | Shift+drag in Select | viewport_3d.rs:1048-1470, 2195-2260 | M |
| Per-corner slope | Same as above (no dedicated slope tool) | HorizontalFace.heights | S |
| Floor/ceiling split direction, per-triangle texture/heights | Properties panel toggles | geometry.rs:1059-1250; layout.rs:3679-4020 | M |
| Face X/Z relocation (drag) | Drag selected face (no Shift) | viewport_3d.rs:6768-7030, 2155-2193 | M |
| Extrude floor | Properties "Extrude" button (status bar claims `[E]`; no key handler exists) | geometry.rs:1986; layout.rs:4364 | S |
| Delete faces/sectors/objects | Del/Backspace in 3D, 2D | viewport_3d.rs:513-600, 5654 | S (but wall index renumbering is a hazard) |
| Hover picking vertex>edge>face with depth, objects | Mouse move (Select/Object tool) | viewport_3d.rs:7028-7520 | M (L if the viewport has a different projection model) |
| Multi-select: Shift range, Ctrl toggle, box, Ctrl+A, wall-path BFS | Mouse + modifiers | viewport_3d.rs:109-250, 1048-2010, 7512-7650 | M |
| Selection kinds + undo of selection | n/a | state.rs:154-380 | S |
| Unified undo/redo (level snapshot) | Ctrl+Z, Ctrl+Shift+Z | state.rs:938-1068 | S (L if replaced with command-based undo) |
| Texture apply (palette click) and auto-scroll to face texture | Click thumbnail / select face | texture_palette.rs:471-590; state.rs:1262 | S |
| Face property copy/paste | Ctrl+C / Ctrl+V | layout.rs:912-1310 | S |
| Geometry copy/paste with H/V/R transform and ghost preview | Ctrl+C multi, then H/V/R, click | layout.rs:1311-1880; viewport_3d.rs:403-424 | M |
| UV fields: offset/scale/angle, flip, rotate, reset | Properties panel | layout.rs:4418-4760 | S/M |
| UV editor over texture canvas (drag, G/T/R, handles) | Texture panel UV mode | texture/texture_editor.rs, texture_palette.rs:1389-1900 | L (depends on the paint editor) |
| UV projection toggle for sloped walls | Wall properties | geometry.rs:3120-3170; layout.rs:4975 | S |
| Vertex colour per corner (PS1 5-bit picker, presets) | Properties panel | layout.rs:3553-3640; ui/widgets (`draw_ps1_color_picker`) | M (colour picker is a custom widget) |
| Blend mode / normal mode (double-sided) / black-transparent | Properties | layout.rs:3439-3552 | S |
| Object placement (sector snap, unique player start) | Tool 5 click | viewport_3d.rs:657-700, 1030-1120; grid_view.rs:1446-1505 | S |
| Object drag XZ / Y | Select or Object tool | viewport_3d.rs:1226-1370 | S/M |
| Light component overrides (color/intensity/radius) | Properties for object | layout.rs ~5700-5900; geometry.rs:2235-2290 | S |
| Rooms add / delete / hide | Rooms panel | layout.rs:3014-3140 | S |
| Move room / move sectors in 2D | 2D drag | grid_view.rs:973-1100 | M |
| Auto-portals (vertical + horizontal, trapezoid) | automatic after edits | geometry.rs:3657-3990 | M (only worth it if the runtime culls by portal) |
| Room ambient + fog | Rooms panel | layout.rs:3014-3398 | S |
| Skybox editor | left panel | layout.rs:1880-2720; geometry.rs:321-1020 | L if ported at all (not PS1 hardware-natural; skip) |
| 3D camera free/orbit, centre on selection | RMB, WASD, `.` | viewport_3d.rs:6365-6570; state.rs:1160-1250 | S/M |
| 2D views T/F/S, pan/zoom | left panel | grid_view.rs | M |
| PS1 look toggles in the viewport | toolbar | layout.rs:752-855 (see sec. 3) | depends on renderer |
| Level browser with 3D preview | Toolbar | editor/level_browser.rs | M |
| Level save/load (RON + Brotli, validation) | Ctrl+S/O | world/level.rs | S |
| Play-test | Test tab | game/runtime.rs, game/collision.rs, main.rs:810 | PSoXide already has Play |

Keybinding quick reference: `1..5` tools; `R` rotate wall dir (wall tool) / rotate paste clipboard; `F` wall gap high/low; `L` link/unlink coincident vertices; `H`/`V` flip paste preview; `Esc` clear selection and paste; `Ctrl/Cmd+A` select all faces in room; `.` centre camera; `Del`/`Backspace` delete; `Ctrl/Cmd+C/V` copy/paste; `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` undo/redo; `Ctrl/Cmd+N/O/S`, `+Shift` save-as; free cam `WASD QE`, Shift fast; mouse: LMB select/place/drag, Shift at press = Y drag/range select, Ctrl toggle, RMB look/orbit, Shift+RMB pan (orbit), wheel dolly/zoom.

Snapping: sectors 1024 in XZ, heights 256 clicks (hard-coded `CLICK_HEIGHT`; the `state.grid_size` field exists but only affects the drawn ground grid). There is no free-form vertex XZ move at all (TRLE constraint "Y-only vertex dragging", commit "Enforce Y-only vertex dragging in 3D viewport (TRLE constraint)").

Gizmos: none for geometry in the world editor (transform gizmos exist only in the modeler, commit 2025-12-28). Objects are shown as small coloured shapes/wireframes (octahedron for lights, cylinders/boxes/capsules/spheres for collision, viewport_3d.rs:4088-4270, 5979-6220).

---

## 3. Rendering and preview

- The 3D viewport is the game's own **CPU software rasterizer** (`src/rasterizer`, 6331 lines, ported from tipsy, `rasterizer/mod.rs:1-12`): Vec3/Mat4, perspective + ortho, near-plane clipping, affine or perspective-correct texturing, Z-buffer or painter's algorithm, flat/Gouraud shading, 4x4 Bayer dithering, RGB555 framebuffer and textures with PS1 semi-transparency bit (0x8000) and 0x0000 colour key, six PS1 blend modes (Average, Add, Subtract, AddQuarter, plus Opaque, Erase), fixed-point 16.16 projection ("PS1 jitter"), per-room linear depth-cue fog with a cull distance, optional 320x240 low-res mode (`WIDTH`/`HEIGHT` constants) vs hi-res, 4:3 vs stretch. `RasterSettings` (rasterizer/types.rs:1391-1446) defaults: affine on, fixed-point on, RGB555 on, dithering on, Gouraud, backface cull with wireframe-on-backface (an editor feature).
- Editor overlay is drawn **directly into the framebuffer** with depth-tested line routines (`draw_line_3d_overlay`, render.rs:764) after `scene::render_scene`: hover/selection wireframes, vertex dots, hover grids, wall previews, portals, object gizmos, box select. Framebuffer is uploaded as a macroquad texture.
- Toolbar toggles for the PS1 look: Affine, Fixed-point, Gouraud, Low resolution, Dithering, Aspect ratio, Z-buffer vs painter, RGB555 (layout.rs:752-855). So "WYSIWYG" with the game is exact because both use `scene::render_scene` (scene.rs:138-200, the shared path, commit "Unify scene rendering").
- Lighting preview: per-vertex Gouraud from point lights (placed Light objects, using their per-instance overrides) plus per-room ambient and one default directional light (rasterizer/render.rs:1004-1075). Lights are only collected when shading is on (viewport_3d.rs:3447-3451).
- Mesh generation: `Room::to_render_data_with_textures` (geometry.rs:2839-3355) rebuilds the whole room's vertex/face arrays **every frame** (the `vp_meshgen_ms` timing is hard-wired to 0; there is no cache). Floors/ceilings emit 2 triangles (4 when Both-sided), walls 2 triangles per quad (4 when both). Winding/normal modes per face, default UVs are world-aligned (grid index x `32/tex_width`) so adjacent sectors tile; walls use Projected V = -worldY/1024 x uv_scale.
- Skybox is rendered into the framebuffer before geometry (viewport_3d.rs:3040-3050).
- Runtime (Test tab / `game/`): rooms rendered by the same `render_scene`, no portal culling, fog distance culling, blob shadows (`game/shadows.rs`), particles. Collision is **floor/ceiling-height only**: `Level::get_floor_info` (geometry.rs:3597) finds the room by AABB then samples `interpolate_height` per sector; `collide_cylinder` (game/collision.rs:41-170) checks the cylinder's four corners, step height (default 384), pushes back when the floor is higher than `step_height`, treats outside-all-rooms as wall. Wall faces and the `solid` / `walkable` flags are **never read** by the runtime (grep over `src` finds them only in the editor, geometry and the clipboard). So wall faces are purely visual and collision is the OpenLara/TR heightfield approach, which is the closest thing bonnie-32 has to a TR floordata model. Fog start/falloff is not cut off by portals.

---

## 4. Known pain points, TODOs, unfinished

From `BACKLOG.md` (whole file is 81 lines; only world editor items):
- Stack rooms vertically: "maybe the top view can be toggled with a side view?" (it already has F/S views but placement is not by UI).
- Batch slope/height editing: slope/height edits should apply proportionally to a multi-selection.
- Better floor placement highlight.
- Context-sensitive bottom bar (left/right click actions, WASD/QE when right-clicking).
- Camera floor limit (cannot go below floor).
- Skybox UI polish; gradient fills (linear/spherical) of vertex colours across selected cells; auto-select room vs manual room lock; colour slider lock toggle; convert texture packs to `.ron` textures; Environment/Objects switch.
- Architecture: "unified render pipeline" (partly done by `scene.rs`; editor overlays still bespoke).
From `notes` (personal scratch, 5 KB): "when having a multiple selection, clicking on a cell should deselect everything and select that cell", "remove the colour not working on vertex colours", textures are unloaded after a while (cache lifetime), camera minimum height. The rest of `notes` is Reddit post drafts stating the design pillars (see 5).
From the code:
- `viewport_3d.rs::draw_viewport_3d` is one function of about 5360 lines with ~100 transient drag fields on `EditorState` (state.rs:384-720); a refactor was started 2025-12-14 ("split draw_viewport_3d into smaller functions") and clearly did not stick.
- Status bar advertises `[E] Extrude` (layout.rs:6270, 6280) but there is no `E` handler (E is camera up in Free mode); extrusion is button only (layout.rs:4392).
- `Ctrl+A` omits diagonal walls (viewport_3d.rs:441-505).
- Wall faces are addressed by Vec index in `SectorFace`, so deleting one invalidates other selections.
- No portal use at runtime; portals recomputed after every geometry edit with an O(rooms^2 x cells) scan; also `portals_dirty` recalculation can race hidden rooms (inferred).
- `solid`, `walkable` are authored and persisted but unused by the runtime.
- `UV_SCALE` (0.5) vs per-texture `32/tex_width` mismatch (see 2.6). Texture sizes assumed 64 in the editor's texture map (viewport_3d.rs:3426-3432), user textures use their real width.
- Level format has no version/migration; legacy `object_type` objects silently dropped (samples Cave/West lose lights and spawn).
- Every edit clones the full level for undo and `main.rs:405` clones it every frame for the Test tab.
- Four copies of the load pipeline in `world/level.rs`.
- macOS key-up bug workaround machinery for WASD after Cmd+key (viewport_3d.rs:6415-6455, state.rs:462-465).
- `world/mod.rs:8` and `editor/mod.rs:9` carry `#![allow(dead_code)]` with "not yet used" notes; action registry contains unwired actions (actions.rs).
- `static mut FRAME_COUNTER` unsafe use in `draw_editor` (layout.rs:~492).
- Rooms added by Add Room get no UI to set Y; `Room.id` is not updated after a room is deleted (inferred from layout.rs:3066-3090, not run).
- `trim_empty_edges` keeps objects only if inside the kept grid (drops outside, geometry.rs:2755-2772).
Bug-fix churn visible in history: wall gap detection (commits "Fix wall gap selection..." with an immediate revert 2026-01-06, "Fix triangular wall gap detection", "Improve wall stacking for slanted walls"), cross-room vertex linking (3 commits 2025-12-31), X/Z drag with grid trimming (about 8 commits 2026-01-09), macOS stuck keys (3), panel focus for WASD (2).

---

## 5. What Manny evidently valued

Evidence is the notes, README pillars, and where commits concentrated (counts of commit subjects touching `src/editor src/world`: selection 46, texture 41, wall 27, drag 26, room 25, vertex 20, UV 18, objects/assets 29, light 14, diagonal/split 11; files by commit count: viewport_3d.rs 152, layout.rs 110, state.rs 97, geometry.rs 61, grid_view.rs 52, texture_palette.rs 43).
- "Powerful world editor modeled after the Tomb Raider grid system" (`notes` Reddit draft) and the explicit TRLE constraints: 1024 sectors, 256-unit clicks, Y-only vertex dragging, sector occupation validation, "TRLE sector grid constraints" commits on 2025-12-05.
- Direct manipulation in the 3D view: pick vertex/edge/face, Shift-drag heights, drag rectangles of floors, drag runs of walls, drag faces across the grid, box select. Multi-select in every form got the most iteration.
- Fast texture loop: click face, palette scrolls to its texture, click a thumbnail to apply, paint your own texture in the same panel, UV editing in the same panel ("Unify paint and UV editor with direct vertex dragging", "Integrate texture editor into World Editor", "Add auto-scroll texture palette on face selection").
- PS1 fidelity in the editor itself (dithering, RGB555, affine, fixed-point jitter, black = transparent, per-vertex colour, blend modes, depth-cue fog per room, point-light colours quantised to 5 bit sliders) so the editor view equals the game view.
- Slanted floors/ceilings and walls that follow slants, diagonal walls, per-triangle split, and extrusion, i.e. geometry richer than classic TRLE.
- Integrated asset placement (the level places assets built in the Modeler tab, lights as assets with per-instance overrides) and one-click play-test in a neighbouring tab with live edits.
- Browser (WASM) parity and cloud save, shareable levels, level browser with 3D preview; "pico8-style flexibility".
- Design pillar quote (`notes`): "Designed around PS1 hardware quirks and limitations, encourage user to stick to them but allow crossing them".

---

## 6. Coupling and portability

Stack: macroquad 0.4 (window, input, immediate-mode drawing, texture upload, `get_time`), serde+ron+brotli, image, a **custom immediate-mode UI toolkit** in `src/ui` (`UiContext`, `Rect`, `SplitPanel`, `Toolbar`, `ActionRegistry`/`Shortcut`, `drag_tracker`, `tool_box`, `tabbar`, `text_input`, ~6400 lines; widgets.rs 2282). Not egui. The editor files `layout.rs`, `viewport_3d.rs`, `grid_view.rs`, `texture_palette.rs` call macroquad draw functions directly (draw_text/draw_rectangle) and have layout hard-coded in pixels; they would be rewritten, not adapted, in egui/PSoXide's UI.
Mostly portable as pure logic (little or no macroquad):
- `world/geometry.rs` data model and methods: `HorizontalFace::interpolate_height`, `edge_heights`, `SplitDirection` triangle tables, `Sector::next_wall_position`, `extrude_floor`, `Room::{ensure_sector, set_*, recalculate_bounds, trim_empty_edges, compact, world_to_grid}`, `Level::{find_room_at, get_floor_info, recalculate_portals, detect_*}`. Dependencies are rasterizer `Vec3/Vec2/Color/BlendMode/Vertex/Face` (swap to the PSoXide types) and serde. The mesh generation functions (`to_render_data_with_textures`, geometry.rs:2839-3355) emit bonnie vertices and need rewriting for the PSoXide/GTE packet path, but the corner/winding/UV/normal-mode logic is reusable as a spec.
- `world/level.rs` validation and loading (replace RON/brotli with PSoXide's `project.ron`/versioned format).
- Selection model enums and clipboard structs (state.rs:154-380), undo event design (snapshot-based; trivially portable, but reconsider).
- Picking maths: `find_hovered_elements`, `check_quad_hit_with_depth`, `find_selections_in_rect`, `find_wall_path`, `relocate_faces`, `paste_geometry_at_impl` operate on `EditorState` and the rasterizer `Camera`/`world_to_screen_with_depth`, so they need extraction from `EditorState`, but are algorithmic.
Needs rewriting:
- All panel UI code (layout.rs about 6500 lines is mostly pixel-laid immediate-mode widgets) and the PS1 colour picker/three-way toggle widgets.
- The monolithic input/drag state machine in `draw_viewport_3d` (re-express as per-tool state structs).
- The software-rasterizer overlay drawing helpers, if PSoXide's editor uses a GPU viewport.
- `texture_editor.rs` paint/UV editor (5120 lines) if PSoXide already has its own texture tooling; the UV overlay logic is separable.
- Asset library coupling (`AssetLibrary`, Modeler `AssetBrowser`): objects depend on assets with components; PSoXide has its own object/prefab model.
Other coupling facts: `EditorState` also holds texture library, asset library, UI scroll offsets, slider flags, macOS key workarounds, skybox panel UI state (state.rs:384-720); the editor is not separable from `crate::texture`, `crate::asset`, `crate::modeler::AssetBrowser`, `crate::storage`.

Mapping notes for a TR-style PS1 grid world (observations, not a design): bonnie-32's sector (1024 x 1024 with 256 click heights, 4-corner floor/ceiling, split direction, stacked edge walls, optional diagonal walls) already matches TR's room/sector vocabulary visually; but it carries no TR floordata concepts (no per-sector flags, no collision-only walls, no triggers/floordata commands, no room-to-room links stored on sectors; portals are derived geometry), and its heights are `f32` (PSoXide guest math must be integer, per the project rules). Runtime wall collision does not exist; if Cortex wants TR wall blocking from a sector grid, that has to come from floor/ceiling height differences exactly as bonnie-32's collider does, or from new data.

---

## 7. Screenshots in the repo

- `/Users/ebonura/Desktop/repos/bonnie-32/docs/screenshot-world-editor.png` (3024x1824, the only world editor shot: full layout, brick/checker dungeon room with red point light, 2D grid panel, Rooms panel, Textures Paint tab; level_006.ron).
- `/Users/ebonura/Desktop/repos/bonnie-32/docs/screenshot-asset-editor.png` (modeler, relevant only for asset/object authoring)
- `/Users/ebonura/Desktop/repos/bonnie-32/docs/screenshot-music-tracker.png` (not relevant)
- Web build copy of the editor UI: none (the `dist/web` build has assets only). No GIF or video in the repo. Live demo was hosted from `docs/index.html` + `docs/bonnie-32.wasm`, `docs/bonnie-engine.wasm`.

Sample levels (compressed in repo): `/Users/ebonura/Desktop/repos/bonnie-32/assets/samples/levels/{Cathedral,Cave,Dungeon,Sewers,West,level_008}.ron`; decompressed copies: `/Users/ebonura/Desktop/repos/PSoXide-editor-wt/open-hall/build/oh-runs/scratch/levels/` (identical sizes to a fresh `brotli -d` of the repo files). Rooms: Cathedral 1, Cave 2, Dungeon 2 (90 portals, has a skybox), Sewers 1, West 1, level_008 1; the screenshot shows a file named level_006.ron (not among the samples; its Rooms panel reads 33x26, 720 sectors). Texture packs referenced: `dark-military-fantasy`, `dark-fantasy-townhouse` etc. under `/Users/ebonura/Desktop/repos/bonnie-32/assets/samples/texture-packs/`.
