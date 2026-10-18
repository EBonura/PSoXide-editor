# UX lane notes (2026-10-18): one home per function, the plan first-class, exterior facing

Branch `grid/ux-2026-10-18` on `grid/main` d35a24ee. Wrapped up early (coordinator, Manny 10-18): the items under "Next steps" are not done.

## What was built

### Editor UI

* **Levels panel** (`psxed-ui/src/grid_tools/levels_panel.rs`), top of the left dock: the derived storeys, highest first, "All levels" row, click goes to a level (the storey cursor), double-click renames. Names live in `GridWorld::level_names`, keyed by the floor height in clicks (editor-only, the cook ignores them, an unnamed storey is `Level N`). `+` is the Slab tool in Insert mode set to one storey; the scan icon is the cutaway.
* **Rooms panel**: the room settings (Raise, Ambient, Shading, walls, paint rules, Rough) moved into one collapsed "Settings of <room>" section so the scene graph has room. Clicking the pinned room again unpins it (one active-room rule; the Top view chips that did the same job are gone). New **Outside** option: inside only / inside and outside (see exterior facing).
* **Left dock** of a grid world is Levels, Rooms, Scene Graph. The FileSystem panel is not drawn there (the Resources dock lists the same things).
* **Materials panel** (`grid_tools/materials_panel.rs`), the one home of materials: search box, `+` (new material, new transition material), import (image becomes a material), pencil (the paint material in the Material tab), "All project materials" switch. Materials made or imported from it are pinned in `GridWorld::palette_pins` and always show. The palette marks the paint material (accent ring) and the selected face's material (white F). The Resources dock lists no materials in a grid project and its `+` menu has no Material, Transition or Import Texture entry there.
* **Inspector**: a face, plane, mesh or level verb selection owns it, so the World settings (Physics, Sky, Far Vista) no longer follow a face selection; the "Select faces" hint no longer sits above a selected light. A derived wall of a floor above the first says which floor.
* **View switch** in the tab row: 3D, Plan, Split (View menu too, Ctrl/Cmd+9 cycles). Split draws the plan and the 3D view side by side, both live. Tabs are World and HUD (were 3D and 2D). Overlays (Walk, Sizes, was Cells, Perf) and Keys moved to the tab row; the tool row is tools, then a second row of the tool's options and the snap.
* **Undo and Redo** buttons next to the menus and in the Edit menu; Help has Keys; View labels fixed; the top status strip shows the badge only in a grid world (the bottom bar has the message); the Plan info box lost the node and resource counts.
* **Top view derived walls** are drawn and picked only for the span shown at the cursor (`grid_view::wall_in_slice`); the Bands tool in the Top view follows the same rule. In 3D a picked face already carries its span.
* **Cells** refuses to divide a column of several storeys (`SubdivideBlock::Storeys`) and says why.
* Floor and Ceiling: hovering no longer changes the tool height; pressing on a surface of the same kind takes its level (explicit, on the press).

### Core and MCP

* **Exterior facing**: `RoomDefaults::closure_facing` (`ClosureFacing::{Front, Both}`, default Front, omitted from the file). `Both` makes closure walls double sided in the faces, so the preview and the cook (QUAD_DOUBLE) draw them from outside. Collision unchanged. MCP `grid_set_room closure_facing: front|both`, the Rooms panel "Outside" combo, and `grid_tower.script` sets it. Default Front changes no existing level (see the cookhash note below).
* **Derived faces carry a span**: `DerivedWall::span` (the host column's span that sees it), `FaceTarget::Derived { span }`, `FaceRef::of_derived`. Resolution by edge and kind needs the span only when several faces of one rule stack on the edge. Edge overrides are still keyed by segment (`EdgeKey` has no span), so an override applies to every face stacked on that segment; making it per face needs a key change (open).
* **One diagonal path**: `derive_diag::segment_walls(DiagSegment)` serves both the whole sector and the leaf; `leaf_diag` is now a wrapper and derives every span (it did the ground span only).
* **Spans in fine-cell rooms: still not cooked, by design.** The runtime span tables are per whole sector; leaves below or above a sector have no place in them. The editor now keeps that state out: Cells will not divide a column of storeys, `validate` reports `SpansInCellRoom` as an error, and the cook message says why. Lifting it is a format change (leaf-aware span records) that belongs with the T10 leaf format work.
* **Railings**: no new tool. A railing is an authored solid wall (the Wall tool in a derived room, `grid_place_wall`) along the drop edge, one or two clicks tall: a solid record blocks whatever its height. The walkability overlay already flags the drop edges. A verb that rails a region is open.
* **S3 smells**: every verb tool's `begin` returns `verb_tool::Begin` (Started, Refused, NotApplicable) instead of a bool; hover height adoption is explicit (above); the MCP instructions text is composed from the registry (`psxed-mcp/src/tools/instructions.rs`: prose by hand, the tool list generated, a test that every `grid_*` name in the prose exists).
* Rough is not aimed on purpose: the jitter is a function of position, so every storey of a column moves together.

## Tests

Host tests added: `psxed-project/src/grid/exterior_tests.rs` (closure facing, span addressing), `psxed-ui/src/tests/grid_ux.rs` (Levels panel and names, wall slice, view modes, every tool key, Materials panel, undo buttons, room pin), `psxed-mcp` instruction tests. Four older UI tests were updated for the new layout (settings section closed by default, hover no longer adopts a height, a taller test screen).

## Next steps (exact)

1. Run `cargo test --release --locked -p psxed-project --lib`, `-p psxed-ui --lib`, `-p psxed-mcp --bin psxed-mcp` and the psxed-mcp integration tests (`grid_tower`, `grid_cave`, `grid_level`, `grid_tools`, `grid_storeys`); fix what the layout change broke. CARGO_BUILD_JOBS=3, release (the target holds release builds).
2. Regenerate the tower with `closure_facing both`: `PSXED_TOWER_OUT=<new dir> cargo test --release -p psxed-mcp --test grid_tower`, then replace `editor/projects/grid-tower/project.ron` (keep its `assets` link and `baked`). Screenshot it from outside (`grid_plan`, `screenshot` poses) before and after for the report.
3. Screenshots of every UX change into `/Users/ebonura/Desktop/repos/campaign-2026-10-15/editor-ux/` (a before is there: `before-3d.png`). `cargo build --release -p frontend --features editor`, then `target/release/frontend dump-editor-ui --project editor/projects/grid-cathedral --view 3d --out <png>` (also `--view top`; `docs/grid/frontend-grid-pose.patch` adds `--grid-pose` but touches hydrated emulator files, so do not apply it in the tree). Split has no dump view flag: use `grid.split` by a pose or add a small `--view` handling outside the hydrated files if possible, else document.
4. CI replay (`.github/workflows/ci.yml`): `cargo fmt --all -- --check` (run `cargo fmt --all` first: several files were edited by script), `cargo check --locked --workspace --all-features`, `make lint`, `make material-audit`. State any deviation (release instead of debug tests).
5. `tools/grid-simgate/cookhash.sh ux grid-cathedral grid-level008 grid-dungeon grid-cave grid-sewers grid-west grid-starter grid-gate` and diff against `cookhash-golden.txt`. Expected equal: the default facing is Front and nothing else touches the cooked tables. Then `simgate.sh` on the reference levels if time allows.
6. Open from the brief: railing verb, per-face derived overrides (EdgeKey with a span), `docs/grid/editor-guide.md` and `mcp-guide.md` updates for everything above (Levels panel, Materials panel, view switch, closure_facing, undo buttons), the Place picker was already in the strip (the audit was out of date there), the audit items for Front/Side views and the bottom Resources drawer default are untouched.
