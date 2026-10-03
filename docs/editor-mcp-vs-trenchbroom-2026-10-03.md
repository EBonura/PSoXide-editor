# psxed-mcp vs the TrenchBroom MCP: parity audit (2026-10-03)

Question: can the PSoXide editor's MCP server author the PS1 soulslike as well as the
TrenchBroom connector did for the Silent Reliquary levels? Short answer: it already beats
TrenchBroom on validation and preview, and it falls short on geometry authoring. The gap that
matters most is arbitrary convex brushes. Three correctness bugs also turned up during the audit.

## How this was measured

- Both servers' live tool lists, taken over stdio with `tools/list` on 2026-10-03:
  - trenchbroom-mcp 0.5.0 (`~/.local/share/trenchbroom-mcp`): 19 tools.
  - psxed-mcp at `main` (`editor/crates/psxed-mcp`): 32 tools.
- How each one was used for real work:
  - TrenchBroom: the soulslike foundry-ring v1/v2 build
    (`~/Downloads/ps1 games/FFIX-arena-study/arena117-trenchbroom/foundry-ring-v2/`). That build
    also leaned on outside scripts: `layout.py`, `pvs.py`, `budget.py`, `check.py` and `render.py`.
  - psxed-mcp: the Palermo Mondello level built on 2026-10-03 (`~/Desktop/repos/palermo-psx`).
    That build needed hand-edited RON and a small batch client.
- `audit depth=full` was run on two projects: the shipped `default` project and the Mondello project.
- This updates the Phase 8 import in `editor-mcp-plan-2026-09-18.md`. Since then TrenchBroom has
  gained `create_map` and `add_convex_brushes`, and the convex-brush tool did most of the work in
  foundry-ring v2.

## Capability table

| Area | TrenchBroom MCP | psxed-mcp | Verdict |
| --- | --- | --- | --- |
| Create a level | `create_map` (any path, per call) | `--new` flag at server start; one project fixed per server process | TB ahead |
| Primitive solids | `add_brush_box` | `add_shape`: box, ramp, cylinder, arch, curved wall, stairs; `make_room`; `array` (linear and radial) | psxed ahead |
| **Arbitrary convex solids** | `add_convex_brushes`: vertices plus planar polygon faces, per-face texture and Valve 220 axes, up to 512 brushes per call, whole batch validated before writing | none | **TB ahead, the main gap** |
| Boolean edits | none | `carve` (box void through a brush range) | psxed ahead |
| Move / transform geometry | `translate_entity` (brushes keep their texture lock) | `move_node` for nodes only. Brushes can be copied (`array`) but not moved, rotated or mirrored | TB ahead |
| Delete | `delete_brush`, `delete_entity` | `delete` (range), `delete_node` | parity |
| Brush entities (doors, movers) | any brush entity through `entity_index` | brushes carry a `mover` link, but no tool sets it | TB ahead |
| Brush contents | via entity class / texture | `BrushContents` Solid/Water/Slime/Lava in the format, not reachable from the MCP | TB ahead |
| Detail (non-structural) brushes | `func_detail` | none in the format; every brush is structural to the PXBSP cook | TB ahead (cooker work, not just MCP) |
| Materials / UVs | texture name and axes at creation | `set_material` (with a normal filter), `set_face_uv` (offset, rotation, scale), `materials` with real tile sizes | parity for PS1 needs |
| Create materials / textures / sky | n/a (TrenchBroom reads WADs) | none. Mondello needed psxt-convert plus hand-appended `Material` RON, and a separate sky-cook crate | gap |
| World settings (sky, far vista, culling) | worldspawn via `set_entity_properties` | `set_node` on the World node (checked: `get_node World` returns the full `SkySettings` / `FarVistaSettings` RON) | parity |
| Entities | `add_entity` (any class), `set_entity_properties`, `fgd_classes`, `fgd_class` | `entity_types`, `get_node`, `place_node` (clone), `set_node`, `move_node`, `add_light`, `lights` | psxed ahead on introspection; it can only clone kinds that already exist |
| Inspect | `map_overview`, `list_entities`, `get_entity` (every brush with bounds) | `status`, `scene_info`, `metrics`, `get_brush` (one at a time), `groups` | mostly parity; no list of brushes in a region |
| Named spaces | TrenchBroom layers/groups (read only) | `group_brushes`, `groups` (survive index shifts) | psxed ahead |
| 2D views | `render_preview` (SVG top/front/side) | `plan_view` (PNG plan or section at a slice, grid, scale bar, player disc) | psxed ahead |
| 3D view | none (cutaways came from an outside `render.py`) | `screenshot` in the engine's own renderer, auto-framed | psxed ahead |
| Leak check | `compile_map` reports the leak coordinate | `audit` sealing: flood path plus likely opening (but see bug 2) | parity |
| Visibility / budget | ericw `vis` plus outside `pvs.py` / `budget.py`. ericw ignores `func_detail` and is not the target engine's compiler | `audit full`: PXBSP PVS from the real cooker, heaviest leaves in packet slots with authored anchors | **psxed ahead** |
| Overlaps / grid hygiene | none | `audit`: coplanar face fights, off-grid coordinates, degenerate brushes, brushes the cook discarded | psxed ahead |
| Traversal | outside `check.py` route sampling | `walk_test`: the engine's own collision trace | psxed ahead |
| Run it | `launch_map` (a Quake engine, not the target) | `playtest`: builds a real disc, boots it in the emulator, returns the final frame | psxed ahead |
| Undo | `undo_last_edit`, `list_backups` | same, plus in-memory staging, `revert`, and a guard against saving over the editor | psxed ahead |
| Live editing | patched TrenchBroom applies external edits in place | offline only; the GUI bridge is planned ("offline first, live second") | TB ahead |
| Registration | user scope, map path per call | repo scope (`PSoXide-editor/.mcp.json`), one project per process | TB ahead |

## Bugs found while auditing

These are correctness problems, not missing features. All three were fixed on 2026-10-03 (branch
`fix/mcp-audit-bugs`). Bugs 1 and 2 are code changes. For bug 1, `psxed-project` now also denies
`clippy::print_stdout`, so a new `println!` in the cook path fails lint instead of breaking a
client. Bug 2 has a regression test. Bug 3 was local data: the untracked `mcp-demo`, `palermo-mondello`
and `palermo-mondello2` projects had their resource tables re-synced with `default`, and all of
them cook again. Verified end to end: a full audit of the Mondello project sends zero non-JSON
lines down stdout (the seven resample lines now go to stderr), and its leak report reads
`path starts at authored [0, 144, -7600] (engine [0, 9, -475])`.

1. **The cook writes to stdout, which is the MCP protocol channel.**
   `psxed-project/src/playtest/cook_entities.rs:2013` uses `println!` for
   `[cook] resampled <clip>: ...`. Any cook that resamples an animation therefore sends non-JSON
   lines down the JSON-RPC pipe. A strict client dies on them; the batch client did, with a
   `JSONDecodeError` on `audit depth=full` against the Mondello project. Every other cook
   diagnostic already goes through `emit_cook_output`, which uses stderr. Fix: route this line
   through it too. The shipped `default` project doesn't resample, so it hides the bug.
2. **The leak report scales coordinates twice and mislabels the units.**
   `diagnose_brush_world_leak` already returns authored units (`brush_world.rs`, `scale_point`).
   The audit multiplies by 16 again and calls the first figure "engine". Real output for a player
   left at authored Z -7600: `path starts at [0, 144, -7600] engine = [0, 2304, -121600] authored`.
   The "likely opening" and the "multiply by 16" note are wrong the same way. That sends the
   reader 16 times too far when hunting the leak.
3. **The `mcp-demo` template no longer cooks.** Its `project.ron` uses the shared
   `../default/assets` through a symlink, but its Aletha animation set has drifted. Weapon tracks
   point past their clips' last frame (for example frames 9..54 on a 34-frame clip). So any
   project copied from `mcp-demo` gets no draw-cost section from `audit`. All it says is "the
   project did not cook". Copies also write new textures through that symlink into the tracked
   default project unless the author notices.

## Gaps ranked by how much they block soulslike authoring

1. **`add_convex_brushes`.** The brush format already stores faces as three integer plane points,
   so arbitrary convex solids need no format change, only a tool. It should take vertices and
   faces (or planes), set material and UV per face, accept a whole batch, and validate it all
   before staging any of it. This unlocks angled walls, wedge plugs at doorways, sculpted rock and
   heightfield floors. It also lets an outside layout generator (like foundry-ring's `layout.py`)
   feed the editor directly, with integer vertices as the contract.
2. **Brush transforms.** Translate, rotate by 90 degrees, and mirror a brush range or a named
   group, keeping UVs locked. Without these, iterating on a layout means delete and rebuild.
3. **Material, texture and sky import.** Turn a PNG into a 4bpp/8bpp texture and a registered
   material. Cook an equirect sky. Build far-vista cards. The Mondello pipeline did all of this
   with outside scripts and RON edits; it belongs in the server, in Rust.
4. **Brush-model assignment and contents.** Link brushes to a Door or Destructible node, and set
   brush contents to Water/Lava. Soulslike shortcuts are doors.
5. **Detail brushes in the cooker.** Sculpted rock as structural brushes blew the TrenchBroom BSP
   up to 6,000+ empty leaves. The same rock as `func_detail` then stopped occluding in ericw `vis`.
   The PXBSP cooker has no detail class at all. This is engine work. Measure it on a rock test cell
   before committing to organic geometry.
6. **Project per call and user-scope registration.** Add `create_project` (from `default`, never
   from a stale template) and let tools name the project, so the server works from any session.
7. **Smaller conveniences:** listing brushes in a region or group with their bounds; creating a node
   from kind RON when no example exists to clone; a `screenshot` option that keeps the requested
   camera instead of auto-framing in front of the hero.
8. **Live GUI bridge.** Already planned; it matters once Manny edits alongside the agent.

## Proposed order of work

1. The three bugs: a one-line stderr fix, the leak-unit fix with a regression test, and either
   repairing or retiring `mcp-demo`. Small, and they make every later measurement trustworthy.
2. `add_convex_brushes` and brush transforms, together. Prove them by rebuilding one
   foundry-ring cell from its integer vertex data and comparing `audit` numbers with ericw.
3. Material/texture/sky import tools, ported from the Mondello scripts.
4. Mover and contents assignment.
5. A measured detail-brush experiment in the cooker.
6. Project-per-call, `create_project`, user-scope registration.
7. The live bridge.
