# Graybox Valley

Outdoor BSP traversal fixture using Manny's original Picotron grid texture.
Open `project.ron` in the editor. The project shares the tracked character,
animation, UI and sky assets in `../default`; keep the two projects together.

## Recovered texture

`source_assets/textures/picotron-grid-64.png` is sprite **15** from
`gfx/0.gfx` in `archlight_v3_0.p64`, also present in `archlight_v4_0.p64`.
The cartridges live in `~/Library/Application Support/picotron/drive/`.
It is the original 64×64 indexed bitmap: blue fill, pale grid, and **64** in
the corner. Extraction preserved every pixel and used Picotron's default
palette. `provenance.json` records the source cartridge and hashes.

`assets/textures/picotron-grid-64.psxt` contains the same bitmap as an opaque
4bpp PS1 texture, with its three colours converted to RGB555. It occupies
2,108 bytes including the palette and headers. No resampling or dithering.
Ground and cliff materials tint this same texture; the original material has
neutral tint. At the current 400% face scale, one full tile spans 4,096 authored
units. The **64** label describes texture pixels, not metres or world units.

## Layout

- Open courtyard and a broad gate, with the player facing the exit.
- Valley banks and a central headland to test long views and partial occlusion.
- Raised east path with ramps at both ends and a projecting overlook.
- North terrace with two enemies and a tower landmark.
- Physical outer boundary and sky brushes sealing the BSP.

The playable footprint is 32,768 × 53,248 authored units: 32 × 52 player
heights. The raised path is 1,536 units high and the tower is 10,240 units.
The scene has 36 brushes in named groups. It uses Release cooking, a 4,096-unit
BSP patch extent, a 60,000-unit draw distance and three baked point lights.
The player wake-up sequence and introductory message are disabled here.

## Build and run

From the repository root, with the release frontend built:

```sh
target/release/frontend build-project-disc --project editor/projects/graybox-valley
target/release/frontend --editor --editor-project editor/projects/graybox-valley
```

The disc is written to `baked/graybox_valley.cue`. Use the editor's Play control
to explore with the normal game controls.

## Validation — 2026-10-06

The editor audit found no degenerate brushes, coplanar face overlaps, off-grid
vertices or leak to the exterior. Cooked geometry: 421 world faces, 792 base
triangles, 957 all-face packet slots, 65 non-solid leaves. The audit reports all
421 faces potentially visible from the heaviest leaves; this is still a broad
outdoor visibility workload. The existing missing player turn-clip warning
remains.

Engine collision traces reached both the main route and the complete raised
loop, including the overlook and both ramps, using the standard player hull.
A PS1 guest build completed and a headless emulator run walked from the
courtyard through the valley to the north terrace. Gameplay screenshots confirm
the grid and its labels render correctly.

With experimental DMA FIFO enabled, the moving traversal averaged 24.38 fps
over guest frames 300–1199 and 27.54 fps over 1200–1999. This is one camera route,
not a minimum-frame-rate guarantee or a combat benchmark. It has not been
verified on a physical console. Evidence is under `build/graybox-valley/`:
`audit.txt`, `walk-main.txt`, `walk-raised-loop.txt`, and `forward/` logs/captures.


## Normal Play recheck

The earlier timings above used an instrumented guest. A matched normal Play build
(`cd-stream-bench`, without `emulator-telemetry`) averaged 27.653 fps on the automatic
forward traversal and held 29.645 fps in the final view, under the default DMA model.
The north-terrace PVS includes 421 candidate faces. This contradicts treating 120 PVS
faces as a general 30 fps ceiling. The MCP default cap and one-enemy restriction were
withdrawn. See `benchmarks/engine-stress/valley-recheck.md` for the controlled A/B,
FIFO results, scope and provenance.
