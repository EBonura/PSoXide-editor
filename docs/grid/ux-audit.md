# Grid editor UX audit (2026-10-16)

Read-only audit of the PSoXide editor on `grid/main` for the move to a Tomb Raider style grid world. Trigger: "I'm not sure about the materials in the right pane, since we have resources at the bottom."

Nothing was built, run or launched. Everything below comes from reading `editor/crates/psxed-ui/src/` and the frontend host, plus the screenshots in `build/grid-editor/progress/`. Where a claim is an inference from code that I did not see on screen, it is marked **[inferred]**.

All paths are relative to `editor/crates/psxed-ui/src/` unless they start with `emu/` or `docs/`.

## 0. Evidence notes

* Screenshots 04, 05, 14 and 15 are older than the current code. 04 and 05 still show the brush tool strip (Move, Terrain, Global, snap 16) and the Brush menu. 14 and 15 show the grid strip but have no Rooms panel in the left dock and a collapsed "Room 0" section in the Inspector. Commit `0d71f396` added the Rooms panel and moved room settings out of the Inspector. The current look is screenshots 25, 26, 27, 30 and 31. Code is authoritative where they disagree.
* There is no "Room inspector section" any more. Room settings (Raise, Ambient) are in the left Rooms panel (`grid_tools/rooms.rs:172`).
* The tab called **2D** is not the grid plan view. It is the HUD/menu UI scene editor (`WorkspaceView::Ui`, label at `lib.rs:2650-2658`). The grid's 2D plan is the **Top** view, which lives inside the **3D** tab and is switched through an icon-only "Viewport" menu (`workspace/toolbars.rs:464-470`, `1253-1270`).
* Class legend: **grid** (needed by a grid designer), **brush** (BSP/brush leftover, going away per `docs/grid/deletion-review.md`), **general** (project/game level, stays).

## 1. Map of the UI

Draw order is in `workspace/dialogs.rs:59-86`: action bar, left dock, inspector (or Play performance column), grid status bar and help, bottom Resources dock, centre viewport, then dialogs.

| # | Region | Contents | Drawn at | Class |
|---|---|---|---|---|
| 1 | Frontend host | In editor mode the editor owns the whole frame. The emulator overlay menu opens from File > Emulator Menu. Toasts, freecam and recording badges float top right. | `emu/crates/frontend/src/ui/mod.rs:55-86`, `383`, `428` | general |
| 2 | Title / action bar | PSoXide logo, project name with a pen button and unsaved star, then menus, then Copy/Paste buttons, Build, Play split button, Stop, Debug, and a status strip (badge plus message). | `workspace/input.rs:822-880`, identity `895`, strip `workspace/panels.rs:6` | general, with brush bits |
| 2a | File menu | New Project, New Grid Project, Project submenu, Delete Project, Save, Reload, Emulator Menu, Quit. | `workspace/input.rs:1011-1065` | general |
| 2b | Edit menu | Copy/Paste Geometry (brush clipboard), Group/Ungroup, Duplicate, Delete Selection. No Undo or Redo entries. | `input.rs:1066-1158` | mostly brush |
| 2c | Brush menu | Extrude, Hollow, Subtract. Hidden in grid worlds. | `input.rs:1160` (guard at `1159`) | brush |
| 2d | View menu | Toggles for "World and files", "Resources", "Inspector"; Frame Selection; switch to 3D, UI, Animation, Material workspaces. | `input.rs:1201-1235` | general |
| 2e | Tools menu | Check Overlapping Brush Faces, Build, Play, Stop. | `input.rs:1237-1272` | brush plus duplicates |
| 2f | Help menu | Three static lines. Does not mention the `?` key page. | `input.rs:1274-1279` | general |
| 2g | Copy selection / Clipboard empty | Brush clipboard buttons, always greyed in a grid project (screenshots 25, 26, 27, 30, 31). | `input.rs:836-870` | brush |
| 2h | Build / Play / Debug | Build, Play with chevron (boot target, **BSP cook** mode, PSX budget rows BSP/PVS/Lighting/RAM/VRAM/Packets), Stop, Debug (writes `logs/editor_debug.log`). | `input.rs:1281-1500`, BSP cook `1365` | general, BSP bits |
| 3 | Workspace tabs | 3D, 2D (HUD), Animation, Material. Cmd+1 cycles them. | `workspace/toolbars.rs:356-390`, `input.rs:301-341` | general |
| 4 | Tool strip row | Grid: Select, Floor, Ceiling, Wall, Height, Paint, Split, Place (keys V F C W H P D O). Then the active tool's options, a read-only "Snap 256 (Alt 64)" label, a `?` button. Then four icon-only groups: Visibility, Camera, Frame (`.`), Viewport (3D/Top/Front/Side). The row wraps when narrow. | `grid_tools/strip.rs:10-112`, groups `workspace/toolbars.rs:439-470` | grid |
| 4a | Tool options | Floor/Ceiling height in clicks; Wall direction; Link toggle (Select, Height); Place shows the `+` kind menu and the kind name; Paint shows "Material: name". | `strip.rs:32-98` | grid |
| 4b | Brush strip (not shown in grid) | BSP mode strip, `+` add menu, Terrain menu, gizmo controls, grid/snap controls, Snap level. | `toolbars.rs:420-440`, `510-1000`, `837-935` | brush |
| 5 | Left dock | Top half: Rooms panel (3D workspace only) above the Scene Graph; splitter; bottom half: FileSystem. | `workspace/panels.rs:102-196`, Rooms at `146-147` | mixed |
| 5a | Rooms panel | Eye, name (click makes active, double click renames), sector count, y offset, bin; `+`; Raise and Ambient for the active room. | `grid_tools/rooms.rs:52-228` | grid |
| 5b | Scene Graph | Filter box, node tree with eyes, `+` menu (Entity, Group, Folder only), Duplicate, Delete. Holds the World root, entities, lights. | `panels.rs:198-281`, rows `scene_tree.rs:447` | general |
| 5c | FileSystem | Filter box and a tree: `res://`, `maps/main.map`, then one folder per resource type (materials, models, animations, characters, weapons, meshes, other). Built from the resource list. | `panels.rs:952-1015`, `resource_browser.rs:14-85` | general, duplicate |
| 6 | Centre viewport | 3D view (emulator GPU picture, 4:3 letterboxed) or Top/Front/Side. Tabs and strip sit in its header. | `workspace/toolbars.rs:4-317`, 3D body `workspace/viewport.rs:55` | grid |
| 6a | Info box | "Top Orthographic / Snap / N nodes / N resources / zoom". Top, Front, Side only. | `viewport2d.rs:3-51` | general |
| 6b | Room chips | Checkbox plus name per room, Top view only, only with two or more rooms. | `grid_view.rs:275-345`, call `toolbars.rs:207` | grid, duplicate |
| 6c | Grid overlays | Selection, hover and ghost drawing in 3D and Top. | `grid_tools/overlay.rs:139`, `351` | grid |
| 6d | Front/Side placeholder | Text "Grid worlds are edited in the Top and 3D views". | `toolbars.rs:271-280` | dead for grid |
| 6e | `?` key help window | Every tool key and gesture. | `grid_tools/help.rs:102` | grid |
| 6f | Drop hint | "Drop resource into scene". | `toolbars.rs:299`, `viewport.rs:310` | misleading in grid (see 3.3) |
| 7 | Right dock: Inspector | In order: grid Materials palette, grid Selection panel, then (brush inspector, UI inspector, resource inspector or node inspector). With nothing selected the node inspector shows the **World** node. | `panels.rs:1783-2193`, grid call `1840-1843` | mixed |
| 7a | Materials palette | "Show all project materials" checkbox, 44 px swatches in a scroll area capped at 236 px. Click sets the paint material and applies it to selected faces. | `grid_tools/panels.rs:49-132` | grid |
| 7b | Selection panel | Face count, element name, heights, Material (read-only text), Split flip, UV offset/scale/angle, Flip U/V, Rotate 90, Reset, wall Faces and Solid, surface Walkable. | `grid_tools/panels.rs:134-380` | grid |
| 7c | World sections | World Message, Physics, Sky (Test sky presets, mode, visibility, texture), Culling (incl. BSP Patch Extent), Far Vista. | `inspector_transform_node.rs:105-640` | general, brush bits |
| 8 | Bottom dock | Tabs Resources and Console. Resources: `+` menu (Projectile, Boost Module, Weapon, Character Profile, Animation Source, Clip Role Map, Material, Transition Material, Import Model, Import Texture, Catalogue Animation, Animation Viewer, Starter Content), a filter column with counts, and 120 by 128 px cards. Console: build and guest log. | `workspace/panels/resources.rs:468-790`, `1073-1247` | general |
| 9 | Grid status bar | Last message (green), tool key hints, then room target and sector count, world rooms and faces against 16383, faces in view. Sits under the centre column only. | `grid_tools/strip.rs:115-233` | grid |
| 10 | Material tab | Name, version, searchable material picker, New, New Transition, Duplicate, New Version, Save, "Select uses", "Replace uses"; settings column and preview. | `material_lab.rs:74-306`, body `307` | general |
| 11 | Animation tab | Model and animation viewer, big authoring tool. Left and right docks stay as they are. | `model_animation_viewer.rs:893`, `1212` | general |
| 12 | 2D (HUD) tab | UI scene canvas; left dock swaps to the UI tree, Inspector swaps to the UI inspector. | `workspace/ui_scene.rs:138`, `panels.rs:144-145`, `1018` | general |
| 13 | Play | Viewport shows the guest, Inspector column can swap to guest performance (F3). | `viewport.rs:720`, `panels.rs:1741` | general |
| 14 | Dialogs | New Project (has a "BSP cook quality" radio), New Grid Project, Delete, Unsaved changes, Texture import, Model import, Terrain, Brush overlap. | `workspace/dialogs.rs` (New Project cook radio `122`) | mixed |

## 2. Functionality index

"Where" lists every UI place that does it. Verdicts: OK, DUP (duplicate), NEAR (near duplicate with different scope), DEAD, WRONG (present but does the wrong thing for a grid).

### 2.1 Project and file

| Function | Where | Verdict |
|---|---|---|
| Open or switch project | File > Project (`lib.rs:3953`) | OK |
| New brush project | File > New Project, Cmd+N (`input.rs:79`, `139`) | WRONG: the grid editor's default new-project shortcut makes a BSP project, with a "BSP cook quality" radio (`dialogs.rs:122`) |
| New grid project | File > New Grid Project (`input.rs:1020`) | OK, but not on the shortcut |
| Save | File > Save, Cmd+S, Material tab "Save" (`material_lab.rs:202`) | DUP (three), harmless |
| Reload, Delete project | File menu | OK |
| Undo and redo | Cmd+Z, Cmd+Shift+Z only (`input.rs:87-88`, `148-150`; help page lists them at `grid_tools/help.rs:75`). No button, no Edit menu entry. | missing UI |
| Build | Build button, Tools > Build Project, Cmd+B | DUP, normal |
| Play, Rebuild, Stop | Play button, Tools > Play, Cmd+Enter, Stop button, Tools > Stop | DUP, normal |
| Boot target, BSP cook mode, PSX budget | Play chevron (`input.rs:1325-1420`) | BSP parts are brush |
| Build log | Console tab (`resources.rs:797`) | OK |
| Debug snapshot | Debug button (`input.rs:1492`) | general dev tool |

### 2.2 Materials and textures (the user's question)

| Function | Where | Verdict |
|---|---|---|
| Browse materials | (1) right-dock Materials palette `grid_tools/panels.rs:49`; (2) bottom Resources "Material" filter, `resources.rs:1073`; (3) FileSystem `materials/` folder, `resource_browser.rs:44`; (4) Material tab picker, `material_lab.rs:83`; (5) Resource inspector "Material" section, `resources.rs:191` | five lists of one set |
| Choose current paint material | Palette click sets `grid.material`. A Resources or FileSystem click sets `brush_material` (`workspace/selection.rs:23-27`), which the grid never reads. | WRONG: two "current material" states |
| Apply material to selected faces | Palette click (works, `panels.rs:79-80`). Resources card click uses `selected_material_targets()` (`resources.rs:1209`), which only returns brush faces (`workspace/painting.rs:1364-1370`). | palette OK, Resources WRONG (no effect on grid faces, it just opens the resource) |
| Apply by drag and drop | Drag a card onto the 3D or Top view calls `drop_resource_bsp_3d` or `drop_resource_2d` (`viewport.rs:159-163`, `toolbars.rs:166-175`), which pick brush faces only (`painting.rs:288-345`) and answer "Drop the material onto a brush face" | WRONG **[inferred]** |
| Paint tool | P, drag over faces, Ctrl+click samples (`grid_tools/face_tools.rs:95`, `183`) | OK |
| Palette follows selection | Selecting a face scrolls the palette to its material (`select_tool.rs:37`) but only scrolls, the highlight shows the paint material, not the face's | partial |
| Show the face's own material | Selection panel row "Material" is plain text (`grid_tools/panels.rs:228-231`) | weak, no swatch, not a picker |
| Create a material | Resources `+` > Material (`resources.rs:665`), Resources `+` > Transition Material, Material tab New / New Transition / Duplicate (`material_lab.rs:~168-185`) | DUP; the palette has no `+` |
| Import an image as a material | Resources `+` > Import Texture. Files go to `assets/textures/` and register a Material (`psxed-project/src/texture_import.rs:300-382`). | the palette hides it, see below |
| Palette filter | A material shows only if the world uses it, its path starts with `level_textures/`, or its name starts with "Graybox" (`psxed-project/src/grid/palette.rs:23-31`, `45-55`). A freshly imported or created material matches none, so it is invisible until "Show all project materials", which adds HUD, menu, sky and character images (screenshots 30 vs 31 vs the 36 to 44 material count in Resources). | WRONG for the import loop |
| Edit material settings | Material tab (`material_lab.rs:496`), Inspector when a Material resource is selected (`resources.rs:191-205`, same `draw_material_settings`), ModelRenderer "Material Override" (`panels.rs:2094`) | DUP (three surfaces for one editor) |
| Find or replace uses | Material tab "Select uses" and "Replace uses" (`material_lab.rs:103-126`), tooltips say brush faces | brush-blind **[inferred from tooltips]** |
| UV edit | Selection panel only (`panels.rs:268-312`). No UV canvas for grid (`workspace/uv_canvas.rs` is brush). | OK |

### 2.3 Rooms

| Function | Where | Verdict |
|---|---|---|
| Add, delete, rename room | Rooms panel only (`rooms.rs:63-170`) | OK |
| Show or hide a room | Rooms panel eye (`rooms.rs:97-107`) and Top view chips checkbox (`grid_view.rs:322-330`) | DUP, different widgets (eye icon vs checkbox), chips Top-only |
| Choose the active room | Rooms panel name click (can set, cannot clear), chips name click (toggles, can clear, `grid_view.rs:333`), and implicitly by selecting a face (`select_tool.rs:35` sets `last_room`, which loses to an explicit active room, `grid_tools/mod.rs:370-379`) | NEAR, two states `grid_active_room` and `grid.last_room` |
| Raise and ambient | Rooms panel, applies to the active room (`rooms.rs:172-205`) | OK |
| Fog | Data exists (`psxed-project/src/grid/room.rs:72`), no UI. The World Sky and Far Vista sections have a "Match world fog" checkbox (`inspector_transform_node.rs:445`, `627`) that points at nothing visible. | missing, confusing |
| Room size, origin | no UI, the room grows with the Floor tool | acceptable |
| Room stats | status bar "room N name: M sectors" (`strip.rs:196-212`) | OK |

### 2.4 World content (lights, sky, physics, spawns)

| Function | Where | Verdict |
|---|---|---|
| Add a light, spawn, enemy | Place tool, kind from the `+` menu that is visible only while Place is the active tool (`strip.rs:76-83`). The Scene Graph `+` offers Entity/Group/Folder only and its tooltip says "Place runtime objects from the toolbar Add menu" (`panels.rs:212`). | hard to find |
| Choose which model, character or image to place | The picker `draw_active_place_options` is wired only into the brush strip (`toolbars.rs:688`). In grid, `resolve_place_model_resource` needs a Model selected in Resources/FileSystem, auto-picks only when exactly one exists, otherwise sets a status message "Select a Model resource before placing a prop (N available)" (`toolbars.rs:1420-1444`). Screenshots show Model (9). | WRONG **[inferred]** |
| Place by drag from Resources | `drop_resource_*` requires a brush surface (`painting.rs:288-345`) | WRONG **[inferred]** |
| Select a node | Scene Graph click; Select tool click on the node in 3D or Top (`grid_tools/events.rs:161-218`) | OK |
| Move a node | Select tool gizmo in 3D/Top, Inspector Position fields | OK |
| Gizmo mode Move/Rotate/Scale | Only through Cmd+3 (`input.rs:317-319`). The grid strip has no gizmo control (`strip.rs` never calls `draw_transform_gizmo_toolbar_controls`). | hidden |
| Delete a node | Scene Graph Delete, Edit > Delete Selection, Delete key. In grid, Delete is consumed first by the face delete (`grid_tools/keys.rs:76`). | NEAR, one key two meanings |
| Duplicate | Scene Graph button, Edit > Duplicate, Cmd+D | DUP, normal |
| Light colour, radius | Inspector node properties | OK |
| Sky | World sections: "Test sky" presets Procedural / Quake layered / Sunset cube plus mode, visibility, texture (`inspector_transform_node.rs:154-200`). The helper text talks about "sky-aperture faces", a brush concept. | presets confusing, brush wording |
| Physics, draw distance, far vista | World sections (`inspector_transform_node.rs:136`, `527`, `548`). Culling also has "BSP Patch Extent" (`542`). | general, one brush field |
| Hide a node in the view | Scene Graph eye | NEAR to the Rooms eye (nodes vs rooms) |
| Show lights, bounds | Visibility menu (`toolbars.rs:1161-1185`), which also holds "Brush wireframes" | brush row |

### 2.5 Geometry tools

| Function | Where | Verdict |
|---|---|---|
| Floor, ceiling, wall, height, split, paint, select | Tool strip and keys only (`strip.rs:10`, `keys.rs`) | OK |
| Copy, paste faces | Ctrl+C, Ctrl+V (`keys.rs:29-34`). The top-bar Copy/Paste buttons and Edit > Copy Geometry are brush clipboard and stay disabled (screenshots 25 to 31). | DEAD in grid |
| Group, ungroup | Edit menu, Cmd+G | brush (groups are going away per the deletion review) |
| Snap | Read-only label in the strip (`strip.rs:101`), repeated in the Top info box (`viewport2d.rs:29-34`), and in the key hints (Alt fine). The brush snap menu is hidden. | DUP labels, not changeable (by design) |
| Link vertices | Strip toggle, key L | OK |
| Frame selection | Strip button, View menu, key `.` | DUP, normal |
| Help | `?` strip button, `?` key, status bar hints, tool tooltips, Help menu (3 lines, no mention) | DUP (four), Help menu empty |

### 2.6 Views and camera

| Function | Where | Verdict |
|---|---|---|
| 3D vs Top | Icon-only "Viewport" menu in the strip (`toolbars.rs:464-470`), Cmd+9 cycle. The tab "3D" holds both. | hidden |
| Front, Side | Same menu, shows only a text notice in grid (`toolbars.rs:271-280`) | DEAD |
| Orbit or free camera, zoom speed | Camera menu (`toolbars.rs:1190`). It also carries BSP leak-path items when a leak exists. | OK, brush bits |
| Both views at once | not possible | missing (bonnie-32 had it) |
| Panels on or off | View menu "World and files", "Resources", "Inspector" (`input.rs:1203-1213`) | label wrong, the dock holds Rooms, Scene Graph, FileSystem |

### 2.7 Duplicates summary (explicit)

1. **Materials**: palette (right) vs Resources Material filter (bottom) vs FileSystem materials folder (left) vs Material tab picker vs Resource inspector. Two different "current material" states, and only the palette drives grid faces.
2. **Material settings editor**: Material tab, Resource inspector, ModelRenderer override. Same widget (`draw_material_settings`), three homes.
3. **Rooms**: Rooms panel vs Top chips vs status bar target readout, plus two active-room states.
4. **Resource browsing**: Resources dock (type filter chips) vs FileSystem (type folders). Same list, two views, FileSystem has a fake `main.map` row that does nothing (`resource_browser.rs:34-42`, rows without a resource produce no action at `:149-155`).
5. **Status text**: the top-bar strip and the grid status bar both print `self.status` (`panels.rs:6-50` and `strip.rs:187-200`; both show "Tool: Floor (F)" in screenshot 25).
6. **Eyes**: Rooms eye, Scene Graph eye, Visibility menu.
7. **Help**: four entry points, and the Help menu is the empty one.
8. **Save, Build, Play, Frame**: menu, button and key. Normal, keep.
9. **World settings vs face settings**: both always stacked in the Inspector.
10. **Snap label** in three places.

### 2.8 Dead or confusing entries

* "Test sky: Procedural / Quake layered / Sunset cube" presets and the "sky-aperture faces" help line (`inspector_transform_node.rs:154-200`). Brush-era test helpers that force "Always visible".
* Brush leftovers visible in a grid project: Tools > Check Overlapping Brush Faces (`input.rs:1241`), Visibility > Brush wireframes (`toolbars.rs:1173`), Play chevron "BSP cook" and "BSP / PVS" budget rows (`input.rs:1365`, `1382-1392`), New Project "BSP cook quality" (`dialogs.rs:122`), Culling "BSP Patch Extent" (`inspector_transform_node.rs:542`), Copy selection / Clipboard empty buttons (`input.rs:836-870`), Edit > Group/Copy Geometry, Place > Destructible (comment at `lib.rs:2547`: "owner for BSP brushes"), Cmd+K hollow/subtract shortcuts still consumed in the 3D workspace (`input.rs:132-135`, `162-167`), Camera menu leak-path buttons (only with a leak).
* `main.map` row in FileSystem for a grid project (`resource_browser.rs:34-42`).
* Front and Side views in a grid project.
* Top info box "N nodes / N resources" (`viewport2d.rs:38-39`).
* View menu label "World and files".
* Help menu.
* Cmd+2 cycles the brush tool modes (`input.rs:314-316`, `343-349`). **[inferred]** it has no meaningful effect on the grid tool but writes a "Mode: ..." status.
* Material Lab "Reflective probe retired" notice (`material_lab.rs:1123`).
* `?` button shows as a bare "?" with no label (`strip.rs:106`).

## 3. Problems ranked by impact on a grid designer

### 3.1 Materials: five homes, one that works, import loop broken (highest)

Evidence in 2.2. Concretely, a designer who imports a PNG (Resources `+` > Import Texture) lands in the Inspector's resource panel, finds no such swatch in the right-hand palette (filter at `palette.rs:23-31`), ticks "Show all" and gets HUD, sky and character images mixed in (screenshot 31 shows 3 swatches without it, screenshot 25 shows 12 and screenshot 14 shows 24 including prompt glyphs). Clicking the card in the bottom Resources dock, which is where an import leaves them, selects a resource but does not paint anything and does not change the paint material. Dragging the card into the 3D view reports a brush-only error. This is the user's instinct exactly: the right palette and the bottom browser overlap, and only one of them is wired to the grid.

Also: the face's own material is plain text in the Selection panel, not a thumbnail or picker, and there is no `+` / Import in the palette.

### 3.2 Inspector shows the wrong context and too much of it

`panels.rs:1840-1843` always draws the palette and the face Selection panel first, whatever is selected. A selected light, spawn or enemy sits under roughly 330 px of material swatches plus the sentence "Select faces with the Select tool" (screenshots 25, 30, 31). With a face selected, the World settings (Physics, Sky, Far Vista, expanded by default) follow immediately, because the grid selection only resets node selection to the World root (`select_tool.rs:36`, `workspace/selection.rs:11-15`), so the World inspector becomes the "nothing selected" fallback (screenshots 14, 25, 30). The grid selection does not clear a stale resource selection **[inferred]**: `panels.rs:1856-1859` returns the resource inspector ahead of the node inspector, so after clicking any resource the face panels are followed by that resource, not the World.

The inspector has no header telling you what you are editing (face(s), node, room, world).

### 3.3 Place and drag from Resources do not work for grid worlds

See 2.4. The grid strip shows the `+` kind menu only for Place and not the per-kind picker, so Model/Character/Image props need a pre-selected resource, with failure reported only through the 11 px green status text (shown twice). Drag and drop of models, characters and materials goes through brush-surface lookups. **[inferred]** from `painting.rs:288-345` and `tools.rs:434-442`, not exercised.

### 3.4 Top view is hidden and cannot sit next to 3D

The "2D" tab means HUD, the plan view is a menu item behind an icon-only button, and you cannot see plan and 3D together. bonnie-32 showed both and Manny liked that (`docs/grid/plan.md` section 2: "Left: 2D grid"). The grid plan today replaces the centre. Screenshots 25 to 31 also show the 3D picture is a 4:3 box (about 650 px wide inside a roughly 990 px panel), so roughly a third of the centre width is empty while the bottom dock steals height, which is the binding constraint for the 4:3 picture.

### 3.5 Rooms: three places, missing fog, two active states

See 2.3. The Rooms panel appears only in the 3D workspace (`panels.rs:146`), shrinks the Scene Graph (6 visible rows in screenshot 25), has no fog and no way to unpin the active room, while the chips can. Fog data exists with no UI.

### 3.6 No visible undo or redo

Keyboard only (`input.rs:87-88`). The plan says "Undo and redo as today", but bonnie-32 had toolbar buttons (`docs/grid/bonnie32-editor-inventory.md` section 2.7). In a mouse-driven tool that is a real gap.

### 3.7 Left dock spends a third on FileSystem

FileSystem lists the same resources as the Resources dock, with a fake file (`main.map`), names like `menu_frame_overlay_full.psxt` and `picotron_grid_64.psxt` (screenshot 25), and gets about 330 px of the dock.

### 3.8 Brush and BSP leftovers

Listed in 2.8. They are mostly going away anyway (deletion review stages 1 to 8); until then they make a grid project look like a half-converted brush project, notably the dead grey Copy/Clipboard buttons and the BSP cook items in Play and New Project.

### 3.9 Message and hint surfaces are small and doubled

Status text twice (2.7 item 5), tool hints and the face-budget readout in 11 px mono at the bottom (`strip.rs:187-229`), room readout "room 0 Room 0: 720 sectors" repeating the name. The budget (`world: 1 rooms, 1662 faces / 16383 (10%)`) is the key grid limit and is the hardest text to see on the screen.

### 3.10 Material Lab and Resource inspector overlap, "Select uses" is brush-only

3.1 covers browsing; for editing, the Material tab and the right-dock resource inspector are the same editor (`material_lab.rs:496` vs `resources.rs:191-205`), so there are two places that look different and do the same.

### 3.11 Smaller items

* Cmd+N creates a brush project (`input.rs:139`).
* Gizmo mode only by Cmd+3.
* Delete key means faces first, nodes otherwise.
* Place `+` hint on the Scene Graph points to a button that is usually not on screen.
* The `?` page is the only complete reference and the Help menu does not mention it.

## 4. Proposals

### 4.1 Recommended target layout (bonnie-32 family, PS1 picture stays central)

```
+------------------------------------------------------------------------------------+
| logo Name*  File Edit View Tools Help | Undo Redo | Build  Play v  Stop | READY     |
+------------------------------------------------------------------------------------+
| [World] [Animation] [Material] [HUD]   (tabs; HUD was "2D")                        |
| V Select F Floor C Ceil W Wall H Height P Paint D Split O Place | options | ?       |
|                                        [ 3D | Top | Split ]  vis  cam  frame       |
+-----------------+---------------------------------------------+--------------------+
| Plan (Top mini) |                                             | Materials          |
|  2D grid, all   |                                             |  [search] [+][Imp] |
|  rooms, click   |        PS1 3D VIEWPORT (centre)             |  swatch grid       |
|  to select      |        4:3 picture, same as today           |  (world set, All)  |
|-----------------|                                             |--------------------|
| Rooms      [+]  |                                             | Properties         |
|  eye name (n)   |                                             |  header: what is   |
|  Raise Ambient  |                                             |  selected          |
|  Fog            |                                             |  Face(s) | Node |  |
|-----------------|                                             |  Room | World      |
| Scene Graph     |                                             |  (one at a time)   |
|  (nodes only)   |                                             |                    |
+-----------------+---------------------------------------------+--------------------+
| Assets / Console drawer (collapsed by default: models, animations, characters,     |
| audio, console log; opens when Place needs a model)                                |
+------------------------------------------------------------------------------------+
| status message | tool key hints                    room 0: 36 sectors | 1662/16383  |
+------------------------------------------------------------------------------------+
```

Rules behind it:

1. **One home per function.**
   * Materials: one panel in the right dock. It is the palette, a search box and `+ New` / `Import` buttons, backed by the same list as the Resources "Material" filter. Remove Material cards from the bottom dock for grid projects, delete the FileSystem materials folder.
   * Rooms: one panel (left). Chips removed.
   * Resources that are not materials (models, characters, animations, audio): bottom drawer, collapsed by default.
   * Material settings editing: Material tab only. In the Inspector a Material resource shows a thumbnail, name and an "Open in Material tab" button. Remove the duplicate `draw_material_settings` call.
2. **Properties is context driven**, with a header. Faces selected: face properties only. Node selected: node properties only. Nothing selected: the active Room (ambient, raise, fog). World settings are reached by clicking the World row in the Scene Graph (already the root row) and are not shown below face selection.
3. **Visible 3D / Top / Split switch** in the strip, replacing the icon-only Viewport menu; rename tab "2D" to "HUD" and the "3D" tab to "World".
4. **Plan mini-view** in the left dock reuses `grid_view::draw_grid_world` (`grid_view.rs`), so Top and 3D are visible together without a full Split.
5. **Undo and Redo buttons** next to the title; Edit menu entries.
6. **Place** shows its kind menu and the resource picker in the strip whenever Place is active.
7. **One status text**: bottom bar keeps message, hints and budget; the top strip keeps only the badge (READY, UNSAVED, BUILDING, PLAY).
8. Bottom drawer closed by default wins back height, and the right dock uses the horizontal space the 4:3 picture already leaves empty.

### 4.2 What to merge, move, hide by context or delete

| Action | Item | Cost |
|---|---|---|
| Hide in grid | Copy/Paste buttons, Edit > Copy/Group, Tools > Check Overlapping, Visibility > Brush wireframes, Play chevron BSP cook and BSP/PVS rows, New Project BSP radio, BSP Patch Extent, Place > Destructible, Camera leak items | cheap (guards) |
| Hide in grid | Front/Side entries, Top info box node/resource counts, "Test sky" presets and sky-aperture text | cheap |
| Delete | FileSystem `main.map` row | cheap |
| Fix | Help menu: one line and a "Keys (?)" entry | cheap |
| Fix | Cmd+N: make New Grid Project the default for grid editor, or add a chooser | cheap |
| Fix | Grid selection clears the resource selection (`select_tool.rs:36` plus `clear_resource_selection_state`) so the World or resource inspector never follows a face selection | cheap |
| Fix | Rooms panel: allow unpin of the active room, or drop the "active vs last" split | cheap |
| Fix | Rename tab "2D" to "HUD"; fix View menu label "World and files" | cheap |
| Add | Undo/Redo buttons and Edit entries | cheap |
| Add | Visible 3D / Top segmented control | cheap |
| Add | Palette `+ New`, `Import`, and make newly created or imported materials appear (for example: show every material whose path is not under a UI or sky folder, or tag the project's materials) | cheap to medium |
| Merge | One "current material" state (delete `brush_material`, point Resources card click, drag-drop and the palette at `grid.material`) | medium |
| Merge | Resources Material cards and the palette into one panel; wire grid-face apply to card click and to drag-drop on grid faces | medium |
| Move | Place resource picker into the grid strip | medium |
| Move | Rooms: fog, and the Top chips removed; Rooms shown in every workspace that has a viewport of the world | medium |
| Gate | Inspector context header and per-selection content (3.2) | medium |
| Move | FileSystem out of the left dock; Scene Graph gets the space | medium |
| Add | Left Plan mini-view | big |
| Add | 3D + Top Split view | big |
| Delete | Brush systems themselves | separate effort, `docs/grid/deletion-review.md` |

### 4.3 Alternatives where the trade-off is real

* **A (recommended)**: palette in the right dock, merged with the bottom Material cards, bottom drawer collapsed. Cost: single material list wiring. Gain: one place, close to Properties, uses the empty side space of the 4:3 picture.
* **B**: the bottom Resources dock becomes the only material browser (Godot or Unreal style), the right dock is Properties only. Gain: one browser for everything, large thumbnails. Loss: the palette is far from Properties and from the face under the pointer, and the dock takes viewport height, which is the scarce direction. Not recommended for the paint loop.
* **C**: Keep the right palette, keep Resources at the bottom, but make them one list (a "Materials" filter in the bottom dock mirrors the palette, both set the same state). Cheapest of the real fixes; leaves two surfaces for one set, but removes the wrong behaviour.
* **D**: no mini plan, just a visible `3D | Top | Split` control. Cheaper than the Plan mini-view, still gives plan and 3D together via Split.
* **E (palette as popover)**: a hotkey opens a floating swatch palette at the pointer (bonnie-32 style quick paint). Pairs with A or C, not a replacement.

### 4.4 Suggested order

1. Cheap guards and fixes (hide brush items, undo buttons, status duplication, clear resource selection, tab names, `main.map`).
2. Single current-material state and Resources click/drag wiring (kills the worst bug in 3.1).
3. Inspector context header and per-selection content.
4. Place picker in the strip.
5. Materials panel merge, FileSystem out, Rooms fog.
6. Plan mini-view or Split.

## 5. Design smells to track separately

* `EditorWorkspace` still carries two tool states: `grid.tool` is mirrored into `active_tool` (`grid_tools/mod.rs:383-387`), so the grid piggybacks on the brush `ViewTool`.
* Place from grid goes through `place_bsp_at_world_hit` and `bsp_authoring_root` (`workspace/painting.rs:59`, `214`); the names are BSP, the behaviour is generic.
* Two material states (`brush_material`, `grid.material`) and two active-room states (`grid_active_room`, `grid.last_room`).
* `draw_inspector` selection priority is a long ladder (`panels.rs:1836-1870`) where grid panels are prepended instead of being one of the cases.
* `Rows` for the `?` page, tool tooltips and the status bar hints repeat the same text in three places (`help.rs`, `grid_tools/mod.rs:95-115`, `strip.rs:115-131`).
