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

## Detail wedges and the bed slab

Every wedge is a **detail brush** (Quake 2/3 semantics, the *Detail brush*
checkbox in the brush inspector). A detail brush is drawn and collides, but it
contributes no splitter plane to the render tree, portals or visibility; its
faces are assigned to the structural leaves they touch. The group also holds
one structural **bed slab** under the wedges, spanning the footprint up to one
height step below the lowest vertex. The slab seals the underside (the wedges
alone would leave the world open) and is the terrain's only contribution to the
tree. Its top and sides are buried in the wedges, so it never shows. Groups
saved before this change hold structural wedges only; they still open, and
applying them rewrites the group as detail wedges plus a bed.

Detail brushes still carve and are carved by structural brushes when the render
surfaces are built, so buried faces disappear as before. Liquids are always
structural.

Point traces (the camera arm, projectiles, melee) normally walk the render BSP,
which cannot see detail brushes. A model with any detail brush therefore also
stores an exact point clip hull in head slot 1, and the editor-playtest runtime
prefers it for those traces. Maps without detail brushes keep the empty
sentinel there and cook byte for byte as before.

Measured on the Graybox Terrain study (emulator, normal release guest, input
tape polls 420 to 1590; the route differs between variants):

| Wedges | Structural | Detail + bed |
| --- | ---: | ---: |
| 32 | 27.29 fps, 17,760 B | 26.40 fps, 23,564 B |
| 64 | 19.51 fps, 32,276 B | 18.95 fps, 45,316 B |
| 128 | 15.41 fps, 58,144 B | 15.15 fps, 82,124 B |

Detail brushes collapse the tree to the bed and enclosure (24 nodes, one open
leaf, a one-byte PVS row), but they do not buy frame rate. Almost all of the
terrain's cost is collision against the body hulls, which detail brushes do not
change, and the single large leaf loses node-box culling for the terrain faces.
Use detail wedges for the shorter tree and PVS, not for speed.

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
