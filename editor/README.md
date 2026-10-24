# `editor/` (editor & content pipeline)

The editor authors sector-grid levels and PSoXide gameplay records. Quake-PSX
maintains its own game-specific content pipeline.

The host-side authoring stack: the project model, the asset cookers that
turn source content (OBJ/glTF meshes, PNG/JPG textures, WAV audio) into
binary blobs the runtime consumes, and the reusable egui UI panels. The
editor's Play action cooks the active scene and hands it to
[`engine/examples/editor-playtest`](../engine/examples/editor-playtest).

These crates run on the host, not on PSX target; they are members of
the repo-root HOST workspace (one lockfile shared with `crates/`,
`emu/`, and `tools/`).

## Grid level quickstart

This is the shortest supported create/edit/play loop. From the repository
root, bootstrap its pinned dependencies and launch the native editor:

```bash
make bootstrap
cargo run -p psoxide-emulator -- --editor --windowed
```

1. Choose **File → New Project…**, enter a name and press **Create**. The
   project is a grid project: a 6 by 6 sector room with a floor, a ceiling and
   walls on the whole outline, one lamp and the Graybox Reach player in the
   middle. It borrows the player, camera, sky and assets from
   `editor/projects/graybox-reach` (its `assets` folder links there).
2. Shape the level with the grid tool strip: **Floor**, **Ceiling**, **Wall**,
   **Height**, **Paint**, **Split** and **Place**, in the 3D view or the Top
   view. **Select** picks vertices, edges and faces; Ctrl/Cmd+C and Ctrl/Cmd+V
   copy and stamp a selection. Press `?` for every key and gesture. Rooms live
   in the Rooms panel and may overlap in plan, which is how floors stack.
3. Pick a Material in the palette to apply it to the selected faces, or use
   **Paint** to apply it by clicking. Imported textures appear as Materials in
   the Resources panel.
4. A project may have one player source. Move the starter's player, or use
   **Place** with Player Spawn, to put it elsewhere. **Point Light** and the
   other node kinds come from the same Place menu. For a door, add a **Logic**
   node, change its kind to **Door** and name the Box Prop it moves.
5. Save with **File → Save** (`Cmd/Ctrl+S`) and press **Play**. Play saves the
   authored state, cooks the grid level, builds the real PS1 runtime, and boots
   it in the editor viewport. The first build is slower because it compiles the
   guest; later iterations reuse it.
6. Stop or return to editing, make a change, then press **Rebuild & Play**.
   No generated-file copying or command-line cook is required. Use the Play
   button's chevron to inspect the latest PS1 memory/packet budget.

If Play refuses the level, the status strip reports the cook error and focuses
the first actionable node/resource where possible. [`docs/grid/editor-guide.md`](../docs/grid/editor-guide.md)
covers every tool, panel and key.

## Crates

| Crate | Purpose |
|-------|---------|
| [`psxed`](crates/psxed) | Content-pipeline CLI. Cooks source assets into binary blobs the runtime consumes. |
| [`psxed-project`](crates/psxed-project) | Editor project model: scenes, nodes, resources, and PS1-facing authoring metadata. |
| [`psxed-ui`](crates/psxed-ui) | Reusable egui workspace panels for the editor. |
| [`psxed-format`](https://github.com/EBonura/PSoXide/tree/main/crates/psxed-format) | Cooked-asset binary formats. Shared with the SDK's `psx-asset` parser so layout drift is impossible. |
| [`psxed-obj`](crates/psxed-obj) | OBJ parser + vertex-cluster decimator. Emits PSXM blobs. |
| [`psxed-gltf`](crates/psxed-gltf) | glTF/GLB mesh importer. Cooks scene meshes into PSXM blobs. |
| [`psxed-tex`](crates/psxed-tex) | PNG/JPG → PSXT texture cooker: crop, resample, quantise to 4/8-bit CLUT, pack for VRAM. |
| [`psxed-audio`](crates/psxed-audio) | WAV → PS1 SPU ADPCM audio cooker. |

## The format contract

`psxed-format` is owned by the SDK and imported under `crates/psxed-format`.
It is the single source of truth for cooked-asset layout: the
editor crates here are the **producers**, and the SDK's
[`psx-asset`](../sdk/crates/psx-asset) is the **consumer**. Both depend on
`psxed-format`, so a layout change can't drift between the two sides.

## See also

- [Root README](../README.md#quick-start). Launching the editor.
- [`docs/editor-architecture.md`](../docs/editor-architecture.md). Editor internals.
- [`docs/editor-model-authoring.md`](../docs/editor-model-authoring.md), [`docs/editor-lighting.md`](../docs/editor-lighting.md), [`docs/editor-runtime-coordinates.md`](../docs/editor-runtime-coordinates.md). Authoring workflows.
