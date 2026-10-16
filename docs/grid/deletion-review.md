# Deletion review for `grid/main` (2026-10-16)

Scope: Manny's direction (b), "review all the functionality and delete everything which is not related to the grid world". This is a review and an ordering, not a deletion. Nothing was removed. The companion design is `docs/grid/level-format-and-runtime.md` (called "the runtime doc" below); every REPLACE here points at a section of it.

Method. I listed every crate, module, example, tool, Makefile target, CI step, doc and project in the tree at `HEAD` `761b4069` plus the uncommitted edits another session was making while I read (`git status` showed changes in `brush_grid/collision.rs`, `brush_world.rs`, `grid/mod.rs`, `grid/bonnie_project.rs`, `playtest/grid_world.rs` and five `psxed-ui` files), counted lines with `wc -l`, and traced dependencies with `grep` on `use`, `crate::` and `psx_bsp::` paths. Line counts are "approximately as of 21:20 on 16 Oct" and will drift. "Mentions" columns are keyword counts (`brush|bsp|pxbsp|csg`, case-insensitive) and are an **[estimate]** of entanglement, not a measure of removable lines.

Legend. **KEEP** is needed by a grid-world game. **DELETE** is brush/BSP authoring, cook, runtime or oracle code with no grid use. **REPLACE** is a BSP-provided service the grid needs under another implementation (pointer to the runtime doc). **MIXED** is a file that holds both and must be cut, not removed. **KEEP-FOR-CONSUMERS** is code that grid does not need but that quake-psx, hl-psx or the emulator frontend still build against (section 2).

---

## 0. Summary

1. About 40k of psxed-project's 98k source lines are brush, BSP, streamed-brush-world or bonnie-import code that the grid world does not use, with more inside mixed files (section 3.1, **[derived]** from `wc`). About 17.5k lines of psxed-ui are wholly brush authoring plus an estimated 5k to 8k inside mixed files. About 3k of psxed-mcp's 6.8k are brush tools.
2. **You cannot delete in the order the code is written.** Four knots tie the grid to the brush world and must be cut first (section 4): `FaceUv` lives in `brush.rs`; the light bake lives in `brush_light.rs`; the New Project template and the shipped `default` project are brush worlds; and the sky module and the grid renderer's helpers live inside the `psx-bsp` crate.
3. **Three constraints are not about this tree.** (a) Files under `sdk/`, `crates/`, `emu/crates/{emulator-core,psoxide-*,psx-gpu-render}` and most of `emu/crates/frontend` are materialized from two other repos and cannot be edited here; four of those files call editor symbols this review wants to delete. (b) quake-psx and hl-psx build against `engine/` and `editor/crates` of this repo at pinned revisions. (c) Play still gets collision, activation, sky, movers and liquids from PXBSP, so the BSP runtime cannot go before `GridRuntime` exists.
4. Recommended order: prepare (cut the four knots, add the grid starter and stubs), delete the zero-coupling material, remove brush authoring from the editor and MCP, build `GridRuntime` (runtime doc phases 0 to 4), then remove the BSP cook and runtime, then the brush data model, then the engine crate remains. Section 6.
5. Do **not** merge `grid/main` into `main`. Quake and Half-Life stay on BSP through `main` (section 5).

---

## 1. What is tracked here and what is materialized

`components.lock.json` pins two upstream repos; `make bootstrap` copies them in and `.gitignore` hides the copies (`/sdk`, `/crates`, `/tools/mkisopsx`, ..., `/emu/crates/emulator-core` and the rest, `.gitignore` lines for "Locked component sources"). Anything edited under those paths is silently overwritten by the next bootstrap, so changes there must go upstream and be picked up by a revision bump in `components.lock.json`.

| Path | Origin | Editable here | Notes |
|---|---|---|---|
| `sdk/` (including `sdk/crates/psx-asset`, `psx-rt`, `psx-gpu`, `psx-gte`, ...) | SDK repo `EBonura/PSoXide` rev `36ead4b0` | no | `psx-asset` holds the `.psxw` `World` parser (`sdk/crates/psx-asset/src/lib.rs`, around line 2447 per the archaeology doc) and its tests |
| `crates/` (`psx-iso`, `psx-hw`, `psx-trace`, `psx-disasm`, `psx-anim-*`, `psx-audio-cook`, `psxed-format`) | SDK repo | no | `crates/psxed-format/src/world.rs` (508 lines) is the `.psxw` v5 spec and is still compiled; `psx-iso` owns `WORLD.PAK` and must stay |
| `tools/mkisopsx`, `tools/psoxide-link`, `tools/psoxide-pgo`, `tools/psoxide-hazard`, `tools/guest_symbol_gate.sh`, `tools/sdk-examples.mk` | SDK repo | no | `mkisopsx` still understands `room_NNN.psxw/.psxc` chunk names (archaeology doc, `tools/mkisopsx/src/main.rs:799,823,892`) |
| `emu/crates/{emulator-core,psoxide-debug-ui,psoxide-jit,psoxide-settings,psoxide-validation,psx-gpu-render}` | emulator repo `EBonura/PSoXide-emulator` rev `6edde550` | no | |
| `emu/crates/frontend/` **except** the 15 tracked files below | emulator repo | no | `app.rs`, `cli.rs`, `gfx.rs`, `main.rs`, `mcp.rs`, `Cargo.toml` and the rest |
| `emu/crates/frontend/src/{editor_assets,editor_preview,editor_textures,embedded_playtest,playtest_disc}.rs`, `editor_preview/*.rs`, `assets/fonts/lucide.ttf` | **this repo** (tracked, `.gitignore` allow-list) | yes | CI also runs `rustfmt --check` on exactly five of them (`ci.yml` line 17) |
| `editor/`, `engine/`, `docs/`, `assets/`, `benchmarks/`, `validation/suite.ron`, `Makefile`, `Cargo.toml`, `.github/`, the rest of `tools/` (`psoxide-dev`, `psoxide-hwtest`, `psoxide-perf`, `psoxide-vmcook`, scripts) | **this repo** | yes | 2296 tracked files under `editor/`, 329 under `engine/`, 227 under `docs/` |

Note on the user's phrase "`.psoxide/`": that directory is how quake-psx and hl-psx materialize **this** repo as their `editor` component (`quake-psx/components.lock.json`, `hl-psx/components.lock.json`). It does not exist inside this tree.

### 1.1 Materialized files that call editor code this review would delete

These are in the emulator repo's frontend and are not editable here. Each needs either a stub that keeps the signature alive in a tracked file, or a change in the emulator repo plus a `components.lock.json` bump.

| Caller (untracked) | Symbol it needs | Defined in (tracked) | Fate |
|---|---|---|---|
| `emu/crates/frontend/src/cli.rs:3743` | `psxed_project::brush_world::diagnose_brush_world_leak(project.clone())` | `editor/crates/psxed-project/src/brush_world.rs` | keep a stub returning `Err`/empty until the emulator repo drops the call |
| `cli.rs:3731,3752` and `gfx.rs:299-343` | `editor_preview::prepend_bsp_surface_grid_overlay`, `append_bsp_leak_path_overlay` | `editor_preview.rs:296,316` (tracked) | keep as no-op functions with the same signature |
| `main.rs:1904-1926` | `state.editor.show_brush_surface_grid_enabled()`, `visible_bsp_leak_path()`, `visible_bsp_leak_opening()` | `psxed-ui` `EditorWorkspace` | keep as methods returning `false` and `&[]` |
| `app.rs:566-572` | strings only ("falling back to BSP starter"), plus `psxed_project::new_project_template_dir` (:2 uses) | `psxed-project/src/lib.rs:458` | keep the function; point it at the grid starter |
| `emu/crates/frontend/Cargo.toml:17,39` | `editor = [..., "dep:psx-bsp", ...]` and `psx-bsp = { workspace = true, optional = true }` | root `Cargo.toml` `[workspace.dependencies]` | the `psx-bsp` workspace dependency **must keep resolving** until the frontend manifest changes upstream. Resolution fails even for an optional dependency if the workspace table lacks it. |
| `editor_preview.rs:71-74`, `editor_textures.rs:426-427`, `editor_preview/backdrop.rs:62-87` (tracked) | `psx_bsp::sky::*` | `engine/crates/psx-bsp/src/sky.rs` | tracked, so fixable here once `sky` is extracted (section 4, knot 4) |

Cheapest long-term fix, an idea for Manny rather than a plan: the frontend's editor feature should not know about `psx-bsp` at all. One small change in the emulator repo (drop the `psx-bsp` optional dependency, route sky through `psx-engine` or a new sky crate) removes the whole row. Until then the crate name stays resolvable.

---

## 2. Who else consumes this repo

Both `quake-psx` and `hl-psx` materialize this repo as their `editor` component, **pinned by revision** in their `components.lock.json`, with these paths: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `engine`, `editor/crates`, `editor/assets`, `editor/projects/default`, `emu/crates/frontend`, `tools/psoxide-dev`, `tools/psoxide-hwtest`, `tools/psoxide-perf`, `tools/psoxide-vmcook`, `assets`.

| Consumer | Pinned editor rev | Crates it depends on (Cargo path deps into `.psoxide/engine/crates`) |
|---|---|---|
| quake-psx | `fb2a9ab4` (lock), receipt shows an older `fecde607` | `psx-bsp`, `psx-engine`, `psx-render-contract` (`quake-psx/game/Cargo.toml:187,202`, `crates/quake-core/Cargo.toml:26-27`, `crates/quake-formats/Cargo.toml:9`, `crates/quake-affine/Cargo.toml:13`) |
| hl-psx | `5b77b8fb` (lock), receipt shows an older `847d3f00` | `psx-engine`, `psx-goldsrc`, `psx-render-contract` (`hl-psx/game/Cargo.toml:122`, `host/hl-logic-tests/Cargo.toml:19`) |

Both pinned shas are ancestors of both `main` and `grid/main` today (`git merge-base --is-ancestor`), and they stay valid as long as history is never rewritten. So nothing done on `grid/main` can break either consumer **until one of them bumps its pin to a `grid/main` commit** or `grid/main` merges into `main`.

Transitive requirements inside `engine/` (from the `Cargo.toml` of each crate):

* `psx-bsp` depends on `psx-engine`, `psx-gpu`, `psx-gte`, `psx-math`, `psx-pack`. `psx-game-runtime` depends on `psx-bsp` (one file: `region_stream.rs`). `psx-engine` depends on `psx-level`. So `psx-level` and `psx-engine` are **required by hl-psx** and cannot lose anything it uses; `psx-bsp` is required by quake-psx.
* `psx-goldsrc` (12.8k lines, oracles under `tests/oracles/`) is Half-Life/Counter-Strike code; nothing in the grid world needs it, but hl-psx does.
* `psx-render-contract` is shared by both.

Consequences for the plan:

1. **Rule R1.** Never merge `grid/main` into `main` as a whole. If a shared fix (for example the motor step-height config field) is wanted on both lines, cherry-pick it.
2. **Rule R2.** If quake-psx or hl-psx needs a newer editor revision for an unrelated reason, bump it to a `main` commit, never a `grid/main` one.
3. **Rule R3.** `psx-bsp`, `psx-goldsrc`, `psx-render-contract` and the public API of `psx-engine` and `psx-level` stay on `grid/main` untouched until Manny decides to cut the consumers loose. Deleting them on `grid/main` is safe for the consumers today (pins), but it makes `grid/main` un-mergeable forever and strands the pin on a branch that is not the product line. I list them as KEEP-FOR-CONSUMERS rather than DELETE for that reason, with the deletion path written down (section 6, stage 9).
4. **Better, longer-lasting option** (Manny asked to be told about these): move the shared engine crates (`psx-bsp`, `psx-goldsrc`, `psx-engine`, `psx-level`, `psx-render-contract`) out of the editor repo into the SDK repo or an `engine` repo that all three games and the editor pin. Then "delete everything not grid" becomes a clean deletion in the editor repo and Quake/HL stop depending on an editor checkout for their engine. It is a larger change than this review and I am not proposing it for now.

---

## 3. Inventory and classification

### 3.1 `editor/crates/psxed-project` (src 98.4k lines; 168 files incl. tests and bins)

DELETE, brush and BSP world (all `src/`; counted at HEAD plus the other session's edits):

| Module | Lines | What it is | Depended on by (outside the DELETE set) |
|---|---|---|---|
| `brush.rs` | 2446 | convex half-space brush kernel, **also defines `FaceUv`** | `grid/surface.rs`, `grid/faces.rs`, `grid/room.rs` (FaceUv), `scene_types.rs` (`Scene.brushes`), `units.rs`, `lib.rs`, `tests.rs`, `playtest.rs`, `terrain.rs`, `psxed-ui` (everywhere), `psxed-mcp`, frontend `editor_preview`, `editor_textures.rs:2324` |
| `brush_compile.rs` | 2140 | CSG to surface BSP, portals, leaves | `document_types.rs` (`SurfaceEdgeRepair` setting), `units.rs`, `brush_grid/mod.rs` (`CompiledSurface`), `editor_preview` |
| `brush_world.rs` | 3910 | full BSP cook entry (`compile_brush_world`) | `playtest.rs` (17 refs), `document_types.rs`, `terrain.rs`, frontend `cli.rs:3743` |
| `brush_world_stream.rs` + `brush_world_stream_tests.rs` | 2137 + 876 | streamed PXBSP cook | `stream_world.rs`, bins |
| `brush_region/` (14 files) | 5445 | region partitioner for streamed brush worlds | `brush_world_stream.rs` |
| `brush_pack.rs`, `brush_pxbsp.rs` | 1373, 1249 | PXBSP record packing and container | `brush_world.rs` |
| `brush_vis.rs` + `brush_vis/cluster.rs` | 938 + 561 | PVS (draft and portal-flow), including the env-gated `OPEN_HALL_SAMPLED_VIS` experiment | `brush_world.rs` |
| `brush_collision_hulls.rs`, `brush_region_hulls.rs` | 1032, 1397 | Minkowski hull clipnodes | `document_types.rs` (`CollisionLeafTarget`), `playtest.rs` |
| `brush_portal.rs`, `brush_seams.rs`, `brush_overlap.rs`, `brush_convex.rs`, `brush_primitives.rs`, `brush_transform.rs`, `brush_walk.rs` | 768, 465, 262, 309, 671, 292, 203 | portals, seam audit, overlap audit, convex builders, ramp/stairs/arch generators, mirror/rotate, hull walker | brush tools in psxed-ui and psxed-mcp |
| `brush_playtest.rs` | 597 | file-name constants `BRUSH_WORLD_FILENAME`, `BRUSH_LEAK_FILENAME` plus fixture tests | `psxed-ui/src/lib.rs:209`, `psxed-ui/src/tests/project_workspace.rs` (many), `playtest/manifest.rs` |
| `solid_cell_constraints.rs`, `solid_cell_proofs.rs` | 330, 280 | offline halfspace proofs for the BSP hull, "not wired into cooks or runtime yet" per its own header | `playtest.rs` (3), `tests.rs` |
| `terrain.rs` | 669 | heightfield to closed brush solids | psxed-ui `workspace/terrain.rs` |
| `quake_map_import.rs` | 507 | Quake `.map` geometry import | none |
| `stream_world.rs`, `stream_walk.rs` | 1952, 307 | stress-world generator and route walkability for streamed brush worlds | bins `gen_stream_world`, `stream_cook`, `stream_report`, `stream_world_sweep` |
| `playtest/world_object_leaf_links.rs` | 174 | cooked leaf links for the BSP object registry | `playtest/cook_props_lights.rs`, `manifest.rs` |
| `brush_grid/raw_level.rs` | 484 | reads a raw bonnie file for bench scripts (`OPEN_HALL_GRID_LEVEL`) | `brush_grid/mod.rs` |

Sum of the rows above: **about 31.8k lines** **[derived]**. Test counts: 241 `#[test]` in these files **[measured by grep]**.

DELETE, binaries (`src/bin/`, 9,750 lines total):

| Binary | Lines | Why it goes |
|---|---|---|
| `gen_brush_directional_sky`, `gen_brush_first_playable`, `gen_brush_layered_sky`, `gen_brush_liquid_runtime`, `gen_brush_open_courtyard` | 533, 253, 296, 244, 291 | generate brush fixtures; also feed `Makefile` acceptance targets and the New Project template (section 4, knot 3) |
| `gen_quake_e1m1_project` | 443 | Quake E1M1 as a brush project |
| `gen_stream_world`, `stream_cook`, `stream_report`, `stream_world_sweep` | 65, 170, 111, 284 | streamed brush world tooling |
| `pxbsp_draw_cost`, `pxbsp_seam_audit` | 145, 133 | PXBSP measurement tools |
| `import_bonnie32_level` | 1117 | bonnie to **brush** project (the superseded importer; its `--perch` HookPoint logic for Graybox Reach is the one thing worth reading before deletion) |
| `gen_grounding_side_view` | 289 | brush slab fixture generator |
| `gen_cortex_ignition_tech_demo_0_2`, `sync_cortex_ignition_tech_demo_0_2` | 740, 919 | generator and migrator for a superseded tech demo; not world-specific but dead weight for a grid game. I mark DELETE with low confidence: ask Manny if he still regenerates 0.2. |

Sum: **6,033 lines**. KEEP binaries: `cook_playtest` (275), `audit_animations`, `audit_clip_tails`, `import_locomotion` (1437), `merge_character_loadouts`, `miniaturise_project`, `prune_clips`, `prune_starter_resources`, `reimport_model_probe`, `transcode_psxanim_v4`. Their `[[bin]]` entries in `Cargo.toml` go with the deleted ones (`Cargo.toml` declares 16 `[[bin]]` entries; `src/bin/` holds 25 files, the rest are auto-discovered).

DELETE later (after the new importer decision, section 6 stage 9):

| Module | Lines | Reason it waits |
|---|---|---|
| `grid/bonnie.rs`, `bonnie_project.rs`, `bonnie_tests.rs`, `import_tests.rs`, bin `import_bonnie32_grid` | 746, 595, 249, 267, 141 (1,998 total) | Manny: "no more bonnie import; the six imported projects are references". The imported `.ron` documents (`editor/projects/grid-*`) are standalone, so the importer is only needed to re-import. Delete once Manny confirms the references are final. Also drops `brotli-decompressor` from `Cargo.toml`. |

REPLACE:

| Module | Lines | Replaced by |
|---|---|---|
| `brush_grid/mod.rs` (GRD1 writer, lattice snap, light bake driver) | 660 | the PGL cook (runtime doc sections 3 to 4); its corner-snap and `uv_overflow` code move |
| `brush_grid/collision.rs` | 373 | native collision tables (runtime doc section 5); deleting it removes the "sky lid" and the thin-slab derivation |
| `brush_grid/document.rs` | 190 | cook input adapter, folds into the PGL cook |
| `playtest/grid_world.rs` | 225 | `playtest/grid_cook.rs`: validate, cook core and chunks, no `prepare()` brush derivation |
| `playtest/budget.rs` | 1326 (142 mentions) | grid budgets (cells, quads, vertices, pool pages); its PXBSP sky-packet constants come from the relocated sky crate |
| `playtest/manifest.rs`, `playtest/schema.rs` | 2846, 2070 (26 and 30 mentions) | MIXED: keep manifest plumbing, replace `PlaytestPxbspWorld`, `PLAYTEST_WORLD_GRID`, the PXBSP world records and `WORLD_PACK`/`stream_chunks` writing with the PGL core and room chunks |
| `playtest.rs` | 2584 | MIXED: `build_package` calls `grid_world::prepare`, then scales the project, then **fails if `scene.brushes.is_empty()`** (`playtest.rs:518-522`, "BSP is the sole world source"), then the whole PXBSP path. Rewrite around `SceneWorld::Grid` only |

KEEP (grid world, characters, animation, materials, audio, UI, assets):

| Module | Lines | Notes |
|---|---|---|
| `grid/` (everything except the bonnie files) | about 6.2k incl. `edit/` 3.5k | the document, faces, light, limits, edit operations; 136 `#[test]` **[measured]** |
| `scene_types.rs` | 1278 | MIXED: drop `Scene.brushes`, `Brush.mover`-related docs, brush groups (`:896`), `bsp` wording in `World` node docs |
| `scene_grid_types.rs` | 2007 | misnamed survivor: materials, UV, `Transform3`, prop sizes. Rename (for example `scene_common_types.rs`) while it is open anyway; remove its BSP test at `:2001` and comments at `:800-804` |
| `document_types.rs` | 2247 | MIXED: remove `bsp_cook_mode`, `collision_hull_bsp`, `bsp_render_edge_repair`, `bsp_collision_leaf_target`, `bsp_solid_cell_proofs` (`:292-310`, defaults `:414-418`); the legacy-grid load guard (`legacy_grid_node_count`, `:1806`) stays and gets re-examined (below) |
| `resource_types.rs` | 3245 | characters, weapons, animation, audio, materials. MIXED only in `MaterialResource.sky_aperture` (BSP sky-aperture flag, used by `brush_grid/collision.rs` and `playtest/grid_world.rs` to find the sky material): replace by a plane/cell `OPEN_SKY` rule |
| `world_types.rs` + `world_types/sky.rs` | 551 + 1955 | sky, far vista, camera, physics, culling. MIXED: `WorldCullingSettings.bsp_patch_extent` (`:455-485`) goes |
| `arch_prop.rs`, `box_prop_erosion.rs`, `cylinder_prop.rs`, `prop_surfaces.rs`, `spatial.rs` | 1022, 511, 639, 223, 303 | props are scene nodes, world independent |
| `animation_*.rs`, `clip_window.rs`, `model_import.rs`, `model_material_texture.rs`, `texture_import.rs`, `sky_texture.rs`, `import_util.rs`, `resolve.rs`, `streaming.rs`, `ui_types.rs` | 626, 378, 255, 424, 1365, 1417, 712, 164, 64, 137, 246, 2152 | assets, characters, UI |
| `units.rs` | 962 | MIXED: node, character and model scaling stays; `scale_brush` (`:144-153`) and the three brush imports go |
| `brush_light.rs` | 476 | MIXED, and a knot: `BrushPointLight`, `BrushMaterialTint` and `lit_point_color` (`:13-30,201`) are what `grid/light.rs` uses; `bake_brush_vertex_lighting*` and `brush_occluder_planes` (`:35-200`) are brush-only. Split first, then rename the survivor (`light_bake.rs`) |
| `playtest/{cook_entities,cook_props_lights,cook_ui,assets,xa_music,performance}.rs` | 4391, 2360, 1794, 159, 345, 63 | entities, lights, UI, XA music: grid independent (a handful of `leaf`/`brush` mentions in `cook_props_lights.rs` to cut) |
| `tests/scene_sky_contract.rs` | small | uses `archive/fixtures/brush-directional-sky`; rebase on a grid fixture |

Net: of 98.4k source lines, DELETE is 31.8k (modules) + 6.0k (bins) + 2.0k (bonnie) = about 39.8k, "about 40k of 98k deletable outright" **[derived]**. The REPLACE rows add up to about 10.3k lines (660 + 373 + 190 + 225 + 1326 + 2846 + 2070 + 2584), but most of `manifest.rs`, `schema.rs`, `playtest.rs` and `budget.rs` stays, so I do not count them as removable. The cut portions of the MIXED files are on top and are an **[estimate]**.

### 3.2 `editor/crates/psxed-ui` (94.5k lines, 85 files)

DELETE (wholly brush or BSP):

| File | Lines | Notes |
|---|---|---|
| `workspace/tools.rs` | 6686 | about 97% brush (1518 mentions): brush picking and handles, extrude, clip, CSG subtract/hollow, UV controls, vertex/edge drag. **Extract first**: `SelectTool` (`:120-347`, node and entity selection), `PaintDispatchTool` (`:349`) and the `ViewportTool3d` trait (`:82-96`) are shared with the grid tools |
| `tests/brush_tools.rs` | 6219 | 1419 mentions |
| `tests/ashen_sanctum.rs` | 1643 | reads `archive/fixtures/souls-bsp-vertical-slice`, uses `psx_bsp` (the only `psx_bsp` use in psxed-ui besides Cargo) |
| `tests/orthographic_brush.rs` | 466 | |
| `workspace/groups.rs` | 831 | brush groups (TrenchBroom style), with group clipboard |
| `workspace/terrain.rs` | 726 | terrain editor window |
| `workspace/uv_canvas.rs` | 568 | brush-face UV canvas (the grid inspector has its own UV controls; verify before deleting) |
| `workspace/brush_overlaps.rs`, `workspace/brush_elements.rs` | 202, 129 | |

Sum: **17,470 lines**; 140 `#[test]` in the test files and groups/uv_canvas **[measured]**.

MIXED (cut the brush parts; counts are keyword mentions, an **[estimate]**): `lib.rs` (4202; 276; `EditorWorkspace` holds ~300 fields including `selected_brush`, `brush_drag`, `brush_move`, `brush_vertex_drag`, `BspToolbarMode`, `BrushEditMode`, the leak-check job at `:142`), `workspace/selection.rs` (1141; 180), `workspace/geometry.rs` (1763; 138, brush copy/paste), `workspace/toolbars.rs` (1728; 130, the `BspToolbarMode` strip at `:598` and the inline 2D ladder at `:100-410`), `workspace/painting.rs` (1491; 112), `workspace/tools2d.rs` (254; 63), `workspace/editing.rs` (1352; 63, brush gizmo map), `workspace/input.rs` (1590; 53), `workspace/viewport.rs` (1308; 41, leak overlay `:345-550`), `inspector_transform_node.rs` (4850; 21, World node BSP settings), `tests/interaction_rig.rs` (1866; 348), `tests/project_workspace.rs` (4898; 310, many cook-to-`brush_world.pxbsp` tests), `tests/consistency.rs` (1167; 119), `tests/world_editing.rs` (960; 16), `play_mode.rs`, `scene_tree.rs`.

KEEP: `grid_tools/` (4.7k incl. `place_tool`, `wall_tool`, `rect_tool`, `height_tool`, `select_tool`, `face_tools`, `clipboard_tool`, `pick`, `view`, `overlay`, `panels`, `pose`, `events`, `keys`, `strip`), `grid_view.rs`, `tests/grid_*.rs` (61 tests across the grid test files and tool files **[measured]**), `viewport2d.rs` (shared painter), `gizmo.rs`, `history.rs`, `selection.rs`, `style.rs`, `icons.rs`, `model_animation_viewer.rs` (10.3k), `model_import_preview.rs` (3.7k), `inspector_character_ui.rs` (3.5k), `inspector_assets.rs`, `material_lab.rs` (2.1k), `ui_preview.rs`, `resource_browser.rs`, `scene_tree.rs`, `searchable_picker.rs`, `animation_catalogue.rs`, `starter_catalogue.rs` (check for brush starters), `play_overlay.rs`, `preview_overflow.rs`, `workspace/{panels,dialogs,ui_scene}.rs` and `panels/resources.rs`, `tests/{animation_studio,entity_resources,scene_tree_selection,viewport_gizmo,geometry_resources,ui_preview,ui_layout}.rs`, `examples/`.

REPLACE: the 2D tool ladder (`toolbars.rs:100-410`) becomes the `ViewportTool2d` trait the plan already calls for (`docs/grid/plan.md` phase P3); `workspace/tools2d.rs` is the start of it.

### 3.3 `editor/crates/psxed-mcp` (6.8k lines)

| File | Lines | Class |
|---|---|---|
| `edit.rs` | 1786 | DELETE: `add_shape`, `add_convex_brushes`, `transform_brushes`, `make_room`, `array`, `carve`, `set_material` (brush faces), `delete`, `get_brush`, `set_face_uv`, `group_brushes`, `groups` |
| `audit.rs`, `regions.rs` | 504, 123 | DELETE: PVS `audit`, `area_budget`, `regions` |
| `inspect.rs` | 244 | DELETE with `plan_view` (sections brush solids) and `walk_test` (hull walker) |
| `lib.rs` | 743 | MIXED: `plan_view`, `Brush` imports go |
| `main.rs` | 1655 | MIXED: the `#[rmcp::tool]` list. Keep `scene_info`, `entity_types`, `get_node`, `place_node`, `set_node`, `move_node`, `delete_node`, `add_light`, `lights`, `materials`, `playtest`, `screenshot`, `combat_flow`, `combat_duel`, `combat_timeline`, `set_combat_windows`, `status`, `save`, `revert`, `undo_last_edit`, `list_backups`, `metrics`; drop `set_cook_mode` (BSP cook mode) |
| `nodes.rs`, `play.rs`, `shot.rs`, `combat.rs`, `duel.rs`, `performance.rs` | 198, 241, 355, 126, 556, 260 | KEEP |

About 3k of 6.8k lines are brush tools **[derived: 1786 + 504 + 123 + 244 + wrappers, the wrapper part an estimate]**. REPLACE: add the grid tools listed in `docs/grid/psoxide-editor-architecture.md` section 5 (`grid_info`, `set_cell`, `paint_rect`, `set_wall`, `fill`, `grid_plan_view`, grid `walk_test` and `audit`), now backed by `psx-grid`'s `line_of_sight` and `locate` (runtime doc 5.5).

### 3.4 Emulator frontend files tracked here (`emu/crates/frontend/src`)

| File | Lines | Class |
|---|---|---|
| `editor_preview.rs` | 2974 | MIXED. Brush/CSG preview: `PreviewSolvedBrush*`, `PreviewCsgCache`, lit caches, `emit_brush_patch`, `walk_brushes*` (about `:711-1540`, roughly 830 lines), `walk_bsp_model_instances` (`:1584-1703`), `visible_sky_aperture` (`:1321`). Keep model instance drawing, scratch, camera, props. 213 brush/bsp mentions |
| `editor_preview/bsp_support.rs` | 295 | DELETE |
| `editor_preview/overlays.rs` | 1831 | MIXED: 63 brush mentions (leak path, brush selection outlines); keep node gizmos and bounds |
| `editor_preview/grid.rs` | 154 | KEEP (the grid walker; replaced later by the shared `psx-grid` renderer, runtime doc phase 8) |
| `editor_preview/{backdrop,camera,depth,particles,poi,primitives,props}.rs` | 343, 163, 169, 241, 326, 656, 172 | KEEP; `backdrop.rs` imports `psx_bsp::sky` (relocate) |
| `editor_textures.rs` | 2360 | KEEP (a test at `:2324` pushes a `Brush`; `psx_bsp::sky` constants at `:426`) |
| `editor_assets.rs`, `embedded_playtest.rs` | 542, 205 | KEEP |
| `playtest_disc.rs` | 874 | KEEP; the embedded `WORLD.PAK` builder from `room_NNN` chunk files (`:293-330`) is the container the runtime doc reuses. Comments and one test mention PXBSP (`:316`, `:731-750`) |

### 3.5 `engine/`

| Crate or file | Lines | Class | Reason |
|---|---|---|---|
| `crates/psx-bsp/src/render/grid.rs` | 1107 | KEEP, move | becomes the core of `psx-grid` (runtime doc phase 0); imports `FrustumPlanes`, `clip_polygon_plane`, `load_gte_clip_planes`, `open_hall_bands`, `PXBSP_CLIP_ALL_PLANES`, `TEXTURED_GOURAUD_COMMAND` from its parent `render.rs` (`grid.rs:39-42`) |
| `crates/psx-bsp/src/sky.rs` | 1926 | KEEP, move | view-ray sky (layered and cube); used by `editor-playtest/src/sky_runtime.rs`, `psxed-project/src/playtest/budget.rs`, `editor_preview.rs`, `editor_textures.rs`, `editor_preview/backdrop.rs`. Has nothing to do with BSP ("kept renderer-neutral", its own header) |
| `crates/psx-bsp` rest: `render.rs` 5265, `collision.rs` 3200, `collision_provider.rs` 648, `pxbsp.rs` 1278, `pxbsp_resident.rs` 1937, `pxbsp_stream.rs` 2847, `resident.rs` 1083, `mover.rs` 521, `destructible.rs` 220, `solid_cell_proof.rs` 460, `lib.rs` 2004, `visibility.rs` 3, `toolchain_probe.rs` 159, plus 9 examples and 2 tests | about 19.6k src | KEEP-FOR-CONSUMERS | quake-psx builds it (section 2). `CONTENTS_*` and `LiquidContentsSample` (in `collision.rs`) are used by editor-playtest and move to `psx-grid` as copies |
| `crates/psx-goldsrc` | 12.8k | KEEP-FOR-CONSUMERS | Half-Life/CS runtime pieces and oracle tests (`tests/oracles/legacy-*`, provenance JSON); CI runs its tests (`ci.yml` line 23) |
| `crates/psx-engine` | 40 files | KEEP | motor, camera, collision trait, render3d, affine surface, present queue. Check only `affine_surface.rs` for PXBSP comments; `world_render.rs` (378) is "material and lighting vocabulary shared by every world renderer" and stays |
| `crates/psx-level` | 3927 | KEEP | record types. MIXED at the edges: `LevelWorldObjectRecord` and its comments about PXBSP leaf links (`lib.rs:1554,1614`), `PXBSP` wording; `psx_level::MAX_ROOM_MATERIALS` naming |
| `crates/psx-game-runtime` | 40.9k | KEEP | entities, combat, props, particles, VRAM, CD stream. DELETE `region_stream.rs` (360, PXBSP region installer) and the `world-stream` feature wiring; KEEP `cd_stream/region.rs` (228) and `streaming/` (page pool, scheduler policy), which the grid streamer reuses. `logic.rs` has a PXBSP mention to clean |
| `crates/psx-render-contract`, `psx-carousel`, `psx-chainloader`, `psx-disc-toc` | small | KEEP | |
| `examples/editor-playtest/src/bsp_runtime.rs` | 2965 | REPLACE | `GridRuntime` (runtime doc section 6 is the member-by-member table) |
| `examples/editor-playtest/src/bsp_runtime/world_stream.rs`, `bsp_runtime/grid_world.rs` | 251, 140 | DELETE / REPLACE | PXBSP region streamer glue; prototype grid glue folds into `GridRuntime` |
| `examples/editor-playtest/src/{main,playtest_*,game_logic_runtime,sky_runtime,runtime_config,runtime_arenas,duel,hook_runtime,...}.rs` | ~19.5k | KEEP | ten files call `bsp.*` (counted: `game_logic_runtime`, `playtest_update`, `playtest_scene`, `playtest_runtime`, `main`, `duel`, `hook_runtime`, `player_ranged`, `marker_runtime`, `opening_sequence`) and `main.rs:391`, `playtest_runtime.rs:95-106`, `playtest_update.rs:532,1049-1050` name `psx_bsp::collision::CONTENTS_*`; they change signatures-compatibly |
| `examples/editor-playtest/Cargo.toml` features | 16 benchmark/knob features | MIXED | `world-order-{global,slot,linked,bucketed}`, `room-surface-profile` (13 cfg uses), `tr-subdivision-ceilings`, `tr-subdivision-wide-band`, `world-selection-reuse`, `tessellation-debug`, `sort-probe` are old world-renderer experiments: grep each `#[cfg(feature = ...)]` and delete the dead ones with `world-stream`, `grid-world` (replaced by the manifest switch) |
| `examples/editor-playtest/build.rs` | small | MIXED | `playtest_pxbsp` and `playtest_grid_world` cfgs (`:24-46`) collapse to one |
| `examples/{hello-engine,game-*,showcase-*,hardware-tests,toolchain-probe}` | | KEEP | nothing world-specific; `hardware-tests` is the measurement suite (memory `extend-hardware-tests-not-new-probes`) |

Open-hall bench knobs still present: `OPEN_HALL_GRID_LEVEL` (`brush_grid/mod.rs`, `raw_level.rs`, `playtest/grid_world.rs`, `schema.rs`), `OPEN_HALL_SAMPLED_VIS` (`brush_vis.rs`), `OPEN_HALL_SPLIT_BANDS` (`psx-bsp/src/render.rs`, `render/grid.rs`; it is a **build-time** override of the depth-band profile that the grid renderer's `GRID_RENDER_PROFILE` goes through `open_hall_bands`), `OPEN_HALL_FAR_WORLD` (`bsp_runtime.rs`). Delete all four, but keep the *capability* of choosing the near-split depths as an ordinary constant in `psx-grid` (the near-plane work in runtime doc 4.7 will tune it).

### 3.6 `tools/`, `Makefile`, CI

| Item | Class | Notes |
|---|---|---|
| `Makefile` targets `editor-blank-playtest-check` (`:948-1015`), `editor-bsp-liquid-check` (`:1016-1045`), `editor-souls-bsp-check` (`:1424`) | REPLACE | rebuild as grid equivalents (runtime doc phase 4 gates); they `cmp` brush fixtures and run `tools/check-editor-blank-playtest.py`, `tools/editor_souls_bsp_check.sh` |
| `Makefile` `cortex-ignition-v1-*` targets (`:1251-1331`) | AUDIT | build the shipped Cortex project disc (a brush project today). Keep the targets, re-point at the grid `default` |
| `Makefile` `profile-demo3*`, `profile-demo7-camera-sweep`, `probe-warp` | AUDIT | old demo-disc profiling; unrelated to world kind. Not grid specific, not harmful; leave unless they fail |
| `Makefile` `cook-playtest`, `build-editor-playtest`, `bootstrap`, `verify-components`, `material-audit`, `fmt`, `lint`, `lint-policy-guard`, `runtime-numeric-guard`, SDK example and hwtest targets | KEEP | |
| `tools/editor_souls_bsp_check.sh` (391), `tools/check-editor-blank-playtest.py` (391) | DELETE with their targets | |
| `tools/build_graybox_reach.py` (145), `tools/expand_graybox_reach.py` (111) | DELETE | build the brush Graybox Reach level |
| `tools/psoxide-perf` (`engine_stress.rs` 1544 with 9 mentions, `quake_chain_adapter.rs` 406, `bin/quake-chain-adapter.rs` 9, `mcp_client.rs` 508 with 6) | KEEP-FOR-CONSUMERS | quake-psx and hl-psx materialize `tools/psoxide-perf`; trim the Quake adapter only with them |
| `tools/psoxide-dev` (2690), `psoxide-hwtest`, `psoxide-vmcook`, the character/animation Python scripts, `build_guest_staged.sh` | KEEP | `build_guest_staged.sh` has 2 BSP mentions to check; `psoxide-dev` hosts `runtime-numeric-guard` |
| `.github/workflows/ci.yml` | KEEP, edit | steps 20-28 stay; `cargo test ... -p psx-goldsrc` stays while the crate stays; the rustfmt list of five frontend files stays; the editor test step shrinks with the deleted tests |
| `benchmarks/engine-stress/` (fixtures `e0,e1,base,rsmall` project.ron, 2026-10-06 results), `benchmarks/performance-suite/`, `benchmarks/corridor-frame300-reference.ppm`, `benchmarks/cortex_ignition_v1-bench-2026-06-11.pxtape` | MIXED | `engine-stress` is the PXBSP face/area-calibration campaign (39 files): DELETE with `psoxide-perf/src/engine_stress.rs` consumers; `performance-suite` is generic and stays |
| `validation/suite.ron` | KEEP | |

### 3.7 `docs/`, `editor/projects`, `editor/archive`, `assets`

`docs/` (227 tracked files): history. Deleting is free because `main` keeps it, but I recommend moving rather than deleting, so `grid/main` keeps one folder of retired context (`docs/archive-bsp/`) and the live docs stay short. BSP/Quake group (DELETE or archive): `bsp-engine-overhaul.md`, `quake-bsp-migration-plan.md`, `quake-parity-audit.md`, `quake-psoxide-convergence-handoff.md` (2936 lines), `quake2-renderer-transfer-2026-08-28.md`, `convergence-*` (4 files), `p6-pxbsp-streaming-followup.md`, `streaming-*` (6 files, partly salvaged by the runtime doc: keep `streaming-design-2026-10-08.md` section 10 and `streaming-audit-2026-07-25.md` until phase 5 is done), `brush-editor-integration.md`, `editor-terrain.md`, `engine-stress-limits-2026-10-03.md` (PXBSP face-count limits). Already retired grid docs (`legacy-grid-boundary.md`, `world-grid-architecture.md`, `floors-plan.md`, `floors-editor-architecture.md`, `world-format-roadmap.md`): they become *current* again for the grid revival, so **do not delete** them; update their banners instead and mark the roadmap superseded by the runtime doc. Perf/optimisation docs (`perf-*`, `optimization-*`, `instruction-level-*`, `cortex-*`, `hardware-test-*`, `emulator-accuracy-from-silicon.md`) KEEP: they are about the PS1 and the shared renderer, not the world kind.

`editor/projects`: tracked `default` (1058 files, 227 MB on disk), `graybox-reach` (622), `graybox-valley` (8), `cortex-two-area-level` (45), plus ignored local `grid-*` projects (the six imports and `grid-cathedral-edit`). All four tracked `project.ron` files are **brush worlds** (about 244, 99, 36 and 340 brush face lists respectively by grep) and `default` is both the New Project template seed and the Quake/HL-pinned path `editor/projects/default`. Classification:

* `default`: REPLACE the world, KEEP everything else. It carries characters, animations, audio and UI that every project seeds from (`lib.rs:86 DEFAULT_PROJECT_RON = include_str!("../../../projects/default/project.ron")`; `starter_catalogue.rs`). Convert its scene to a small grid starter (plan P5) and prune the brush data. Because quake-psx and hl-psx pin `editor/projects/default` at their revision, this only affects them after a bump.
* `graybox-reach`: the Reach character and enemies live here. KEEP the resources, REPLACE the world with a grid arena (the bench character per memory `cortex-grid-engine-preference-2026-10-16`).
* `graybox-valley`, `cortex-two-area-level`: brush levels. DELETE (they remain on `main`), or convert if Manny wants them as grid references.
* `stream-world.config.ron`: DELETE (streamed brush stress worlds).
* The `grid-*` projects are untracked (`projects/.gitignore` ignores everything not allow-listed). Decide which of the six become tracked fixtures (suggest `grid-cathedral` for the bench and one multi-room map such as `grid-dungeon` for portals).

`editor/archive/fixtures` (263 tracked files, 7.2 MB): `brush-directional-sky`, `brush-first-playable`, `brush-layered-sky`, `brush-open-courtyard`, `cortex-0.4`, `grounding-side-view`, `souls-bsp-vertical-slice` (239 files). All brush projects, all referenced by tests and the Makefile (`psxed-project/src/brush_playtest.rs`, `brush_compile.rs:1480`, `playtest/budget.rs:1125,1261`, `tests/scene_sky_contract.rs:12`, `lib.rs:458` as the New Project template, `psxed-ui/src/tests/project_workspace.rs:1308,2787,4050`, `tests/brush_tools.rs:1295`). DELETE after the tests that read them go (stage 2) and `new_project_template_dir` moves (stage 0).

`editor/assets` (sky, ui): KEEP. `assets/` top level (audio, branding, media, two model sets, textures): KEEP.

### 3.8 Materialized items nobody can delete here (record only)

* `.psxw` v5: `crates/psxed-format/src/world.rs` (508), `sdk/crates/psx-asset` `World` parser and its tests (`sdk/crates/psx-asset/src/tests.rs:32,51` pin the version). Superseded by PGL. Raise a change in the SDK repo after the grid format ships; leave untouched here.
* `tools/mkisopsx` `room_NNN.psxw/.psxc` chunk-name handling and the "no PSXW chunks" placeholder text: harmless, upstream.
* `crates/psx-iso` `build_world_pack` / `WORLD.PAK`: **KEEP and depend on it**; the runtime doc's container is this.

---

## 4. The knots (what blocks a naive `git rm`)

1. **`FaceUv` is defined in `brush.rs`** (`brush.rs:16`) and used by `grid/surface.rs` (`GridTri.uv`, `GridWall.uv`), `grid/faces.rs`, `brush_grid`, the grid inspector and the runtime doc's plane record. Move `FaceUv` and its `apply_linear`/`is_identity` helpers to the survivor types file (`scene_grid_types.rs` or a new `uv.rs`) and re-export it under the old path for one commit.
2. **The light bake is brush-shaped.** `grid/light.rs` calls `crate::brush_light::{lit_point_color, BrushPointLight}`; the grid cook calls `bake_brush_vertex_lighting` (via `brush_grid/mod.rs`) against *derived collision brushes as occluders*. Split `brush_light.rs`, and give the bake a non-brush occluder: the shared `line_of_sight` (runtime doc 4.6, 5.5). Until that exists the grid cook still needs the derived slabs, so `brush_grid/collision.rs` and part of `brush.rs` survive until runtime doc phase 3.
3. **New Project and `default` are brush worlds.** `lib.rs:458 new_project_template_dir = fixtures_dir().join("brush-open-courtyard")` and `DEFAULT_PROJECT_RON` (`lib.rs:86`) are read by the New Project dialog (`psxed-ui/src/lib.rs:3768,3907`), the starter catalogue (`starter_catalogue.rs:197`), `inspector_transform_node.rs:4487`, the frontend (`app.rs`, two uses), and about ten tests. Nothing in the editor can create a project without them. A grid starter (plan P5: one room, a floor, a spawn, a light, the Reach character) must exist and be wired in *before* the brush fixtures are removed.
4. **Sky and the grid renderer's helpers live in `psx-bsp`.** `sky.rs` (1926 lines) is imported by `editor-playtest/src/sky_runtime.rs`, `psxed-project/src/playtest/budget.rs`, and four frontend files. `render/grid.rs` imports six helpers from `psx-bsp/src/render.rs` (`grid.rs:39-42`). Extract both into a world-neutral crate (`psx-grid` for the renderer helpers; a small `psx-sky` or a `psx-engine` module for sky) and leave `psx_bsp::sky` as a re-export on `grid/main` so the PXBSP path keeps building during the transition.
5. **`Scene.brushes` is the world, and Play fails without it.** `playtest.rs:518-522` aborts the cook if the scene holds no brushes; `grid_world::prepare` adds *derived* brushes so a grid project passes. Removing `Scene.brushes` is only possible after the cook reads the grid directly (stage 6).
6. **The legacy-grid load guard.** `legacy_grid_node_count` (`document_types.rs:1806-1811`, error `ProjectIoError::LegacyGridWorld` at `lib.rs:495`, tests `tests.rs:3644-3682`) rejects the node kinds `Section(`, `Room(`, `Map(`, `WaterVolume(`, `Portal(`. The grid document uses `SceneWorld::Grid` and does not collide with them, but keep the guard (and its tests) so a pre-2026-10-08 grid project fails loudly instead of loading half-empty; reword its message.
7. **Frontend calls into deleted symbols** (section 1.1): the stubs need to be in place before stage 4.
8. **`units::scale_project_to_engine_units`** is called by `playtest.rs` after `grid_world::prepare` and scales brushes; the grid cook does its own `rt()` mapping. Both cannot scale the same project; the new cook scales nodes/lights/characters with the shared `units` functions and the grid with `rt()` in one place (runtime doc 2.2).
9. **`psxed-project` depends on `psx-bsp` for 58 `collision` and 23 `pxbsp_resident` references** (cook-side validation and tests). They disappear with the brush cook; until then the dependency stays.
10. **Tests that pin brush behaviour inside kept files.** `psxed-ui/tests/project_workspace.rs` (4898 lines, 310 mentions) mixes Play/cook plumbing tests (keep) with brush-cook byte-comparison tests (delete); same for `interaction_rig.rs` (`MouseRig` is worth keeping, its brush gestures are not), `consistency.rs`, `world_editing.rs`, `psxed-project/src/tests.rs` (4260 lines). Triage per test, not per file.
11. **Test counts to expect.** psxed-ui has 576 `#[test]`, psxed-project 689 (grep). The DELETE sets above hold about 140 and 241 respectively, plus an estimated 150 to 250 inside mixed files. The grid keeps 197 (61 UI + 136 project). The suite gets much smaller; CI time falls with it.

---

## 5. What quake-psx and hl-psx need from this repo (checked, not assumed)

| Item | quake-psx | hl-psx | Therefore |
|---|---|---|---|
| `engine/crates/psx-bsp` | yes (4 crates) | no | KEEP-FOR-CONSUMERS; do not edit in a way main lacks |
| `engine/crates/psx-engine` | yes | yes | KEEP; additive changes only |
| `engine/crates/psx-render-contract` | yes | yes | KEEP |
| `engine/crates/psx-goldsrc` | no | yes | KEEP-FOR-CONSUMERS |
| `engine/crates/psx-level`, `psx-game-runtime` | transitively (psx-engine needs `psx-level`) | transitively | KEEP |
| `editor/crates` (whole) and `editor/projects/default` | materialized (they build the host workspace for the frontend/emulator) | same | pinned; do not bump to `grid/main` |
| `tools/psoxide-{dev,hwtest,perf,vmcook}` | materialized | materialized | KEEP |
| `emu/crates/frontend` editor files | materialized | materialized | KEEP |

Not found: any dependency of either game on `psxed-project`'s brush modules by name (searched both repos' `Cargo.toml`, `Makefile`, shell and workflow files, excluding vendored copies under `.psoxide/`). I did not search their Rust sources for `psxed_project::brush`; they build the host workspace as a whole, so brush modules compile into their `.psoxide` copies without being called. **[unverified]** that nothing calls them; the pinned-revision rule makes it moot for now.

---

## 6. Removal order, building at every step

Check at the end of every stage (all must pass before the next stage starts):
`cargo check --locked --workspace --all-features` and `cargo test --locked -p psxed-project -p psxed-ui -p psxed-mcp --features frontend/editor` (root workspace), `cargo check --locked --manifest-path engine/Cargo.toml --workspace --all-features`, `cargo test --locked --manifest-path engine/examples/editor-playtest/Cargo.toml`, `make lint`, and one `make build-editor-playtest` plus an embedded Play of the grid starter. Commit per stage; never amend (memory `no-git-amend-in-active-tree`).

**Stage 0: make the deletable deletable (no behaviour change).**
Cut knots 1, 2 (split only), 3, 4. Add the grid starter, switch `new_project_template_dir` and `DEFAULT_PROJECT_RON` handling to it, keep the brush fixtures on disk. Create `psx-grid` and the sky crate with re-exports from `psx-bsp`. Add the frontend stubs (section 1.1) as the first edits of stage 4's files, but land them now so stage 4 is mechanical. Add a `ProjectWorldFormat`-style check in the cook so a project that is not `SceneWorld::Grid` gives a clear error (archaeology doc item 14: "fail closed on mixed state").
Build stays green because nothing is removed.

**Stage 1: zero-coupling deletions.**
Docs listed in 3.7 (move to `docs/archive-bsp/`), `tools/build_graybox_reach.py`, `tools/expand_graybox_reach.py`, `benchmarks/engine-stress/`, `editor/projects/stream-world.config.ron`, bins `gen_quake_e1m1_project`, the four `stream_*` bins, `pxbsp_draw_cost`, `pxbsp_seam_audit`, `gen_cortex_ignition_tech_demo_0_2`, `sync_cortex_ignition_tech_demo_0_2` (and their `[[bin]]` entries), `quake_map_import.rs`, `stream_world.rs`, `stream_walk.rs` (the bins above are their only users), the open-hall env knobs `OPEN_HALL_SAMPLED_VIS` and `OPEN_HALL_FAR_WORLD`. Remove `engine/crates/psx-bsp/examples/*` only if the quake-psx pin does not need them (it does not build `psx-bsp` examples; they are parity harnesses). Keep `OPEN_HALL_SPLIT_BANDS` until the near-split constant lands in `psx-grid`.
Build stays green because each item's only consumers are in the same list.

**Stage 2: remove brush authoring from the editor UI.**
Order inside the stage: (a) extract `SelectTool`, `PaintDispatchTool` and `ViewportTool3d` out of `workspace/tools.rs`; (b) delete `workspace/groups.rs`, `terrain.rs`, `uv_canvas.rs`, `brush_elements.rs`, `brush_overlaps.rs` and `tests/{brush_tools,orthographic_brush,ashen_sanctum}.rs`; (c) prune brush state and methods from `EditorWorkspace` and the toolbars (`BspToolbarMode`, `ViewTool::Brush`, the leak-check job), keeping the stubs from 1.1; (d) triage the mixed tests (knot 10); (e) delete the editor-side fixture reads, then `editor/archive/fixtures/*` brush fixtures and `gen_brush_*` bins and `gen_grounding_side_view`. `SceneWorld` can now collapse to grid-only in the UI (keep the enum variant `Brushes` in the *document* for one more stage so old files still deserialize).
After this stage the editor opens only grid projects; Play still works through the derived-brush path.

**Stage 3: MCP.**
Delete `edit.rs`, `audit.rs`, `regions.rs`, `inspect.rs`, the brush tools in `main.rs`, `plan_view`; add the grid tools. `psxed-mcp` has 21 tests (grep); keep the node/light/play/combat ones.

**Stage 4: frontend preview.**
Delete `editor_preview/bsp_support.rs`, the brush solve/CSG/lit caches, `walk_brushes*`, `walk_bsp_model_instances` (re-implement model instance placement on the grid walker), and the brush parts of `overlays.rs`. The frontend entry points keep their names (stubs). `rustfmt --check` on the five CI files must stay clean.

**Stage 5 (prerequisite, parallel track): `GridRuntime`.**
Runtime doc phases 0 to 4. Not a deletion; listed because stage 6 cannot start before its gates pass: the Cathedral route-tape walk, the lava/respawn gate, the souls-slice equivalent.

**Stage 6: remove PXBSP from Play.**
In this order: (a) switch `build_package` to the grid cook only and drop the `scene.brushes.is_empty()` guard, `grid_world::prepare`, `brush_grid/collision.rs`, `brush_grid/raw_level.rs`, `playtest/world_object_leaf_links.rs`; (b) manifest/schema: remove the PXBSP world records, `brush_world.pxbsp`/`.pts` outputs and `BRUSH_*_FILENAME` consumers (`psxed-ui/src/lib.rs:209`); (c) editor-playtest: delete `bsp_runtime.rs`, `bsp_runtime/world_stream.rs`, the `world-stream` and `grid-world` features, the dead bench features (3.5), collapse `build.rs` cfgs; (d) `psx-game-runtime/src/region_stream.rs`; (e) `Cargo.toml` of psxed-project and psxed-ui drop the `psx-bsp` dependency; the `psx-bsp` crate itself stays (stage 9).
The Makefile acceptance targets are rewritten in this stage (3.6).

**Stage 7: remove the brush data model.**
Delete `Scene.brushes`, `brush.rs` (after `FaceUv` moved), all remaining `brush_*` modules, `terrain.rs`, `solid_cell_*`, `units::scale_brush`, `ProjectDocument.bsp_*` fields and `WorldCullingSettings.bsp_patch_extent`, `MaterialResource.sky_aperture` if unused, `SceneWorld::Brushes`. Project files: `serde` tolerates unknown keys only where `deny_unknown_fields` is absent; add a one-shot migration in `from_ron_str` that strips the removed fields (the existing `migrate_legacy_project_ron` at `document_types.rs:1813` is the place) and verify with the project round-trip test (memory `project-ron-round-trip-not-byte-clean`: do not expect byte equality, assert semantic equality).
Convert or drop `graybox-valley` and `cortex-two-area-level`; convert `graybox-reach` and `default` worlds (stage 0 made the starter, now the shipped content).

**Stage 8: the bonnie importer and test fixtures for it.**
When Manny confirms the six references are final: delete `grid/bonnie*.rs`, `import_tests.rs`, `import_bonnie32_grid`, `import_bonnie32_level`, the `brotli-decompressor` dependency, and `docs/grid/bonnie32-editor-inventory.md` can stay as history.

**Stage 9: engine crates, only when Manny cuts the consumers loose.**
`psx-bsp` reduces to nothing once `sky` and the grid renderer have moved; it can be removed from `engine/Cargo.toml` members and from the root `[workspace.dependencies]` only after the emulator repo drops `psx-bsp` from its frontend manifest (section 1.1) and quake-psx no longer pins this repo for its engine (section 2). `psx-goldsrc` leaves the same way. Until then they stay and CI keeps testing them.

Rough totals when stages 1 to 8 are done **[estimate]**: psxed-project source falls from 98k to about 50k lines, psxed-ui from 94k to about 70k, psxed-mcp from 6.8k to about 3.8k, `editor-playtest` loses about 3.4k, `psx-game-runtime` loses 0.4k, the frontend editor files lose about 1.5k. About 80k lines in total.

---

## 7. Risks and decisions

1. **Order inversion.** Doing the deletions before `GridRuntime` exists leaves Play with no collision. The stage list keeps the PXBSP path alive through stage 5 for exactly this reason. Do not "just delete psx-bsp from the cook" early.
2. **Test loss is silent.** Deleting 140 + 241 tests removes the only coverage of the shared motor/camera behaviour that some brush tests happened to exercise (the motor's own tests are in `psx-engine` and stay). Triage mixed test files by what they assert, and port the Play/cook plumbing tests (`play_and_rebuild_buttons_emit_requests_through_real_egui_input` and its neighbours) before deleting their files.
3. **`default` is shared.** Rewriting `editor/projects/default` is invisible to Quake/HL until a bump, and then it is a surprise. Note it in the commit message and in the bump checklist.
4. **Un-mergeable branch.** After stage 6 `grid/main` cannot merge into `main` (rule R1). Decide now whether that is intended; if Manny wants one line eventually, the shared-engine extraction in section 2 item 4 has to happen first.
5. **Upstream coordination.** Four materialized frontend call sites and one Cargo manifest line need emulator-repo changes to finish cleanly; until then there are stubs. Same for `.psxw` in the SDK repo.
6. **Stale `make` targets referencing deleted binaries.** Several targets run `cargo run -p psxed-project --bin gen-brush-*`; grep the Makefile and CI for every deleted bin name in the same commit that deletes it.
7. **Unverified claims.** I did not build or run anything. Dependency edges come from grep and may miss macro-generated or string-based uses; the stage-end build checks are the actual test. The counts of tests inside mixed files are estimates.
8. **Memory notes that go stale.** Several auto-memory entries describe BSP-era workflows (`psoxide-testing-via-editor-play`, `editor-level-building-recipe`, `psxed-mcp-trenchbroom-parity-audit`, `soulslike-trenchbroom-level`, `trenchbroom-level-work-scope`). Update them when stage 3 lands so future sessions do not follow BSP recipes on `grid/main`.

Decisions for Manny: (1) keep or delete the tech-demo 0.2 generators; (2) which of the six `grid-*` projects become tracked fixtures; (3) whether `grid/main` is meant to stay unmergeable, or whether to extract the shared engine first (section 2, item 4); (4) convert or drop `graybox-valley` and `cortex-two-area-level`; (5) whether to move or delete BSP docs (I recommend `docs/archive-bsp/`).
