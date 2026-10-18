# Play speed: where a Play spends its time (2026-10-18)

Status: measured, first fix landed, the rest is queued. Branch `grid/play-speed-2026-10-18` on `grid/main` d35a24ee. Machine: the shared dev Mac with other lanes building at the same time (7 cargo/rustc processes were running), so every figure is noisy by a few seconds. Raw logs: `campaign-2026-10-15/play-speed/`.

The editor's Play does, in order: save, cook in process (`cook_playtest_to_dir`: `build_package` then `write_package`), `make build-editor-playtest` (bootstrap check, `tools/build_guest_staged.sh`: rsync to the stage, `cargo build --release` of the MIPS guest, hazard patch, stack guard), `build_embedded_playtest_disc` (in process), load the disc and run. The CLI `cook-playtest` is the same cook plus a second `build_package` for its summary.

## Measured before (grid-tower and grid-cathedral)

| Phase | Time | Notes |
|---|---|---|
| `make bootstrap` | 0.3 s | receipt match; not a cost on a hydrated tree |
| Cook, CLI `cook-playtest` | tower 8.2 to 9.0 s, cathedral 10.5 s | log shows every `[cook] tracks ...` line twice (two `build_package` calls) |
| of which `build_package` | about 0.45 s each (cathedral) | model and animation cooking is NOT the cost |
| of which XA music encode | 10.7 s (cathedral, one 12.8 MB `MUSIC1.XA`) | resample + ADPCM, redone on every cook even when the WAV is unchanged |
| Guest build, cold stage (cargo-home kept) | 32.3 s total, cargo 28.4 s | LTO release, deps included |
| Guest build, warm, nothing changed | 2.5 s | rsync `--checksum` keeps mtimes, cargo has nothing to do |
| Guest build, warm, level changed (tower to cathedral) | 17.0 s total, cargo 14.5 s | the cost the embedded level forces |
| Disc image + launch | not yet measured | see next steps |

So the brief's two suspicions were half right: the animation cook is run twice by the CLI but it is cheap. The expensive repeat work is the XA music encode, which dominates the cook.

## Landed

1. XA music cache (`playtest/manifest.rs` `write_xa_music`): the encoded files are kept in `generated/.cook-cache/` under a key over the WAV bytes, baked speeds, file layout and `XA_COOK_REVISION`; a hit hard-links the cached file into `generated/xa_music/`. One entry is kept, so it never grows. Bump `XA_COOK_REVISION` when `psx-audio-cook` or `xa_music::encode` change their output.
2. `cook-playtest` builds the package once and writes that package (it called `build_package` twice).
3. Timing lines in the cook output: `[cook] timing: package N ms` (editor Play and CLI) and `[cook] timing: write N ms (XA music N ms)`, so the Console shows where a Play went.

## Measured after

| Phase | Before | After |
|---|---|---|
| Cook CLI, cathedral, music unchanged | 10.5 s | 0.6 s |
| Cook CLI, cathedral, music changed or first time | 10.5 s | 11.8 s (encode, once) |
| Editor in-process cook (package + write) | about 11 s | about 0.6 s on a hit |

Evidence: `cookhash.sh` over the 8 golden levels: `core` and `rooms` hashes identical to `tools/grid-simgate/cookhash-golden.txt` for all 8. The `manifest` column differs for all 8 because the manifest embeds absolute checkout paths in comments (`/// UI_SFX_SAMPLE_000_BYTES - /Users/...`), so the golden only holds in the checkout that recorded it. That is an existing flaw in the gate, not caused by this change (this change does not touch manifest rendering). Fix: hash the manifest with the checkout root normalised.

## Next steps, in order of payoff

1. Measure the disc build and launch (`frontend build-project-disc --project editor/projects/grid-tower/project.ron` with `PSOXIDE_GUEST_STAGE_ROOT` private; time `build_embedded_playtest_disc` and the headless `launch` to first frame). Not done yet.
2. Level data off the EXE, to remove the 14 to 17 s incremental guest build on a level edit. Findings so far:
   - The guest `include_bytes!`s `grid_level.pgl` and `grid_rooms.pgr` (`grid_runtime.rs` lines 52 and 61). `GridRuntime::load_manifest` already adds rooms through `add_room(index, &'static [u8])` and `ResidentRooms`, so loading them from the disc at boot (read into a leaked, word-aligned buffer) fits the streaming design (`streaming-design.md`: unit = room record).
   - Disc layout: WORLD.PAK is header-only and UI.PAK follows it, so UI and music LBAs are baked into the manifest. The level pack must go AFTER the music files (or be a separate named file) so a level of a different size does not move UI.PAK or the music table. The guest then needs the pack's LBA without a manifest edit: either a fixed header sector the disc builder patches, or an ISO9660 lookup by name.
   - The manifest (entities, lights, spawns, characters, 431 KB of Rust statics) still changes on entity edits and still forces the rebuild. Moving only pgl/pgr helps geometry-only edits. A full fix needs the level-dependent part of the manifest read from disc too; that is a larger design (typed records into a byte format) and should be decided before building step 2.
   - Gates to update with it: `cookhash.sh` (hash the pack file instead of the two embedded files), simgate (cooked bytes identical, only their home changes), `mkisopsx`/`playtest_disc.rs` file order, and the `sim-trace` disc build.
   - `playtest_disc.rs` and `embedded_playtest.rs` are editor-owned; `emu/crates/frontend/src/app.rs` and `cli.rs` come from the emulator component (changing them needs a change in `EBonura/PSoXide-emulator` and a lock bump). `Makefile` and `tools/build_guest_staged.sh` are editor-owned, so the Play entry can change without touching `app.rs`.
3. A content-addressed cache for model and animation cooks is NOT worth building: `build_package` is 0.45 s. Skip until a heavier project shows otherwise.
4. Bootstrap stamp: not worth it, 0.3 s.
5. Cold guest builds: use a persistent private `PSOXIDE_GUEST_STAGE_ROOT` per checkout; the shared `/tmp/psoxide-psx-guest-v1` is serialised by a lock between lanes.
6. Fix the cookhash manifest column (normalise the checkout root) so the golden is usable from any worktree.
