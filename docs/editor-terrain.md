# Low-poly terrain

In the Room workspace, choose **Terrain → Generate terrain…**. Generate rolling
hills, a rocky ridge, an island, or a flat patch. The seed is deterministic.
Cells control polygon density; cell size controls footprint independently of
height and roughness. Origin is the minimum X/Z corner and starting surface Y,
in editor world units. Generate applies recipe changes to the draft.

Drag on the top-view map to Raise, Lower, Smooth or Flatten. Radius is measured
in grid cells; strength controls the rate. Hold Shift to smooth or Alt to lower.
Right-click to sample the height for Flatten. Each stroke and regeneration has
local undo/redo. Drag the shaded 3D preview to orbit. Preview colors distinguish
slopes; the selected material is applied to the actual scene geometry.

**Add terrain to scene** commits a named group as one document undo step. Closing
the panel discards its unapplied draft. Select that group (or one of its brushes)
and choose **Terrain → Edit selected terrain…** to reopen it, including after
saving, loading, duplicating or translating the group. Sculpt edits retain
existing per-face materials and UV settings unless a material is explicitly
chosen. Regenerating a different footprint may create new faces using the
selected material.

Terrain is a heightfield: one surface height at each X/Z grid vertex, with
alternating triangle diagonals and 16-unit height quantization. Up to 16 × 16
cells produce 512 top triangles and 512 closed convex brushes. Start at the
8 × 8 default and increase only where the landscape needs more shape. The
project's normal world/collision compiler and runtime budgets still apply.

The base sits 256 units below the generation origin; sculpting clamps above it
so it cannot invert a solid or punch holes. Shared lattice vertices prevent
cracks between triangles. Ordinary BSP tools can subsequently edit the brushes;
cutting, rotating off the grid, or deleting part of the patch may make it
ineligible for heightfield sculpting. Those edits are preserved and the terrain
panel reports the incompatibility rather than replacing them. Use ordinary
brushes for caves, bridges and overhangs.

For a new standalone patch, **Enclose with sky** adds five solid sky-aperture
brushes: four boundary walls and a ceiling at the chosen Y. The terrain closes
the bottom. A sky-aperture material must already exist. The enclosure is a child
group, so terrain sculpting still reopens normally and undo removes both together.
The World node controls the visible sky. These walls also block traversal.

Enclosures remain ordinary brushes: resize them if you expand the terrain or
raise it above the ceiling. Cut matching openings when joining another sealed
area. Automatically enclosing every patch in a connected landscape would wall
off its neighbours, so this option is off by default. Run the BSP leak check
after making connections.
