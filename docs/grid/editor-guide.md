# Grid world editor guide

How to run the grid editor from this worktree, open a grid project, use every tool, press Play and bring in a level. Plan and decisions are in `plan.md`.

## Run it

From the worktree root (`/Users/ebonura/Desktop/repos/PSoXide-editor-wt/grid-editor`):

```bash
cargo run --release -p frontend --features editor -- --editor --windowed --editor-project editor/projects/grid-cathedral
```

Swap the last path for any grid project: `grid-cathedral`, `grid-level008`, `grid-cave`, `grid-dungeon`, `grid-sewers`, `grid-west`, or a project you made with File > New Grid Project. Leave `--editor-project` off to open the default project and switch with File > Project. The first build takes a few minutes; `make run-release` does the same through the Makefile.

Play and Build shell out to `make build-editor-playtest`, which runs `make bootstrap` first. Bootstrap refuses to continue when a hydrated emulator file has been edited ("Imported file changed"), so keep `emu/crates/frontend/src/cli.rs` and `gfx.rs` as the pinned emulator revision ships them. `.tools/sdk-*/bin/psoxide-components --root . --check` says whether they match.

## Open or create a project

* File > Project lists every project under `editor/projects`. A grid project opens in the 3D view with the grid tool strip across the top.
* File > New Grid Project... asks for a name and makes `editor/projects/<name>/`: a 6 by 6 sector room with a floor, a ceiling, walls on the whole outline, one lamp and the Graybox Reach player in the middle. It borrows the player, camera, sky and UI from `editor/projects/graybox-reach` (the project's `assets` folder links to it), so that project has to exist. Press Play and you are walking in it.
* Levels from bonnie-32 come in with the command line importer only, there is no menu item for it:

```bash
cargo run --release -p psxed-project --bin import-bonnie32-grid -- <level.ron> ~/Desktop/repos/bonnie-32/assets/samples/texture-packs/ editor/projects/<name>
```

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

The strip has eight tools. Wrapped to a second line when the window is narrow. Keys work in the 3D and Top views.

| Tool | Key | Gesture |
| --- | --- | --- |
| Select | V | Click picks the vertex, edge or face under the pointer. Shift+click extends a range, Ctrl/Cmd+click toggles, drag draws a box. Shift+drag raises or lowers the selected corners. Delete removes the selected faces. Ctrl/Cmd+A selects the room. |
| Floor | F | Drag a rectangle; the room grows to fit. Shift while dragging sets the height. Ctrl/Cmd+drag deletes floor. |
| Ceiling | C | Same as Floor, for ceilings. |
| Wall | W | Drag along an edge to place a run of walls; they fill the gap up to the ceiling or the next wall. R turns the direction (auto, N, E, S, W, two diagonals). Ctrl/Cmd+drag removes. |
| Height | H | Drag a vertex, edge or face up and down in 256 unit clicks, Alt for 64. L unlinks corners that share a position. PgUp/PgDn or ]/[ nudge the selection. |
| Paint | P | Click or drag faces to apply the palette material. Ctrl/Cmd+click samples a face's material. |
| Split | D | Click a floor or ceiling to flip its diagonal. |
| Place | O | Click to put the chosen node (light, spawn, actor...) at a sector centre; Alt places freely. |

Also: Ctrl/Cmd+C copies the selected faces, Ctrl/Cmd+V shows a ghost (click stamps, Shift keeps stamping, H and V flip, R turns), Esc cancels a drag, a ghost or the selection, Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z undo and redo, `.` frames the selection, Ctrl/Cmd+S saves.

3D camera: middle or right drag orbits, with Shift it pans, scroll zooms, and with the free camera W A S D fly while the right button is held.

Units: a sector is 1024 grid units, a click 256, the fine step 64. A project's scale maps grid units to editor units (File > New Grid Project uses 5/4, so a sector is 1280 editor units; the imported levels keep the scale their import chose. `GridScale::DEFAULT`, still 21/16, moves to 5/4 with the next phase). Walls belong to the sector edge they are on, with a front side (the interior) and an optional back or both.

## Play

Press Play (or Build to export a disc). The editor cooks the grid document, builds the guest and starts it in the Play viewport. Nothing else is needed: there is no environment variable and no feature flag. What the cook does:

1. Validates the world against the shared caps (room size, wall stack, face budget, coordinate range). A problem stops the cook with a message such as `grid world: room 0 sector 3,4: ...` in the status line and the build log.
2. Derives thin collision slabs from the document (solid walls and walkable surfaces) plus a sky lid, and cooks them through the existing PXBSP path for collision, activation and the camera. Turn `solid` off on a wall or `walkable` off on a surface (Inspector) to walk through it.
3. Writes the render pack from the same faces the editor draws, with the light baked per corner from the Point Light nodes, the room ambient and the slabs as shadow casters.
4. Marks the manifest as a grid world; the guest build picks the grid renderer from that.

The cook warns when a texture is not 64 by 64, when faces have no material the cook can bind and when collision slabs were refused.

## Known gaps

* Camera and character: the starter and imported levels use the Graybox Reach character, which costs most of the frame.
* Brush and BSP authoring still exist for brush projects and are hidden for grid ones; the PXBSP cook is only a transitional bridge for collision.
