# Renderer cleanup, 5 September 2026

Removed 22 unselected classic-affine feature switches and their implementations.
These covered fixed-fan rendering, resident subdivision caches, alternative
subdivision kernels, shared-edge experiments and alternate depth selectors.
Quake's corresponding cleanup removes 40 switches and 46 obsolete benchmark
commands. Earlier experiments remain available in Git history.

The engine still supports the paths selected by actual projects:

| Path | Consumer |
| --- | --- |
| General classic-affine renderer | Half-Life and BSP editor playtests |
| GPU polygon/lattice clipping and specialized Quake kernel | Quake's default renderer |
| TR subdivision lattice | Grid-based editor playtests |

Quake retains its validated historical SDK/engine pin. Removing experiments
from current engine source does not rewrite that dependency revision.

## Checks

- Engine default and retained-feature test configurations pass; default has 416 tests.
- Engine Clippy passes for all targets and retained features with warnings denied.
- Cortex 0.4b rebuild is byte-identical to the previous executable:
  `48af21becf039267b0330c9e35452fa3e00c39309254b321c55d3707a0d70a1c`.
- Quake's host builder passes 59 tests. Normal and benchmark guest builds pass
  post-link instruction-hazard checks.
- Quake's baseline and candidate each complete two deterministic E1M1 routes.
  Both record 1,682 presentations and 2,239,253,715 emulated bus cycles
  (25.425 fps). VRAM hash: `6c23b5e6511bc16e`; display hash: `621bf7ee03f427a4`.
  Gameplay checkpoints and map transitions also match.

These are emulator comparisons. This cleanup has not been tested on a console.
Published disc downloads still use their previous validated source revisions.
