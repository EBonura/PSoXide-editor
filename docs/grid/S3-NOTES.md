# S3 notes (2026-10-18): tools follow the cursor, spiral stairs, the tower

Branch `grid/s3-2026-10-18` on `grid/main` 07354cc5. Design: `multi-storey.md` (see "As built: S3"). Guides: `editor-guide.md` (Storeys), `mcp-guide.md` (Storeys, Spiral stairs, The octagonal tower).

## What was built

1. **Focus** (`psxed-project/src/grid/edit/focus.rs`), replacing the per-call `with_span_at` swap as the one mechanism: capture once, `run` an edit, `view` for a gesture's snapshot. Refuses `OutOfOrder`, `SpansChanged`, `NoSpan`, `SeveralSpans`; fills slivers with `normalize_column` and returns the count and the moved floor. `with_span_at` is the capture-and-run shortcut the MCP used.
2. **Span in faces and refs.** `GridFace.span`, `FaceTarget::Surface { kind, span }`, `FaceRef::floor_in/ceiling_in/surface/span_of`, `faces_of_leaf` lists every span, `range_surfaces` takes a span, `delete_faces` cuts a slab out through either of its surfaces (descending boundary order, one cut per slab), the UV and clipboard code follow the span.
3. **Editor tools aimed** (`psxed-ui/src/grid_tools/aim.rs`): `capture_focus`, `with_focused_world` (a gesture's `begin` reads the focused view), `follow_edit`, `cursor_follows_pick`. Floor, Ceiling, Raise, Ramp, Stairs, Break run through `Focus`; `VerbGesture` keeps its focus. A press on a region that the verb refuses no longer starts a new region over the refusal message.
4. **Wall gap by span** (`next_wall_heights`): the span open at `pick_y`, and only the walls standing in it count. The Top view passes the cursor for columns of several spans.
5. **Top view slice** (`grid_view.rs`: `Slice`, `GridRoomView.level`).
6. **Preview view.** `GridPreviewView { hidden_rooms, cutaway }` replaces two thread-locals with one typed value, read once in `build_phase1_frame_reusing` and passed to `walk_grid` as an argument.
7. **`grid_raise unit: click`** now refuses a leaf of several spans and a span aim.
8. **Spiral stairs**: `edit/verbs/spiral.rs`, tool `Spiral` (Shift+T), MCP `grid_spiral_stairs`; `grid_stairs` takes `y`/`storey`.
9. **Void-carrying slabs and per-span diagonal faces** (found building the tower: the octagon's chamfer cells are half cells).
10. **The tower** and its script, test and play route.

## Decisions made

* **Typed preview argument, with a limit.** The frontend's render entry points (`main.rs`, `gfx.rs`, `cli.rs`) are the pinned emulator's, so a new argument or a new `ViewportCameraState` field would break the hydrated files. The two side channels became one typed snapshot read once at the editor-owned edge and passed down as an argument. A true argument through `render_editor_preview` needs the emulator pin to move.
* **`FaceTarget::Surface { kind, span }`** instead of separate `Floor { span }` and `Ceiling { span }` variants: every site that only cares about floor versus ceiling keeps one arm.
* **Single-span columns are never aimed.** A column of one span is its ground span whatever the height (this makes `y` lenient for the MCP on single-span sectors; before it refused a height outside the span). Only columns of several spans need, and check, the aim.
* **The cursor follows only when set.** A level with one storey and no cursor behaves exactly as before. Following sets the Floor tool's height too.
* **A drag that would break a column holds the last good amount** and says why; the document is never left half edited.
* **Spiral is a slab per visit, not a ground-floor edit.** It needs no focus (it inserts spans), so its gesture runs unfocused; the aim only picks the starting floor.
* **Spiral is its own tool** (Shift+T), not a Stairs mode: discoverable in the strip and keeps the Stairs code unchanged.
* **Tower outline drawn a quarter sector out** so `grid_polygon` (centre-in-polygon) takes the 12 diagonal cells and the chamfer pass makes them half cells.
* **The route is open loop**, tuned from tracked paths (about 44 units a poll straight, 32 on each axis diagonally).

## Not done / open

* **Exterior facing of derived walls** (`RoomDefaults.closure_facing`, decided 10-17): derived walls are one-sided, so the tower is hollow from outside (the outside screenshots see the inside of the far walls). Not part of this lane's list.
* **Levels panel, `+ Level`, named storeys, light gate in the preview, span culling** (MS3/MS8 leftovers).
* **Top view derived wall lines** are drawn for every storey (the slice dims fills, not walls).
* **Cells tool, Rough, Bands on multi-span columns** are not aimed (Rough is render-only per sector; Cells splits keep all spans untested; Bands address wall runs).
* **Derived-wall addressing across spans.** `FaceTarget::Derived` has no span; several faces of one kind on one edge (slab edges, per-span diagonal closures) resolve to the first, so an override or repaint reaches only that one.
* **Railings** around the well and flights: the walkability overlay will flag drop edges.
* **Camera in the ring**: Play frames show the third person arm working in the one sector ring; no widening was needed.
* **Spans in rooms with fine cells** still refuse at the cook.

## Smells noticed

* `derive_diag::diagonal_walls` and `leaf_diag` duplicate each other; the span fix landed in the dense one only.
* `FaceRef::derived` and the Top view pick code build `FaceRef::floor(room, cell)` just to reach a sector (`face_tools`, `pick`): a `sector_at(room, cell)` helper exists elsewhere.
* `verb_tool::begin_amount` returned `bool` for three different outcomes (started, refused with a message, not applicable); it is now an enum, but each tool's `begin` still returns `bool`.
* Hover adoption in `rect_tool` (`height_under`) sets the Floor height from any floor under the pointer; with storeys that silently changes what the next click makes. It stays because the cursor now overwrites it on every edit.
* `psxed-mcp` instructions text in `main.rs` lists tools by hand and lags the registry.

## Evidence

* Host tests: `psxed-project` lib, `psxed-ui` lib (incl. `tests/grid_aim.rs`, 14 interaction tests in both views), `psxed-mcp` (incl. `grid_spiral`, `grid_tower`).
* Cook hashes of the eight reference fixtures equal `tools/grid-simgate/cookhash-golden.txt`.
* Tower: `psxed-mcp/tests/grid_tower.rs` (build and read back, and with `PSXED_MCP_VERIFY=1` the climb). Screenshots and play frames: `/Users/ebonura/Desktop/repos/campaign-2026-10-15/editor-s3/`.
