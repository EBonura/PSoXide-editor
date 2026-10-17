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
* File > New Grid Project... asks for a name and makes `editor/projects/<name>/`: a 6 by 6 sector room with a floor, a ceiling, walls on the whole outline, one lamp and the Graybox Reach player in the middle. It borrows the player, camera, sky and UI from `editor/projects/graybox-reach` (the project's `assets` folder links to it), so that project has to exist. Press Play and you are walking in it.
* The six reference levels (`grid-cathedral`, `grid-level008`, `grid-dungeon`, `grid-cave`, `grid-sewers`, `grid-west`) are tracked projects. They came from bonnie-32 maps; the importer is gone, so the projects are now the source of truth.

## The screen

* Centre: the 3D view (the emulator's software GPU, so it is the PS1 picture) or the Top view (`2D` tab, then Top). Switch with the tabs.
* Left dock: Rooms (below), then the scene graph and the file tree.
* Right dock: Materials palette, then the Inspector for the selected faces or node.
* Bottom bar: last message, the keys of the active tool, the room's sector count and the world's face budget (`faces / 16383`, orange from 85%).
* `?` opens a page with every key and gesture.

### Rooms panel

One row per room: the eye shows or hides it (hidden rooms leave the 3D preview, the Top view and picking), click a name to make it the room new geometry goes into, double-click to rename, the bin deletes it (the last room stays), `+` adds an empty room one storey above the highest. Under the list: Raise (the whole room's height offset in clicks) and Ambient (RGB, 128 shows textures unmodified). Rooms may overlap in plan; that is how floors stack.

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

Also: Ctrl/Cmd+C copies the selected faces, Ctrl/Cmd+V shows a ghost (click stamps, Shift keeps stamping, H and V flip, R turns), Esc cancels a drag, a ghost or the selection, Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z undo and redo, `.` frames the selection, Ctrl/Cmd+S saves.

3D camera: middle or right drag orbits, with Shift it pans, scroll zooms, and with the free camera W A S D fly while the right button is held.

Units: a sector is 1024 grid units, a click 256, the fine step 64. A project's scale maps grid units to editor units (File > New Grid Project uses 5/4, so a sector is 1280 editor units; the imported levels keep the scale their import chose. `GridScale::DEFAULT`, still 21/16, moves to 5/4 with the next phase). Walls belong to the sector edge they are on, with a front side (the interior) and an optional back or both.

## Free planes

A free plane is a flat textured quad that is not tied to the sector grid: a bridge deck, a ramp, a slanted wall, a banner, a grate. Each room owns a list of them (at most 64). Press N (or pick Plane in the strip).

* **Place**: click a floor, a wall or empty ground. The plane lies on that surface facing out of it, lifted a hair so it never fights the surface, snapped to the 256 lattice, one sector square. Drag instead of clicking to size it: the press point is one corner, the size snaps to the lattice. The strip's "New plane" menu sets the collision class and sidedness a new plane starts with.
* **Select**: click a plane with the Plane or the Select tool (Shift extends, Ctrl/Cmd toggles). In the Top view a plane picks by its outline or inside it; hold Alt to click through to the sector under it. The Inspector then shows one context, the plane.
* **Handles** of the selected plane: the four corners and four edges stretch it (the opposite edges stay put); the green ring knob turns it about the vertical axis in 15 degree steps; the blue arrow tip tilts the front up and down; dragging the body slides it in its own plane, Shift moves it up and down. Ctrl/Cmd snaps finer (64 units, 5 degrees), Alt is free. Moves snap the plane's edge or centre onto the lattice, whichever is nearest, so edges land on grid lines. Roll and exact numbers are in the Inspector.
* **Keys**: R quarter turn, PgUp/PgDn or ]/[ nudge up and down a click (Shift: 64), Delete removes, Ctrl/Cmd+C and V copy and paste with a ghost (R turns, H flips, click stamps), Esc during a drag puts the plane back, Ctrl/Cmd+Z undoes.
* **Material**: the palette applies to the selected planes, the Paint tool paints a plane under the pointer (Ctrl/Cmd+click samples its material). Blend mode and alpha cutout (palette index 0 transparent) come from the material, so a banner or grate is a plane with the right material and "Both sides" ticked.
* **Inspector**: position (grid units), size, yaw/pitch/roll, Wall/Floor/Ceiling orientation presets, mapping (Tile repeats every sector from the top-left corner, Stretch puts one whole texture across), texture offset, scale, angle and flips, both sides, fullbright, enabled, and the collision class.

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
* Free planes: the camera does not pass through them (a solid plane blocks the camera arm like a wall), they are not yet in the psxed-mcp tools, and translucent planes follow the engine's ordering of blended packets.
* Brush and BSP authoring still exist for brush projects and are hidden for grid ones; the PXBSP cook is only a transitional bridge for collision.
