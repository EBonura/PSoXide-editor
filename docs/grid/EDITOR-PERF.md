# Editor performance (2026-10-18)

Branch `grid/editor-perf-2026-10-18`, rebased on `grid/main` fe9fef18. Machine: the shared dev Mac with other lanes building at the same time, so every figure is noisy by tens of percent run to run. Every before/after pair below was measured back to back on the same machine, in alternating runs, so the ratio means more than the absolute number. Baseline is `grid/main` fe9fef18 (the preview figures) or f0e212a5 (the live windows).

Manny's report: an idle editor window sat at 100% CPU for 6.5 hours. It reproduces: **an untouched `grid-tower` window costs 97 to 100% of a core** in a release build. The window redrew the whole UI and rebuilt the whole 3D preview 60 times a second whether or not anything changed.

## What an editor frame was made of

The 3D view is the emulator's software GPU fed a command log that `editor_preview` rebuilds from the document. A frame is: the egui UI (`EditorWorkspace::draw`), the preview build (`build_phase1_frame_reusing`), the texture and asset refresh the shell runs before it, the GPU submit, the egui paint. Sampling the live window (`sample`) and the headless harnesses showed where it went:

| Cost | Where | Why |
|---|---|---|
| Preview build, every frame | `editor_preview/grid.rs` `walk_grid` | derived every visible cell's walls, tiled its textures, baked the lights, per frame, though none of it depends on the camera. Plus `cell_bounds` (a full wall derivation) for every cell of every room |
| UI: Play menu | `draw_build_play_controls` | `estimate_playtest_budgets` (derives every face of the world) ran every frame for a menu that is closed. 81% of a tower UI frame |
| UI: texture thumbnails | `refresh_texture_thumbs` | re-signed every material (Debug formatting) and re-opened every unreadable file, every frame |
| UI: Top view | `grid_view::draw_grid_world` | re-derived the walls of every visible cell, every frame. 90% of a tower Top-view frame |
| Texture/asset refresh | `EditorTextures::refresh`, `refresh_models`, `EditorAssets::refresh` | built a plan and stat'ed every material and model file, every frame (1.5 ms on the tower) |
| Shell tick | `main.rs` `schedule_next_redraw` | 60 Hz tick whenever the editor workspace is front, no matter what |

Not costs: project load (4 to 5 ms, 8 ms with `open_directory`), save (5 to 6 ms), a document clone for an undo snapshot (0.05 to 0.09 ms) or the dedupe compare (0.02 to 0.04 ms). Memory: a cathedral document is about 0.8 MiB, so the 64 undo snapshots are 53 MiB on the largest level and 3 to 11 MiB on the others; the live window is about 500 MiB before and about 510 MiB after (the preview now keeps a snapshot of the document and the prepared faces). The first four were not touched.

## What changed

1. **Prepared grid geometry** (`editor_preview/grid_cache.rs`, `state.rs`). Everything about a face that does not depend on the camera (shade, baked corner colours, rebased texels, the cook's tile cuts) is kept per room and per box (cell, plane, placed mesh). A room is thrown away when it changes (compared with a snapshot, so an edit to one room leaves the others), and every room is thrown away when what all faces are made from changes: the world's scale, facing shade or mesh assets, the project's materials, the lights, the texture slots. A room's boxes and cull bounds come first (the view needs the bounds of every box); a box's faces are made the first time the camera sees it, so a drag across a large room pays for what is visible, no more than before. A frame culls, applies the cutaway and emits.
2. **Frame memo.** A frame whose inputs are all unchanged and that reads nothing from the animation clock is handed back as it was. What "unchanged" means is a deep compare against a snapshot of the document (microseconds when equal), not a revision counter, so no edit path can forget to bump it. A frame reads the clock when it draws an autoplaying model, a POI beacon, a particle emitter or a layered sky; every reference level has an autoplaying model, so on those the memo never hits and the win is the cheap frame, not the skipped one.
3. **Refresh throttle.** `EditorTextures::refresh`, `refresh_models` and `EditorAssets::refresh` skip when the document is unchanged and they looked less than 500 ms ago (an image saved from elsewhere shows within half a second). `EditorAssets::scan` is the unthrottled form the tests use.
4. **UI**: the Play menu's budget estimate is computed only while the menu is open, once per document revision; texture thumbnails re-scan on a document change or every 2 s; the Top view keeps its derived wall strokes per revision (`DerivedWallLines`, the slice filter still applies per frame).
5. **Redraw policy** (`psxed-ui/src/redraw.rs`). `EditorWorkspace::redraw_interval()`: 60 Hz for 1.5 s after any input, 30 Hz (the console's rate) for a cook or build or for a focused 3D view whose preview draws from the clock, 10 Hz otherwise. The preview reports whether it moves with the clock (`publish_preview_animated`). The unconditional repaint request of POI beacons, which pinned the window at the display rate, is gone in favour of that. **The shell side is an emulator-repo change, see below.**

## Numbers

Preview build, per frame, release (`editor_preview::perf`, median of three alternating runs, baseline fe9fef18):

| Level (camera) | Orbit / static frame before | after | Texture refresh before | after | Edit + rebuild before | after |
|---|---|---|---|---|---|---|
| cathedral (overview) | 1.79 ms | 0.50 | 0.25 | 0.07 | 1.79 | 2.01 |
| cathedral (interior) | 0.74 | 0.25 | | | 0.74 | 0.90 |
| tower (overview) | 7.58 | 0.34 | 1.49 | 0.05 | 7.54 | 7.84 |
| tower (interior) | 4.46 | 0.19 | | | 4.45 | 4.69 |
| dungeon (overview) | 0.54 | 0.20 | 0.26 | 0.04 | 0.53 | 0.47 |
| dungeon (interior) | 0.20 | 0.12 | | | 0.20 | 0.22 |
| level008 (overview) | 0.58 | 0.21 | 0.25 | 0.05 | 0.58 | 0.70 |
| level008 (interior) | 0.29 | 0.14 | | | 0.29 | 0.39 |

The edit rebuild is the one number that did not improve: an edit throws the changed room away and the visible cells are prepared again, which is what the old path did every frame, plus noticing the change (compare and snapshot of the document, 0.07 to 0.11 ms, and the room snapshot, 0.02 to 0.11 ms) costs 0.1 to 0.2 ms on a single-room level. Of the 8 ms a tower edit takes, 1.8 ms is deriving the cull bounds of the room and the rest is preparing the visible cells' faces. See the levers below. A level with several rooms rebuilds only the edited room.

UI frame (`workspace.draw`, 1800 by 1000, dev profile test binary, baseline fe9fef18):

| Level | 3D view idle | 3D view, pointer moving | Top view idle |
|---|---|---|---|
| cathedral | 1.33 ms to 0.37 | 1.40 to 0.43 | 1.43 to 0.45 |
| tower | **6.93 to 0.34** | 7.02 to 0.39 | **9.40 to 0.55** |
| dungeon | 1.06 to 0.33 | 1.09 to 0.36 | 1.09 to 0.36 |
| level008 | 1.03 to 0.34 | 1.05 to 0.37 | 1.09 to 0.39 |

Live window, `frontend --editor --windowed`, idle for 9 s after start, CPU from `top` (release; alternating base and new runs; base is f0e212a5, new includes the shell redraw policy):

| Level | Before | After |
|---|---|---|
| cathedral | 24 to 28% | 8 to 9% |
| tower | **97 to 100%** | 6.5 to 6.8% |
| dungeon | 16 to 21% | 5 to 7% |
| level008 | 17 to 21% | 6 to 7% |

On the cathedral, with the editor-side changes alone (the shell still ticking at 60 Hz) the window sat at 12 to 13%, so about half the saving is each. The remaining 6 to 9% is the animated preview (an autoplaying model) redrawing at 30 Hz: about 1.5 ms of UI and paint per frame. A level with nothing animated would tick at 10 Hz.

## Proof that nothing visual or cooked changed

* Preview commands: `editor_preview::perf::reference_levels_draw_what_they_always_drew` hashes the full GPU command log and overlay lines of the nine grid levels (cathedral, tower, dungeon, level008, cave, sewers, west, starter, gate) from two cameras each with the animation clock pinned. The 18 hashes were recorded from the code before the cache and pass unchanged.
* Incremental equals cold: `an_incremental_build_equals_a_cold_one_after_every_kind_of_edit` raises a floor, recolours a material, darkens a room and moves a light on three levels, building each time from the warm cache and from nothing, and compares the hashes. `a_still_scene_is_handed_back_and_any_change_rebuilds_it` covers the memo; `grid_cache::tests` covers the invalidation rules (an edit to one room keeps the other; a changed context drops all).
* Editor screenshots: `dump-editor-ui --view 3d` and `--view top` over the same nine levels, baseline binary against new binary, 18 full 1920 by 1080 editor PNGs, byte-identical.
* Cooked bytes: `tools/grid-simgate/cookhash.sh` over the 8 golden levels, `core` and `rooms` identical to `cookhash-golden.txt` for all 8 (the `manifest` column embeds checkout paths, see PLAY-SPEED.md; this change does not touch the cook).
* Tests: `cargo test -p psxed-ui` 530 pass (519 before, plus the harness and redraw tests; the interaction tests are unchanged). `cargo test -p frontend --features editor --release` 200 pass; the one failure, `cli::press_script_tests::editor_frame_selection_is_applied_before_preview_extraction`, fails the same way on the baseline (it reads `editor/archive/fixtures/brush-first-playable`, which is not in the repository). Clippy `-D warnings` is clean for `psxed-ui` and `frontend` with all targets.

## The shell change (needs the emulator repository)

`emu/crates/frontend/src/main.rs` (`schedule_next_redraw`) is a hydrated emulator file, so it is not edited here. The change is one commit on top of the pinned emulator revision 6edde550: `cc9d6734` on `EBonura/PSoXide-emulator`, branch `editor/idle-redraw-2026-10-18`. It makes the paused shell ask `state.editor.redraw_interval()` instead of ticking at 60 Hz whenever the editor is front. To take it: land it on the emulator's main, then bump `components.lock.json` (`emulator.revision`) and re-hydrate (`psoxide-components --root . [--source emulator=<checkout>]`). It was built and measured here by dropping its `main.rs` over the hydrated one for the live runs above and restoring the original afterwards (`psoxide-components --check` matches again). Until the bump the editor is simply never asked for the interval and ticks at 60 Hz, which is still 2 to 4 times cheaper than before.

## Run the harnesses

```bash
# UI frames, hover, edits, load/save, undo snapshot size (dev profile, as `make run` is)
cargo test -p psxed-ui perf_ -- --ignored --nocapture --test-threads=1
# preview build per camera, orbit, edit, texture refresh, GPU submit
cargo test -p frontend --features editor --release editor_preview::perf -- --ignored --nocapture --test-threads=1
# live window: run it, `top -l 4 -s 2 -pid <pid>`, kill it
frontend --editor --windowed --editor-project editor/projects/grid-tower
```

`PERF_LEVELS=grid-tower,grid-dungeon` narrows the levels; `PERF_FRAMES_MUL=30` stretches the UI loops so a sampler (`sample <pid> 3`) can attach to the test process.

## Next levers

1. **Edit rebuild.** Cell-level invalidation: only the cells within the dependency radius of the edited leaves need new faces and bounds. The radius is not one cell (a derived wall's texture frame runs along the whole wall), so it needs a randomised equivalence test (incremental against cold, hundreds of random edits per reference level) before it can be trusted. It would take a tower edit from 8 ms toward 1 ms.
2. **Cull bounds from a coarse box first.** `cell_bounds` derives every cell's walls to get a tight box (1.8 ms on the tower); a box made from the cell and its neighbours' height ranges contains it, so only cells that pass the coarse test need the exact one. Output stays identical if the visibility test is monotone in box containment, which needs checking.
3. **Skip the GPU submit and the egui paint on an unchanged frame** (`gfx.rs`, `psx-gpu-render`: emulator repository). Worth about 0.1 to 0.4 ms of CPU per tick.
4. **`[profile.dev.package.psxed-ui]` and `psxed-project` at opt-level 2** in the root `Cargo.toml`: the dev profile builds both at opt-level 0, while `frontend` is at 2. Measured on the UI frame only: 0.80 to 0.62 ms (cathedral). Not applied: the edit-time derivation in `psxed-project` would gain more, but that was not measured in the dev profile, and it slows every incremental rebuild of the editor crates.
5. **Animated previews at 30 Hz are a policy choice**, not a measurement: the console renders at 30 Hz, so the preview matches it, but if you want the editor window's model idle at 60 Hz, change `REDRAW_ANIMATED` in `redraw.rs`.
6. The Play cook still runs on the UI thread (PLAY-SPEED.md); background work that already exists (the heat overlay worker) is untouched.
