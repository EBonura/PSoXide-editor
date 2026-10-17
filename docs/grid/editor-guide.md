# Grid world editor guide

How to run the grid editor from this worktree, open a grid project, use every tool, press Play and bring in a level. Plan and decisions are in `plan.md`.

## Run it

From the repository root:

```bash
cargo run --release -p frontend --features editor -- --editor --windowed --editor-project editor/projects/grid-cathedral
```

Swap the last path for any grid project: `grid-cathedral`, `grid-level008`, `grid-cave`, `grid-dungeon`, `grid-sewers`, `grid-west`, or a project you made with File > New Grid Project. Leave `--editor-project` off to open the default project and switch with File > Project. The first build takes a few minutes; `make run-release` does the same through the Makefile.

Play and Build shell out to `make build-editor-playtest`, which runs `make bootstrap` first. Bootstrap refuses to continue when a hydrated emulator file has been edited ("Imported file changed"), so keep `emu/crates/frontend/src/cli.rs` and `gfx.rs` as the pinned emulator revision ships them. `.tools/sdk-*/bin/psoxide-components --root . --check` says whether they match.

## Open or create a project

* File > Project lists every project under `editor/projects`. A grid project opens in the 3D view with the grid tool strip across the top.
* File > New Grid Project... asks for a name and makes `editor/projects/<name>/`: a 6 by 6 sector room with a floor, a ceiling, one lamp and the Graybox Reach player in the middle. Its outline walls are not stored: the room derives them (see Derived walls below). It borrows the player, camera, sky and UI from `editor/projects/graybox-reach` (the project's `assets` folder links to it), so that project has to exist. Press Play and you are walking in it.
* The six reference levels (`grid-cathedral`, `grid-level008`, `grid-dungeon`, `grid-cave`, `grid-sewers`, `grid-west`) are tracked projects. They came from bonnie-32 maps; the importer is gone, so the projects are now the source of truth.

## The screen

* Centre: the 3D view (the emulator's software GPU, so it is the PS1 picture) or the Top view (`2D` tab, then Top). Switch with the tabs.
* Left dock: Rooms (below), then the scene graph and the file tree.
* Right dock: Materials palette, then the Inspector for the selected faces or node.
* Bottom bar: last message, the keys of the active tool, the room's sector count and the world's face budget (`faces / 16383`, orange from 85%).
* `?` opens a page with every key and gesture.

### Rooms panel

One row per room: the eye shows or hides it (hidden rooms leave the 3D preview, the Top view and picking), click a name to make it the room new geometry goes into, double-click to rename, the bin deletes it (the last room stays), `+` adds an empty room one storey above the highest. Under the list: Raise (the whole room's height offset in clicks) and Ambient (RGB, 128 shows textures unmodified), then the room's walls: whether they are *derived from heights* or *authored (legacy)*. A legacy room offers **Convert to derived** (the look and the play stay exactly as they were) and **Convert and fill** (also shows the walls the room never had). A derived room offers its wall material (Use palette / Clear), the Open wall height for a floor with no ceiling, and Clean up for overrides whose wall no longer exists. Rooms may overlap in plan; that is how floors stack.

### Materials palette

Lists the materials the world already uses, the images imported with a level (`level_textures/`) and the Graybox set. HUD, menu, button prompt, kit, sky and character images are left out; tick "Show all project materials" to see them. Click a swatch to make it the current material (the Paint tool and new surfaces use it) and to apply it to the selected faces.

## Tools

The strip has nine tools. Wrapped to a second line when the window is narrow. Keys work in the 3D and Top views.

| Tool | Key | Gesture |
| --- | --- | --- |
| Select | V | Click picks the vertex, edge or face under the pointer. Shift+click extends a range, Ctrl/Cmd+click toggles, drag draws a box. Shift+drag raises or lowers the selected corners. Delete removes the selected faces. Ctrl/Cmd+A selects the room. |
| Floor | F | Drag a rectangle; the room grows to fit. Shift while dragging sets the height. Ctrl/Cmd+drag deletes floor. |
| Ceiling | C | Same as Floor, for ceilings. |
| Wall | W | Drag along an edge to place a run of walls; they fill the gap up to the ceiling or the next wall. R turns the direction (auto, N, E, S, W, two diagonals). Ctrl/Cmd+drag removes. |
| Plane | N | Click a surface to put a free plane on it, drag to size it. Click a plane to select it, then use its handles (see Free planes below). |
| Height | H | Drag a vertex, edge or face up and down in 256 unit clicks, Alt for 64. L unlinks corners that share a position. PgUp/PgDn or ]/[ nudge the selection. |
| Paint | P | Click or drag faces to apply the palette material. Ctrl/Cmd+click samples a face's material. |
| Split | D | Click a floor or ceiling to flip its diagonal. |
| Place | O | Click to put the chosen node (light, spawn, actor...) at a sector centre; Alt places freely. |

Letters kept free for the level verbs that come next: U Raise, S Sculpt, A Ramp, T Stairs, B Room, K Corridor, I Opening, E Bands, Y Stamps (N is the Plane tool), and Alt+W and Alt+P for the walkability and performance overlays.

Also: Ctrl/Cmd+C copies the selected faces, Ctrl/Cmd+V shows a ghost (click stamps, Shift keeps stamping, H and V flip, R turns), Esc cancels a drag, a ghost or the selection, Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z undo and redo, `.` frames the selection, Ctrl/Cmd+S saves.

3D camera: middle or right drag orbits, with Shift it pans, scroll zooms, and with the free camera W A S D fly while the right button is held.

Snapping is one rule in every tool: a click (256) by default, **Alt** for the fine step (64), **Shift+Alt** for free movement where a tool can leave the lattice (planes, Place; heights always stay on the 64 lattice).

Units: a sector is 1024 grid units, a click 256, the fine step 64. A project's scale maps grid units to editor units (File > New Grid Project uses 5/4, so a sector is 1280 editor units; the imported levels keep the scale their import chose. `GridScale::DEFAULT`, still 21/16, moves to 5/4 with the next phase). Walls belong to the sector edge they are on, with a front side (the interior) and an optional back or both.

## Tiling

Every face (floor, ceiling, authored wall, derived wall, plane) is textured by one model, edited in the Tiling section of the Inspector:

* **World**: the texture follows the world, so neighbouring faces line up and a bigger face tiles more. The number is texels per sector along each axis; 32 (half a 64 texel tile per sector) is how every face made before this behaves, bit for bit.
* **Fit**: a fixed number of tiles across the whole face, whatever its size. 2 x 2 is "2x2 on this face"; the one-click presets are 1x1, 2x2 and 4x4, the fields take any value in 1/16 tile steps up to 16 per axis. The frame a fit is measured against is the logical face: the sector square of a floor or ceiling, the wall record of an authored wall, the whole run of a derived wall (so 8x1 repeats eight times along the whole run, not once per sector) and the plane rectangle.
* **Default**: the material's own tiling (Material tab, Grid tiling: Legacy, World or Fit; a swatch in the palette carries a badge when it has one), else the legacy world mapping. A face that names its own mode ignores the material.
* **Turn** (quarter turns, clockwise) and **Flip** (H, V) act on the face as you see it, and **Offset**, **Angle** and **Scale** stay as fine adjustments after the mode.

The Inspector always prints what the setting comes to ("effective: 2 x 2 on this face", "0.5 x 0.5 tiles on this face") and warns when the cook has to cut the face: a primitive holds one tile per axis for now, so a 4x4 face becomes 16 primitives (+15). The cook counts the extra primitives in its report. Faces on the plain legacy mapping are left whole exactly as before (see `uv_split::SPLIT_LEGACY`), so levels made before the model cook the same.

## Derived walls

A new room's walls are not drawn by hand. Raise a platform, lower a pit or leave a room's edge and the editor makes the wall for you, from the height difference between the two sectors on either side of the edge:

* a step between two floors gets a **riser**, seen from the lower side;
* a step between two ceilings gets a **drop**, seen from the higher side;
* the edge of the room (the neighbour holds no floor or ceiling) gets a **closure** wall from floor to ceiling, which also blocks movement, so a derived room cannot be walked out of. A floor with no ceiling gets one only when the room's Open wall height is above 0.

Where two floors cross inside one edge the wall is cut at the crossing into two triangles. Equal heights make nothing, so a continuous floor never grows a wall. The Top view draws derived walls in light blue (a ghost closure in yellow, a hidden face dimmed).

A derived face takes its material from, in order: the face itself, the sector the step belongs to (its wall material), the sector looking at it, the room's wall material, then the owner's own floor or ceiling material. Click a derived wall with Select (or hover it with Paint) like any face: Paint writes the face's override, the Inspector edits its material, texture placement, Solid and Hidden flags, and Back to automatic forgets all of them. Delete hides a derived face. Overrides are stored with the room by wall segment, so they survive edits to the neighbours. The Wall tool still places authored walls for what heights cannot say (a freestanding wall, a parapet); it treats derived faces as walls already standing, so an edge a derived wall closes answers "covered". A warning is shown for an authored wall that lies on top of a derived face.

Rooms made before this keep their wall records and their look until you press **Convert to derived** in the Rooms panel. Convert removes an authored wall only when it is exactly a derived face, moving its material, texture and solidity into the override; every other wall stays, and the derived faces the room never had are hidden so nothing new appears. One undo step reverts it.

## Free planes

A free plane is a flat textured quad that is not tied to the sector grid: a bridge deck, a ramp, a slanted wall, a banner, a grate. Each room owns a list of them (at most 64). Press N (or pick Plane in the strip).

* **Place**: click a floor, a wall or empty ground. The plane lies on that surface facing out of it, lifted a hair so it never fights the surface, snapped to the 256 lattice, one sector square. Drag instead of clicking to size it: the press point is one corner, the size snaps to the lattice. The strip's "New plane" menu sets the collision class and sidedness a new plane starts with.
* **Select**: click a plane with the Plane or the Select tool (Shift extends, Ctrl/Cmd toggles). In the Top view a plane picks by its outline or inside it; hold Alt to click through to the sector under it. The Inspector then shows one context, the plane.
* **Handles** of the selected plane: the four corners and four edges stretch it (the opposite edges stay put); the green ring knob turns it about the vertical axis in 15 degree steps; the blue arrow tip tilts the front up and down; dragging the body slides it in its own plane, Shift moves it up and down. Alt snaps finer (64 units, 5 degrees), Shift+Alt is free. Moves snap the plane's edge or centre onto the lattice, whichever is nearest, so edges land on grid lines. Roll and exact numbers are in the Inspector.
* **Keys**: R quarter turn, PgUp/PgDn or ]/[ nudge up and down a click (Shift: 64), Delete removes, Ctrl/Cmd+C and V copy and paste with a ghost (R turns, H flips, click stamps), Esc during a drag puts the plane back, Ctrl/Cmd+Z undoes.
* **Material**: the palette applies to the selected planes, the Paint tool paints a plane under the pointer (Ctrl/Cmd+click samples its material). Blend mode and alpha cutout (palette index 0 transparent) come from the material, so a banner or grate is a plane with the right material and "Both sides" ticked.
* **Inspector**: position (grid units), size, yaw/pitch/roll, Wall/Floor/Ceiling orientation presets, the Tiling section (below; Tile repeats every sector from the top-left corner, Stretch puts one whole texture across), both sides, fullbright, enabled, the collision class and the camera setting.
* **Camera**: the third-person camera passes through a plane that is translucent or alpha-cut (a banner, a grate, foliage) and stops at an opaque one. The Camera row of the Inspector overrides it per plane (passes through / blocks). The character itself still collides with the plane per its collision class either way.

Collision class decides what the cook writes, from the plane's normal: None (decor) is drawn only. Walkable and Solid both collide, and the shape follows the lean: at least 0.7 up or down (flatter than 45 degrees) is a platform the character stands on, at most 0.35 is a vertical wall segment that blocks from both sides, anything between is a box around the plane (it over-blocks and the cook warns). A Walkable plane too steep to stand on reports a warning and blocks instead. A plane has to lie inside its room's sector rectangle; the editor grows the room to hold it, and the cook refuses a plane outside it.

The 3D preview and Play both draw planes with the light baked per corner like any face (a both-sided plane is lit from either side, fullbright skips the bake). Tiled planes are cut at every sector so each piece stays inside one texture repeat.

## Play

Press Play (or Build to export a disc). The editor cooks the grid document, builds the guest and starts it in the Play viewport. Nothing else is needed: there is no environment variable and no feature flag. What the cook does:

1. Validates the world against the shared caps (room size, wall stack, face budget, coordinate range). A problem stops the cook with a message such as `grid world: room 0 sector 3,4: ...` in the status line and the build log.
2. Derives thin collision slabs from the document (solid walls and walkable surfaces) plus a sky lid, and cooks them through the existing PXBSP path for collision, activation and the camera. Turn `solid` off on a wall or `walkable` off on a surface (Inspector) to walk through it.
3. Writes the render pack from the same faces the editor draws, with the light baked per corner from the Point Light nodes, the room ambient and the slabs as shadow casters.
4. Marks the manifest as a grid world; the guest build picks the grid renderer from that.

The cook warns when a texture is not 64 by 64, when faces have no material the cook can bind and when collision slabs were refused.

## Known gaps

* Camera and character: the starter and imported levels use the Graybox Reach character, which costs most of the frame.
* Free planes: they are not yet in the psxed-mcp tools, and translucent planes follow the engine's ordering of blended packets.
* Brush and BSP authoring still exist for brush projects and are hidden for grid ones; the PXBSP cook is only a transitional bridge for collision.
