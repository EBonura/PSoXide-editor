# Grid world editor plan (2026-10-16)

Cortex moves back to a Tomb Raider-style sector grid world, authored with a port of the bonnie-32 level editor. This work lives on `grid/main`; `main` and the integ line stay BSP so we can go back.

Evidence behind this plan (copied into `docs/grid/`):

- `bonnie32-editor-inventory.md`: bonnie-32 v1 data model, every editor operation with code locations and port cost.
- `psoxide-editor-architecture.md`: current PSoXide editor, project model, cook, preview, MCP, seams.
- `old-grid-system-archaeology.md`: the grid room system deleted on 10-08, why, and what to salvage (sha:path).
- `other-maps-survey.md`: the five other bonnie-32 maps.
- Cathedral race, headless emulator, Graybox Reach character: grid world about 250 to 300k cycles per frame against BSP 377 to 460k. The character (about 460k) is now the bigger cost.

## 1. What we are building

The PSoXide editor gains a Grid world kind. A grid project is edited the way bonnie-32 levels were: paint floors and ceilings by dragging rectangles, raise and lower corners in 256-unit clicks, drop walls along cell edges, split cells diagonally, click a texture to apply it, and press Play to walk it on the PS1 runtime. Brush projects keep working unchanged.

What we port from bonnie-32 is the data model and the editing behaviour. Its UI (macroquad plus 6,500 lines of custom widgets) and its 5,360-line viewport function are not liftable; the editor stays egui with one tool per state machine.

## 2. Target UX

Same layout family as bonnie-32 (see `bonnie-32/docs/screenshot-world-editor.png`), inside the existing PSoXide shell:

- Centre: the 3D viewport, still the emulator's software GPU so what you see is the PS1 picture, with a grid walker instead of the brush walker.
- Left: 2D grid (Top / Front / Side), then Rooms (visibility, add, delete, ambient, fog).
- Right: Materials palette (click applies to the selection), then the Properties inspector for the selected cells, faces, walls or nodes.
- Top toolbar, one key each: Select (V), Floor (F), Ceiling (C), Wall (W), Height (H), Paint (P), Split (D), Place (O). Undo and redo as today.
- Status bar: live key hints for the active tool and a budget readout (tiles in room, visible tiles from the camera, cook caps).

Core gestures to reproduce, in priority order (from the inventory's top 15):

1. Drag a rectangle of floor or ceiling; the room grid auto-grows; Shift sets height while dragging.
2. Wall tool: drag a run of walls along edges, R rotates direction, gap fill follows sloped floors and ceilings, stacked segments per edge.
3. Shift-drag a vertex, edge or face in Y with 256 snap (64 with Alt); coincident corners move together unless unlinked (L).
4. Depth-aware picking: vertex, then edge, then face, in screen space.
5. Multi-select: Shift range, Ctrl toggle, box select, Ctrl+A.
6. Click a material to apply it; the palette scrolls to the selected face's material.
7. Copy and paste geometry with H/V flip, R rotate and a ghost preview.
8. Diagonal split with per-triangle material and heights; diagonal walls.
9. UV offset, scale, rotate, flip, reset in the inspector.
10. Lights, spawns, enemies and triggers stay ordinary scene nodes, snapped to cell centres by default.

## 3. Decisions

Taken on my recommendation (each is one field or one module to change if Manny disagrees):

- **D1 Units.** The grid document stores integers only: a sector is 1024 grid units, a click is 256, fine snap 64. A per-world scale (rational, `num/den`) maps grid units to editor units. New worlds default to 21/16 (sector = 1344 editor units), which gives the 1024-unit Cortex character Tomb Raider proportions (Lara 762 against a 1024 sector). The bonnie importer sets the scale per level from bonnie's player height (Cathedral 9/16, TR-scale maps 21/16, 1200-player maps 7/8).
- **D2 One source of truth.** The grid document in `project.ron` is the world. Render pack, collision and light bake are all derived from it at cook time. The raw bonnie file is read only by the importer.
- **D3 Collision.** The prototype derives thin collision brushes from the grid (the importer's `slab()` logic, already proven in the race) and reuses PXBSP collision. Native O(1) sector collision from the salvaged `floor_sample.rs` comes later.
- **D4 Coexistence.** `Scene` gets a world-kind discriminator that defaults to Brushes, so every existing project loads unchanged. The old `Room(` kind name stays rejected; the new kind uses a new name.
- **D5 Walls have one owner.** Each wall belongs to exactly one sector edge (N/E/S/W plus the two diagonals), never shared between neighbours. The old system's East-of-A versus West-of-B ambiguity is not repeated.
- **D6 Caps are checked once.** Room size, tile count and cook caps live in one module used by the editor budget readout, the cook and the runtime, so oversize rooms fail loudly in the editor, never silently at runtime.

Open for Manny later (not blocking the prototype):

- Stacked rooms (rooms above rooms) and how the 2D view switches floors.
- Portals and visibility: TR portals plus per-cell visibility versus distance cut plus fog only.
- Streaming: salvage the old room streamer's portal-hop prefetch on top of the current region streamer.
- Doors, liquids and destructibles, which are brush features today.

## 4. Phases

Each phase leaves the editor working, with tests, lint clean, and screenshots before it is called done.

**P1 Data model and import.** `GridWorld` in psxed-project: rooms (origin, width, depth, ambient, fog), sectors (optional floor and ceiling with four corner heights, split direction, per-triangle material and UV, wall lists per edge), all integer. Serde with defaults, validation, caps (D6). Import bonnie-32 levels into the document (Cathedral, level_008, Cave, Dungeon, Sewers, West), including the survey's importer fixes: per-pack texture lookup, empty texture fallback, level lights or generated lights, `_USER` texture. Undo uses the existing document snapshots.

**P2 Seeing it.** Grid walker in `emu/crates/frontend/src/editor_preview` so the 3D viewport and MCP screenshots show the grid world, textured and with baked corner light. 2D Top view draws cells, walls and nodes.

**P3 Tools.** Extract a `ViewportTool2d` trait from the `toolbars.rs` ladder, then implement the tools in the order of section 2, each with ViewportHarness or MouseRig tests.

**P4 Play.** Play cooks the grid document into the render pack (`brush_grid.rs`, reading the document instead of the raw file) plus derived collision, and builds the guest with the grid renderer selected by world kind, not by env var. Needs the race branch `exp/grid-engine-2026-10-16` merged into `grid/main` first.

**P5 Polish for Manny's first test.** Key hint overlay, budget readout, sensible defaults for a new grid project (a starter room with the Reach character, a light, a spawn), one-click import of a bonnie-32 level, and a short README for the grid mode.

**Later.** psxed-mcp grid tools so agents can build grid levels; native grid collision; portals and visibility; streaming; doors and liquids; vertex colours, transparency, double-sided; stacked rooms.

## 5. Salvage and do-not-repeat

Read old code with `git show <sha>:<path>`; shas and paths are in the archaeology doc. Salvage candidates: `floor_sample.rs` and the grid backend of `character_motor.rs` (collision), `portal_visibility.rs` (visibility), `room_streaming.rs` and `room_graph_ring` (streaming), the old paint tools and their 84 UI tests at `fc3129dc` / `cf154ab4` (reference for tool behaviour and tests, not to copy wholesale).

Do not repeat: per-surface interpretation at draw time with no shared vertices; giant inlined draw loops; caps that disagree between cook and runtime; room-local entity coordinates; stacked floors as scattered checks; 45k-line editor files; two coordinate conventions without a single flip point in the cook; judging performance on a stale cook.

## 6. Risks

- BSP is more than a renderer in the playtest: collision, PVS activation, movers, liquids and sky all come through about 20 `BspRuntime` methods. The prototype keeps PXBSP for collision and activation; replacing those is later work.
- Preview code lives in the emulator frontend crate, outside psxed-ui; the grid walker must land there or the viewport lies.
- The Reach character costs about 460k cycles per frame, so open levels at 30 fps will need character work as well as a lean world.
