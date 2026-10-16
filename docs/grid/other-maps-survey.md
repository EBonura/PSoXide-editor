# bonnie-32 sample levels: survey for the grid-renderer bench

Read-only survey of Cave, Dungeon, Sewers, West and level_008 (decompressed copies in `build/oh-runs/scratch/levels/`). Nothing was built in the open-hall worktree. The survey tool is `build/oh-runs/grid/survey/b32stats/` (a copy of the scratch `b32vis` analyzer with extra stats, a walk graph and a route search, built with its own target dir under /tmp and deleted afterwards). Raw tool output is in `survey/raw/`. Images are in `survey/` (see the last section).

Conventions used below. Sector coordinates are `(gx,gz)` inside a room, the same as the importer's `--spawn X,Z`. Yaw 0 faces +Z, yaw 90 faces +X (the importer uses `forward = (sin yaw, cos yaw)`, and the Cathedral route `--yaw 90` walks east). "Tiles" are the analyzer's PS1 primitives (floors, ceilings, and walls cut into sector-tall bands). Anything marked (est) is an estimate from reading code or from the stated assumption, not a measurement from a run.

## 1. Summary

| | Cave | Dungeon | Sewers | West | level_008 |
|---|---|---|---|---|---|
| Rooms | 2 | 2 (room 1 raised 2048) | 1 | 1 | 1 |
| Grid per room (w x d) | 7x13, 6x8 | 7x19, 10x17 | 6x9 | 8x8 | 21x18 |
| Sectors present | 39 + 24 | 113 + 96 | 46 | 49 | 332 |
| Portals (bonnie) | 6 | 90 | 0 | 0 | 0 |
| Floors / ceilings / walls | 63 / 48 / 34 | 162 / 46 / 146 | 46 / 0 / 59 | 49 / 0 / 23 | 332 / 0 / 58 |
| Analyzer tiles | 205 | 498 | 157 | 87 | 565 |
| Cook primitives (est, quads + tris) | 118 + 90 | 429 + 58 | 135 + 30 | 81 + 10 | 281 + 260 |
| Floor y range (bonnie units) | -512 to 768 | -256 to 2048 | 0 to 512 | 0 to 2560 | 0 to 3072 |
| Ceiling y range | 1792 to 2816 | 1536 to 4608 | none | none | none |
| Extent (sectors x y z) | 7.0 x 3.8 x 14.0 | 10.0 x 4.8 x 19.0 | 6.0 x 2.2 x 9.0 | 8.0 x 3.0 x 8.0 | 21.0 x 4.5 x 18.0 |
| Level's own player height | 762 | 762 | 1200 | 1200 | 762 |
| Frustum tiles p50 / p90 / max | 43 / 132 / 191 | 103 / 290 / 456 | 42 / 85 / 133 | 17 / 58 / 84 | 121 / 336 / 540 |
| Perfect-occlusion p50 / p90 / max | 35 / 92 / 130 | 45 / 131 / 224 | 35 / 77 / 103 | 15 / 43 / 77 | 90 / 211 / 377 |
| 16-cut + 2x2 merge p50 / p90 / max | 41 / 88 / 113 | 86 / 145 / 193 | 42 / 77 / 99 | 17 / 54 / 75 | 74 / 132 / 163 |
| Import blockers as the importer stands | texture packs | empty texture ref (panic), texture packs | none on macOS | `_USER` texture (panic), texture packs | none |
| Recommended spawn / yaw | 4,12 / 180 | 1,0 / 0 | 2,8 / 180 | 5,7 / 180 | 9,17 / 180 |

The Cathedral for scale: 1725 tiles, frustum p50 397 / p90 928 / max 1467. level_008 is the only other map that is open enough to look like it, at about a third of the tile count.

Analyzer numbers come from `scratch/out/report.txt` (all floor sectors, 8 yaws, eye 1700, 90 degree FOV). I re-ran the same tile model for walkable cells only and the frustum figures agree (Cave max 191, Dungeon 456, Sewers 133, West 84, level_008 540).

## 2. Per map

### 2.1 level_008 (open arena, bench first)

**Size.** One room, 21x18 sectors, 332 sectors present, no ceilings, no portals. 332 floors (167 flat, 30 sloped planar, 135 split into two triangles, of which 46 use the NE-SW diagonal and 24 carry `heights_2`), 58 walls (56 cardinal, 2 diagonal; tall ones are cut into 188 bands). Floor y 0 to 3072, walls up to 2816 tall. The plan is an open plain at y 0 in the south and east, a rim of raised cells (up to 3072) along the north and west edges with a sloped band between, and two 3x3 raised blocks (2304) at cells (8..10, 7..9) and (4..6, 14..16). The tiles per view are high because the plain has almost no occluders.

**Importer gaps.** Single texture pack (dark-military-fantasy, 4 textures, all PNGs present), no empty refs, all coordinates on the 16-unit grid, nothing zero-height, nothing non-solid. Gaps that change the look or count:

| Gap | Count | Effect |
|---|---|---|
| Custom UVs (floor / wall) | 4 / 3, plus 4 `uv_2` floors | Wrong mapping on 11 faces; face count unchanged |
| Per-vertex colours | 46 floors plus 13 second-triangle colour sets; every non-neutral corner is 96 (25 percent darker than 128), 118 corners in all | Lost; subtle darkening only |
| Blend modes | none | None |
| Double sided | none (30 walls use `Back`, which the importer does handle) | None |
| Props | 1 mesh asset at cell (16,15) (`asset_003`, tall tapered tower, 52 vertices, about 13 faces) | Not imported: about 13 faces and one obstacle missing in the original's middle-south |
| Open floor edges (no neighbour, no wall) | 54 of 1328 | Hull leak diagnostics only (see section 4) |
| Fog | enabled (dark red-black), start 24064, falloff 5120, cull 2560 | Starts beyond the 21 sector extent, so effectively off; importer ignores it either way |

Visually honest without fixes: yes for geometry and face count. It will look flat-lit because there are no lights (point 3).

**Lighting.** No lights in the RON. Ambient 0.452. The tint above is the only carried shading. Generic default: ambient 115 (0.452 x 255), plus a lattice of warm point lights (Cathedral WARM 255,176,104, intensity 1.25, radius 6 sectors), one per about 36 floor cells, so roughly 9 lights (est).

**Bench route.** Spawn `9,17`, yaw 180 (walk north up column 9 across the whole plain). The floor is flat at y 0 along the route; it stops at the 3x3 block occupying rows 7 to 9 (a 2304 unit riser, not climbable). Length 7.5 sectors, about 138 ticks (est). Why: the per-cell heat map (max over 8 yaws) is hottest along the south edge and the south-east corner (7 to 9 of 9, worst frustum 540 and worst occlusion 377 both at cell `18,17` yaw 225), and the route starts on that hot edge and looks north across the whole plain. Route views: p50 367 / p90 437 / max 452; a 360 turn at the end p50 155 / p90 240 / max 241. Alternate that passes exactly through the worst view: spawn `18,17` yaw 225, length 10.5 sectors (about 195 ticks), walk p50 363 / p90 491 / max 540, 360 at the end max 248. The diagonal ends in the corner of the block and would slide along its face if held too long, so use the exact tick count with no margin. The mesh tower at (16,15) is not imported, so the diagonal is clear in the editor (it would not be in the original).

### 2.2 Dungeon

**Size.** Two rooms. Room 0 at world (-4096,0,-10240), 7x19, 113 sectors. Room 1 at (-7168,2048,-10240), 10x17, 96 sectors, floor at y 2048. Room 1 has three floored columns on the west side (51 cells, open top) and the rest are wall-only sectors that sit over 45 plan cells of room 0 (those 45 are the 45 + 45 portals, all horizontal openings). 162 floors (128 flat, 6 sloped planar, 28 split), 46 ceilings, 146 walls (2 triangular, 3 edges with stacked walls), 290 bands. Floor y -256 to 2048, ceiling 1536 to 4608, walls up to 2560. 144 floor sectors have no ceiling (mostly open to a night skybox). Room 0 is a long hall with a narrow tunnel at its south end. The two rooms are not walkably connected at the editor's step height (25 adjacent cell pairs with a jump above 72 bonnie units, up to 2048; the stair-like cells in room 0's south-west corner climb in 256 steps).

**Importer gaps.**

| Gap | Count | Effect |
|---|---|---|
| Empty texture reference (pack "" name "") | 20 floors, room 0 cells (0..6, 10..12) | The importer reads `<dir>/.png` and panics. Bonnie draws a fallback checkerboard. The cook would also drop them as unmapped. Blocker |
| Two texture packs (64x-opengameart, tiny-texture-pack) | 7 textures | One flat `<dir>/<name>.png` lookup fails unless the PNGs are staged into one directory |
| Custom UVs | 59 walls (58 of them Projected, which matches the importer's world-aligned mapping) | Mostly fine; 1 to 2 walls differ |
| Per-vertex colours | 2 walls (values 104 and 64) | Lost |
| Black-keyed transparency | GemOv3: 13 walls, 63.5 percent black texels; Grating_09: 3 walls, 5.8 percent | Importer keeps black opaque. The grating is a see-through gate in bonnie |
| Double sided / Back | 1 wall Both, 14 walls Back | Back handled, Both treated as one-sided |
| Skybox | night gradient with stars | Ignored (importer uses its sunset sky) |
| Open floor edges | 4 of 648 | Negligible |
| Stacked rooms | 45 plan cells hold sectors from both rooms | The grid cook buckets faces per plan cell with a y range, so these cells get tall bounds. Not verified against the pack reader |

Visually honest without fixes: no, it does not import at all until the empty ref is mapped. After that, geometry and counts are right; the gem panels and the grate render as opaque.

**Lighting.** No lights, no objects. Ambient 0.283 (room 0) and 0.291 (room 1). Generic default: ambient 72 to 74, warm lattice lights; room 0 has 111 floor cells so about 3 lights by the one-per-36-cells rule (est), plus 2 or 3 inside the tunnel and the raised corridor if you want it visibly lit.

**Bench route.** Spawn `1,0`, yaw 0 (south down column 1 through the whole hall). The column is flat y 0 for 14.4 sectors, about 270 ticks (est), and ends where row 15 has no floor. Route views: p50 204 / p90 349 / max 383; 360 at the end p50 86 / p90 337 / max 347. The analyzer's worst frustum (456 at `3,18` looking north) is in the south tunnel behind the Grating_09 gate (the grate is a solid wall on the north edge of cells `2..4,16`) and is not reachable by the walker. Under the importer's fixed scale that tunnel also has ceilings below the 1012 unit character (8 of 162 floor cells). So 383 is the reachable maximum on this route. Higher-mean alternate: spawn `4,0` yaw 0, 9.4 sectors (about 174 ticks), p50 260 / p90 340 / max 367, 360 at the end max 217; it stops at a 768 high block at `4,10`.

### 2.3 Cave

**Size.** Two rooms, adjacent edge to edge with no plan overlap. Room 0 at (-3072,0,-2048), 7x13, 39 sectors. Room 1 at (-2048,0,-3072), 6x8, 24 sectors. 3 + 3 portals (vertical doorways). 63 floors (19 flat, 13 sloped planar, 31 split), 48 ceilings (37 sloped), 34 cardinal walls (94 bands), 15 floor cells without ceiling. Floor y -512 to 768, ceiling 1792 to 2816. A sloped winding passage: room 0 rises from y -512 in the south to 0 and narrows to a single central column (the cells beside it have low roofs); room 1 is a wider chamber at y 512 to 768 in the north that slopes down to 0 where it meets room 0.

**Importer gaps.** Almost clean.

| Gap | Count | Effect |
|---|---|---|
| Three texture packs (64x-opengameart, dark-fantasy-townhouse, tiny-texture-pack) | 5 textures | Lookup by one directory fails unless staged. Blocker |
| Custom UVs | 1 floor, 1 wall | Negligible |
| Per-vertex colours, blend, double sided, transparency | 0 | None (no texture has black texels) |
| Default-mode walls | all 34 | Bonnie stretches the texture once over the wall height, the importer tiles it. Look only |
| Low ceilings | 13 of 63 floor cells have under 1800 units of headroom; the level's own player is 762 tall | With the importer's fixed 9/16 scale the character (1012) does not fit there. The passage's central column is unaffected |
| Open floor edges | 10 of 252 | Negligible |
| Objects | 3 legacy entries (2 lights, 1 PlayerStart) | Not imported |

Visually honest: yes once textures resolve. Face counts exact. The geometry is the most faithful of the five.

**Lighting.** It carries its own lights, stored as legacy `object_type: Light` entries (bonnie's current loader keys instances on `asset_id`, so verify whether it still reads these). Room 0 ambient 0.205, room 1 ambient 0.236.

| Light | Room cell | Height above floor | Colour | Intensity | Radius |
|---|---|---|---|---|---|
| Cool | room 0 `3,10` | 896 | 56,64,192 | 2.0 | 10965.669 (10.7 sectors) |
| Warm | room 1 `3,1` | 1344 | 248,136,112 | 1.0 | 9030.551 (8.8 sectors) |

These should drive the bake instead of Cathedral lamps: ambient 52 and 60, two point lights with those colours and intensities, radius in sectors as listed. Bonnie's PlayerStart is room 0 `1,10` height 448.

**Bench route.** Spawn `4,12` (room 0), yaw 180: walk north along world column 1, up the ramp, through the narrow room 0 neck into room 1 and up its centre column to `3,0`. Length 13.5 sectors, about 253 ticks (est); no riser above 72 units anywhere on the path. Route views p50 75 / p90 134 / max 161; 360 at the end `3,0` p50 16 / p90 186 / max 186 (the level's worst frustum is 191 at room 1 `5,0` yaw 315, two cells away). The reverse direction (start room 1 `3,0` yaw 0) scores higher (p50 104 / p90 164 / max 183, 360 max 168) but starts in room 1, which the importer cannot spawn into (point 4 of the importer list).

### 2.4 Sewers

**Size.** One room, 6x9, 46 sectors, no ceilings, no portals. 46 floors (24 flat, 8 sloped planar, 14 split), 59 walls (53 cardinal, 6 diagonal, 2 triangular, 23 edges with stacked walls), 105 bands. Floor y 0 to 512, walls up to 1280. A small basin with a 4 cell alcove at the north end (rows 0 and 1) cut off from the main 42 cell basin by walls (no height jump separates them).

**Importer gaps.** One pack (retro-texture-pack, 8 textures; the files use an uppercase `.PNG` extension, which works on macOS's case-insensitive volume and would fail on a case-sensitive one). 35 walls carry custom UVs (57 of 59 walls are Projected, which matches the importer). 4 walls are Back (handled), 0 Both, 0 vertex colours, 0 blend modes, 6 floors have a second texture (handled). Black-keyed texels exist in BRICK_6D (8.1 percent), FLOOR_3A (4.7), TECH_1C and TECH_1E (6.8): dark mortar lines that bonnie draws see-through and the importer draws black. One asset instance at `1,8` whose id (6305284058618307782) is not among the shipped sample assets, probably the player start. 6 of 184 floor edges are open. Visually honest without fixes: yes, apart from the lighting and the 35 custom-UV walls.

**Lighting.** None carried. Ambient 0.378 (96). Default: ambient plus 1 or 2 lattice lights (46 floor cells / 36, est).

**Bench route.** Spawn `2,8`, yaw 180 (north up column 2 to `2,2`). Floor heights run between 0 and 512 as continuous slopes (no riser above 72 units anywhere on the map). Length 6.5 sectors, about 118 ticks (est). Route views p50 60 / p90 96 / max 105; 360 at the end p50 37 / p90 86 / max 94. The worst frustum (133 at `1,0` yaw 0) is inside the sealed 4 cell alcove, so it is not reachable; the best reachable views seen on routes are about 105. This map is cheap in absolute terms and mainly validates the light path and a single-pack import.

### 2.5 West

**Size.** One room, 8x8, 49 sectors, no ceilings, no portals. 49 floors (44 flat, 5 split, 1 of them NE-SW), 23 walls (22 cardinal, 1 diagonal, 5 edges with stacked walls), 37 bands, walls up to 3072 tall. Floor y 0 to 2560: a flat 8x5 yard at y 0 plus a 3x3 spur to the north (cols 5 to 7, rows 0 to 2) topped by three isolated raised cells (2304, 2560, 2304) that the walker cannot reach (jump 2560). The walk graph has one 46 cell component and three single cells.

**Importer gaps.**

| Gap | Count | Effect |
|---|---|---|
| `_USER` texture (texture_001) | 4 floors | The brotli RON indexed 64x64 4bpp texture, not a PNG. The importer panics. Blocker (substituting any pack texture is fine for a perf bench) |
| Three packs | 14 textures (1 `_USER`, 5 dark-fantasy-townhouse, 8 dark-military-fantasy) | Staging needed. The 5 townhouse textures are 32x64 and the importer forces every texture to 64x64, so they stretch |
| Custom UVs | 10 floors, 23 of 23 walls | Wrong mapping, face count unchanged |
| Double sided | 3 walls (Both) | Treated as one-sided |
| Black-keyed transparency | beach_bridge on 5 walls, 70.1 percent black | Bonnie draws bridge rails with holes; importer draws solid. Face count unchanged |
| Open floor edges | 25 of 196 | The yard has no boundary walls on most sides. Leak diagnostics, and the player can walk off the world edge |
| Objects | 3 legacy entries (2 lights, 1 PlayerStart at `5,4` height 1024) | Not imported |

Visually honest without fixes: no (does not import), after the texture fix it is acceptable.

**Lighting.** Carries two lights (legacy entries): room cell `1,3` height 512 and `6,3` height 2048, both colour 248,152,152, intensity 1.197, radius 3999.244 (3.9 sectors). Ambient 0.5 (128), the brightest of the set.

**Bench route.** Spawn `5,7`, yaw 180 (north along column 5 to `5,1`). Continuous flat floor; length 6.5 sectors, about 118 ticks (est). Route views p50 37 / p90 62 / max 67; 360 at the end p50 23 / p90 58 / max 68. The worst view (84, at `0,7` yaw 135) is a diagonal from the south-west corner, so the best cardinal route does not touch it. Alternate along the middle row: spawn `0,4` yaw 90, 7.4 sectors (about 135 ticks), p50 27 / p90 70 / max 73. It is the lightest map by far and will hold 30 fps trivially.

## 3. Lighting summary

| Map | Authored lights | Ambient | Vertex colours | Other |
|---|---|---|---|---|
| Cave | 2 (legacy entries, listed above) | 0.205 / 0.236 | none | Use authored lights |
| Dungeon | none | 0.283 / 0.291 | 2 walls | Night skybox |
| Sewers | none | 0.378 | none | |
| West | 2 (legacy entries, listed above) | 0.5 | none | Use authored lights |
| level_008 | none | 0.452 | 46 floors at 96 (darker) | Distant fog (off in practice) |

Generic default for maps with no lights: ambient = round(bonnie ambient x 255), plus warm point lights (255,176,104, intensity 1.25, radius 6 sectors, the Cathedral's nave setting) on a lattice of one per about 36 floor cells, 2048 above the floor or 256 below the ceiling when a ceiling is lower. The importer currently gates lighting on `name == "Cathedral"` and never sets ambient from the level, so all five need this generalised. Lighting is baked into vertex colours, so it should cost about the same as the Cathedral per face; it matters for how the bench looks, not for the primitive count.

Vertex colours are mild everywhere (level_008 25 percent darkening, Dungeon 2 walls). Losing them is acceptable.

## 4. Importer changes, in order

What the importer does today: loops over every room (so multi-room geometry lands at the right world offsets), ignores portals, objects, fog, skybox, vertex colours, UVs, blend modes, transparency and double-sided, handles `Back`, `heights_2`, `texture_2`, `split_direction`, diagonal walls and stacked walls, rejects zero-height walls (none exist in these five), and asserts every coordinate is on the 16-unit grid (all five pass). The cook in `brush_grid.rs` mirrors the same parsing and additionally drops any face whose texture name has no project material.

Needed before the first two benches (level_008, Dungeon):

1. Resolve textures as `<packs-root>/<pack>/<name>.png` (case-insensitive extension) and key materials by pack and name. Interim workaround with no code change: copy the used PNGs into one flat directory per level (no name collisions exist across packs in these five). level_008 needs only one pack so it works today.
2. Map an empty texture ref to a fallback material (a checker or flat colour), registered as a project material so `brush_grid.rs` does not drop the 20 Dungeon floors as unmapped. Without this the Dungeon cannot be imported.
3. Replace the `name == "Cathedral"` light gate with the generic lighting in section 3: ambient from the room, authored lights if the RON has them, otherwise the lattice.

Needed for the remaining three:

4. `--room N` so `--spawn`, the printed map and perches use a room other than room 0 (only needed for the stronger forward Cave route; the recommended Cave route starts in room 0).
5. `_USER` textures (West): decode the bonnie texture RON, or substitute a pack texture.

Optional for visual honesty, none changes the primitive count materially:

6. Per-level scale. The importer and `brush_grid.rs` hardcode 9/16 (576 units per sector, 18 per texel), which is right for the Cathedral's 1800 unit player. Cave, Dungeon and level_008 are authored for a 762 unit player and Sewers and West for 1200. With the fixed scale Cave loses 13 of 63 floor cells and Dungeon 8 of 162 to low ceilings. A `--scale NUM/DEN` of 21/16 (player about 1000, sector 1344) for the 762 maps and 7/8 (player 1050, sector 896) for the 1200 maps keeps every value on the 16-unit grid and makes everything walkable (re-run with per-level player height: Cave 63 of 63, Dungeon 162 of 162, worst walkable views 194, 449, 129, 86, 542). The cook's `CELL_EDITOR`, `TEXEL_EDITOR` and `SCALE_*` constants would need to follow.
7. Black-keyed transparency (Dungeon GemOv3 and Grating, West beach_bridge, mortar lines in Sewers) and `normal_mode: Both` as double sided (4 walls total).
8. Custom UV passthrough (Dungeon 59, Sewers 35, West 33, level_008 11 faces).
9. Props (level_008's tower mesh, about 13 faces) and the legacy PlayerStart spawn.
10. A perimeter skirt for floor edges that have no neighbour and no wall (West 25, level_008 54, Cave 10, Sewers 6, Dungeon 4). The BSP outside fill keeps a leaking world and reports it rather than failing, but the player can walk off the world.

## 5. What to bench first

1. level_008. Largest tile count after the Cathedral (565 tiles, frustum p90 336, max 540), the only other open-air hall, one room, one pack, no blockers. It needs only the lighting generalisation (change 3) to be a fair second data point. It tests exactly the open-area question (draw distance, far cull) at a size where a regression is visible.
2. Dungeon. 498 tiles, indoor and vertical (walls up to 2560, p90 290), two stacked rooms. It tests multi-room cook and per-cell y ranges, which the Cathedral never did. Needs changes 1 to 3.
3. Cave. 205 tiles, two rooms, authored lights and low ceilings. A good lighting check and the cleanest geometry. Needs changes 1 and 3 (use the authored lights).
4. Sewers. 157 tiles, single pack. Cheap baseline and a sanity check of the light path. No blockers on macOS.
5. West. 87 tiles. Lightest, needs change 5. Bench it last or not at all; it will sit at 30 fps and add little.

## 6. Route scripts

Speed assumption (est, unverified by a run): the project's `walk_speed` is 30 and grid projects cook speeds at authored scale (`units.rs`, `speeds_to_q8_unscaled`), so the player moves 30 editor units per tick, one 576 unit sector per 19.2 ticks. Estimated walk ticks = (length - 0.33) x 19.2, where 0.33 sector is the 188 unit radius. The walks end against a wall or riser, so a margin of 20 ticks is harmless on the axis-aligned routes. The Cathedral route (700 ticks) would cover 36 sectors, more than any of these maps, so these walks are shorter and the summary window should be `stop-at-poll - 1200` instead of 1100. The 300 tick right-stick turn is assumed to be a full 360 as in the Cathedral run; confirm with one `--dump-hw` frame.

Prefix for all: `200:cross:20,400:cross:20,600:cross:20,800:cross:20,900:cross:240`. Walk starts at 1200, the turn starts 50 ticks after the walk, stop 50 ticks after the turn.

| Map | `--spawn` | `--yaw` | Walk ticks | Remaining press entries | `--stop-at-poll` | Window |
|---|---|---|---|---|---|---|
| level_008 | 9,17 | 180 | 160 | `1200:lstick=128/0:160,1410:rstick=255/128:300` | 1760 | 560 |
| Dungeon | 1,0 | 0 | 290 | `1200:lstick=128/0:290,1540:rstick=255/128:300` | 1890 | 690 |
| Cave | 4,12 | 180 | 275 | `1200:lstick=128/0:275,1525:rstick=255/128:300` | 1875 | 675 |
| Sewers | 2,8 | 180 | 140 | `1200:lstick=128/0:140,1390:rstick=255/128:300` | 1740 | 540 |
| West | 5,7 | 180 | 140 | `1200:lstick=128/0:140,1390:rstick=255/128:300` | 1740 | 540 |

If a longer measured window is wanted, walk out, turn 180 degrees (150 ticks if the 360 takes 300) and walk back; I did not tune that.

## 7. Files

All under `build/oh-runs/grid/survey/`:

- `<map>-overview-3d.png`: the analyzer's 320x240 overview (copied from `scratch/out`).
- `<map>-worst-frustum.png`: the analyzer's worst-frustum view (copied).
- `<map>-plan-route.png`: plan map, one panel per room (panels are not world-aligned). Floor colour runs blue (low) to orange (high); brown-red cells are floor cells the 1800 unit walker cannot use (low ceiling); hatching means no ceiling; red cell edges are solid walls, yellow non-solid, orange diagonals; the green square is the recommended start and white dots the route.
- `<map>-route-mid.png`, `<map>-route-end.png`: tile-id renders of the view in the middle and at the end of the recommended route.
- `raw/stats_<map>.txt`: full tool output, including ASCII floor maps, per-cell heat maps and the top route candidates (`statsB_*.txt` is the same with each level's own player height). `raw/stats.txt` is an earlier all-levels run.
- `b32stats/`: the tool source (`cargo build --release` there with its own `CARGO_TARGET_DIR`).
