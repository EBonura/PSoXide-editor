# Compact grid level format: a measured design study (2026-10-17)

Status: design study with a working host-side prototype. Branch `grid/compact` (grid/main a943d9c4). Nothing here changes the shipped format; the only code added is the measurement tool `editor/crates/psxed-project/examples/grid_compact/`. Manny's brief: with quadtrees there is room for a very compact level representation, for disc, RAM and streaming. His standing rule, added mid-study: use as few bits as possible for everything (section 2).

How to read the numbers. Same tags as the other grid docs. **[measured]** came out of the tool on the cooked reference levels (the full Play cook, `playtest::build_package`, lights included, `editor/crates/psxed-project/src/playtest/grid_world.rs:255`). **[derived]** is arithmetic on measured values or on struct sizes. **[estimate]** is reasoning with no run behind it (every R3000 cycle figure is one). **[hypothesis]** names the experiment that settles it. No figure is a hardware measurement.

Path shorthand: `E/` = `editor/crates/psxed-project/src/grid/`, `P/` = `engine/crates/psx-grid/src/`, `X/` = `editor/crates/psxed-project/examples/grid_compact/`.

---

## 0. Summary

1. **The quadtree is not where the compaction comes from.** What makes a grid level small is that almost everything in the cooked tables is predictable from the sector heights: floor and ceiling quads from the sector corners, wall quads from the collision wall records, vertex ids from welding, plane constants from the ring, UVs from vertex positions, cell tables from the quads. A predict-and-correct stream that codes only the corrections turns the Cathedral chunk from **111,300 B to 9,779 B bit-packed (8.8 %) or 5,591 B range coded (5.0 %)**, bit-exact, decoded from the real bytes and compared table by table **[measured, section 5]**. The seven reference levels together go from 223,712 B to 21,647 B packed (10.3x) or 13,654 B range coded (16.4x). Deflate on the raw tables reaches 32 %, an LZ4-style matcher 54 % **[measured]**.
2. **The quadtree is still worth building, for other reasons.** On disc, planar prediction already costs 1.8 bits per Cathedral sector (193 B of 5,591 B), the same as a leaf flag would. As resident collision it shrinks five of the seven reference levels by 3 to 34 % at 20 B per leaf plus 2 B per sector and grows two small ones by 2 and 4 % **[measured]**; the large wins the tools design counts on need large flat areas that none of the reference levels has. Its real jobs are the independent decode block (4 sector aligned blocks), culling and LOD (T15), and the memory of future open terrain.
3. **Baked light is now half the file.** After prediction the colour field is about 50 to 70 % of the bytes (2,878 B of 5,591 B for the Cathedral). Colour is an exact function of (vertex, normal) in every level (0 conflicts), shared 1.3 to 2.6 corner uses per key; point lights make every floor quad distinct, so a colour-aware quad merge removes nothing at zero tolerance and at most about 10 % of a level's quads at tolerance 16 of 255 **[measured]**. Lossy 5 bit light would take a further 22 %, not recommended without a frame check.
4. **On disc the format stops being the bottleneck; the seek is.** At 7.5 to 11.8 B per cell instead of 122 B, a 16 by 16 sector unit is about 2 to 3 KB, one or two CD sectors. The streaming design's frontier limit (92 cells per sector moved at 16 sector units, `docs/grid/streaming-design.md:223`) rises only to about 145 with the same unit size, because one 137 ms seek then dominates **[derived]**. To use the compaction the disc unit must grow (64 by 64 sectors, 3 to 50 KB compact) and the decoder must work on 16 by 16 sub-blocks, which costs +10 % bit-packed (+22 % range coded) for the Cathedral **[measured]**.
5. **RAM is a different problem.** Decoding into today's resident tables saves nothing resident. The cheap resident levers are 32 B quads with a room colour palette (-16 % of a Cathedral chunk together with leaf collision) and fully packed quads at measured widths (5 to 7 words instead of 10, -35 to -57 % of the quad table, cycle neutral on paper) **[derived/estimate]**. Real RAM relief needs two tiers: a compressed ring (about 7.5 to 12 B per cell, 10 to 16 times denser) around an expanded draw and collision disc. Decoding costs about 1,200 to 2,500 cycles per quad (about 2,100 to 4,900 per cell) in the range coder and 430 to 700 per quad (710 to 1,570 per cell) bit packed **[estimate]**, which at walking speed is 1.5 to 3.4 % of the CPU (0.5 to 1.2 % packed).
6. **Recommendation:** build the disc form as a bit-packed predict-and-correct stream (section 4), decoded at stream-in into the existing `PGRM` tables; ship the range coder only if disc size ever matters. Keep the resident form as designed (T10 leaves) plus the cheap 32 B quad, and treat fully packed quads and the two-tier ring as experiments with gates (section 8). Convert legacy levels to derived walls (-27 % raw, -31 % range coded on the Cathedral).

---

## 1. What was measured and how

`cargo run --release -p psxed-project --example grid_compact -- <mode> [project...]` (modes: `breakdown probe probe2 fields rquads portals tree codec codecv units baselines walls`). Levels: the six reference projects `editor/projects/grid-{cathedral,level008,dungeon,cave,sewers,west}`, the starter, and the free-plane demo (`examples/grid_planes_demo.rs`, planes sections only, the codec does not cover planes). `GRID_COMPACT_DERIVE=1` first converts every room to derived walls with `convert_to_derived(.., PreserveLook)` (`E/edit/convert.rs:151`), which is what new levels look like. `PSXED_GRID_LOD_FAR=6` cooks the distance LOD. Cooked blobs are cached in `/tmp/grid-compact-cache`.

The scales differ per level and decide every "ru" figure: Cathedral S = 36, sewers and west S = 56, starter S = 80, level008, dungeon and cave S = 84 **[measured, `X/probe.rs` output]**. The Cathedral chunk is 111,300 B here, not the 107,960 B of `docs/grid/level-format-and-runtime.md:624`, because the cook has changed (course cut walls, repeat quads).

The codec traversal (`X/codec.rs`) is written once for encoder and decoder; encoding returns its own reconstruction, which must equal the cooked tables, and every row below was also decoded from the produced bytes with the real range-coder and bit-packer decoders and compared (vertices, quads, cells, sectors, exts, walls, split-level, LOD blocks, material list). Any mismatch aborts the row. What it does not cover: free planes, liquids, movers and LOD quads (carried raw or refused), diagonal wall UVs, bands (none in the reference levels).

---

## 2. Standing design rule: as few bits as possible

Manny's rule, binding on every later phase of the compact format, the leaf format (T10) and streaming:

1. **Every field gets the width its real value range needs**, computed from the data, not from the type that happened to be convenient. A constant field costs zero bits.
2. **Widths that vary by room or unit are stored once in a header descriptor** (5 bits per field: the width; plus a signed base where the range does not start at zero) and applied to every record of that room. Heights are stored in fine units (64 grid units) relative to the room base: the reference levels all sit on the fine grid (0 of the 3,452 vertex heights and 0 sector corner heights off it, section 3.4), so the fine step is exact and a tall room pays for its own span only.
3. **Implicit beats stored.** Anything a decoder can rebuild from fields it already holds is not coded (running counts, prefix sums, plane constants, welded vertex ids, summary flags, repeat flags). The cook's round-trip gate (decode(encode(x)) == x on every cook) is what makes dropping a field safe.
4. **Flags are bitfields, enums are palette indices.** Flag words, materials, colours and normals go through a per-room palette whose index width is `ceil(log2(entries))`. No reserved bytes, no padding fields, no alignment holes in the disc form.
5. **The resident form may be wider only where a cycle count says so.** A field is kept at a hardware friendly width (GTE vertices at 8 B, collision heights at i16) only with a stated reason; every other resident field is packed. On the R3000 a RAM load costs 7 cycles (1 issue + 6 stall, `emu/crates/emulator-core/src/bus.rs:1820`), a store 2 (`bus.rs:1856`), an ALU op 1, so extracting a field from a word you already hold (`srl`+`andi`, 2 cycles; `sll`+`sra` for a signed field, 2; top or bottom fields 1) beats any separate load. Packing wins whenever it removes a load and loses when it only adds shifts to words that were loaded anyway.
6. **Every phase reports the field table.** A phase that adds or changes a record prints `grid_compact fields` (current bits, range, minimum, chosen, decode cost) for the reference levels and the per-level bits saved, as the PR evidence.
7. **Prefer one word fetch that unpacks several fields.** Order fields so those read together share a word; an unpack of k fields from one `lw` costs 7 + at most 2k cycles against 7k for k separate halfword loads.

### 2.1 Per-field table, Cathedral (every room record, one room)

"Current" is the record field width today, "range" the measured min..max in the room, "min" the bits that range needs, "chosen (disc)" the bits per value after the derive rule (a fraction means the field is stored only for the quads where the decoder cannot predict it), and the R3000 cost is the read side of the packing in the resident or install code. Source: `grid_compact fields`.

| Record.field | Current | Range (Cathedral) | Min | Chosen (disc) | How / R3000 cost |
|---|---|---|---|---|---|
| Sector.floor[4] (fine units) | 16 | -4..8 | 4 | 4, predicted from neighbours | install: 2 ALU + base add per corner |
| Sector.ceil[4] | 16 | 56..88 | 6 | 6, absent without a ceiling | as floor |
| Sector.flags | 16 | 0..693 | 3 | 3 (palette of 7), 5 bits derived | 1 table load + mask; WALK_A/B from corner heights, FLAT_* and CLEAR from `summarize_sectors` (`P/collision/summary.rs:14`) |
| Sector.ext | 16 | 0..173 | 8 | 0, running count | counter |
| SectorExt.wall_first | 16 | 0..339 | 9 | 0, prefix sum | add |
| SectorExt.wall_count | 8 | 1..8 | 3 | 3 | 2 ALU |
| SectorExt.plane_count, plane_first | 8, 16 | 0 | 0 | 0 | none without planes |
| SectorExt.splitb | 16 | 0..65535 | 3 | 0, running count | counter |
| ColWall.edge | 8 | 0..5 | 3 | 3 | 2 ALU |
| ColWall.flags, reserved | 8, 16 | 0 | 0 | 0 | none |
| ColWall.y[4] | 16 | -4..92 | 7 | 7, predicted (floor/ceiling, previous wall, neighbour run) | 2 ALU + base |
| SplitB.* | 16 | -4..4 | 4 | 4 | 2 ALU + base |
| Vtx.x, z | 16 | 0..132, 0..104 (click lattice) | 8, 7 | implicit (welded from rings) | resident: `mul` or shift by click |
| Vtx.y | 16 | -4..92 (fine) | 7 | implicit | table `fine_to_ru` |
| Vtx.pad | 16 | 0 | 0 | 0 | resident keeps it: GTE `LWC2` needs 8 B |
| Quad.v[4] | 16 | 0..1817 | 11 | implicit (welding) | hash/bucket probe at install, about 25 cycles per corner |
| Quad.color[4] | 32 | 292 distinct of 6,608 uses | 9 | 9, or implicit when (vertex, normal) was seen | palette load, 7 cycles |
| Quad.uv u, v | 8 | 0..63, 0..191 | 6, 8 | 0.42, 0.56 on average (93 % of quads predicted) | rebase: 2 compares, 2 ALU per corner |
| Quad.d | 16 | -936..1290 | 12 | 1.08 (only non-axis quads) | axis: 2 ALU; other: 3 `mult` (6 to 13 cycles each) |
| Quad.n[3] | 8 | -127..127 | 8 | 0.72 (axis class from the ring cross sign; others via table) | cross product, 6 `mult` |
| Quad.material | 8 | 0..5 | 3 | 3, move-to-front rank | 1 mask + 1 load |
| Quad.flags | 8 | 0..26 | 5 | 0 stored (TRI, AXIS, axis id, NEG derived; DOUBLE from the material, one bit extra) | ring length, cross signs |
| Quad.flags2 | 8 | 0..1 | 1 | 0, follows from the tile count | with the UV rebase |
| RCell.first | 16 | 0..1662 | 11 | 0, prefix sum | add |
| RCell.count | 8 | 0..28 | 5 | 5 (item terminator) | 2 ALU |
| RCell.flags | 8 | 0..1 | 1 | 1 | 1 ALU |
| RCell.min_y, max_y | 16 | -4..92 | 7 | 0, from the cell's vertices | min/max over the cell, 2 ALU per corner |

The widths above are fixed-width minima. The prototype codec goes further by modelling: a predicted value costs a fraction of a bit (section 5).

### 2.2 Bits saved by the rule, per level

Whole level, all rooms, record tables only (`grid_compact fields`; header, material list and padding excluded). "By range" gives every stored field its measured width. "Rule applied" also drops the implicit and derivable fields of 2.1 and adds the palettes the packing needs. The last two columns round each table's records up to whole bytes or whole 32 bit words.

| Level | Current B | By range B | Rule applied B (bit exact) | Bits saved by the rule | Records in bytes B | Records in words B |
|---|---|---|---|---|---|---|
| grid-cathedral | 110,636 | 49,781 (45.0 %) | 28,272 (25.6 %) | 658,912 | 30,437 (27.5 %) | 37,064 (33.5 %) |
| grid-level008 | 36,592 | 14,818 (40.5 %) | 8,815 (24.1 %) | 222,216 | 9,550 (26.1 %) | 12,346 (33.7 %) |
| grid-dungeon | 35,148 | 13,632 (38.8 %) | 7,634 (21.7 %) | 220,112 | 8,707 (24.8 %) | 12,348 (35.1 %) |
| grid-cave | 14,444 | 5,778 (40.0 %) | 3,742 (25.9 %) | 85,616 | 4,256 (29.5 %) | 5,636 (39.0 %) |
| grid-sewers | 10,372 | 4,243 (40.9 %) | 2,560 (24.7 %) | 62,496 | 2,802 (27.0 %) | 3,813 (36.8 %) |
| grid-west | 6,708 | 2,456 (36.6 %) | 1,379 (20.6 %) | 42,632 | 1,609 (24.0 %) | 2,018 (30.1 %) |
| grid-starter | 7,232 | 2,564 (35.5 %) | 1,191 (16.5 %) | 48,328 | 1,339 (18.5 %) | 1,871 (25.9 %) |

The ladder for the Cathedral is therefore 100 % (today) to 45 % (ranges only) to 25.6 % (implicit and derived fields) to 8.8 % (prediction, bit packed) to 5.0 % (prediction, range coded): width alone is worth a factor 2.2, derivation another 1.8, and prediction another 3 to 5.

### 2.3 Resident form under the rule

The resident tables are read every frame (render) or every sim tick (collision), so rule 5 applies. Measured widths per quad, packed into whole words, with the room palette cost included (`grid_compact rquads`):

| Level | Quads | Bits per quad (v / colour / uv / plane / material+flags) | Words | Quad table today -> packed |
|---|---|---|---|---|
| grid-cathedral | 1,662 | 44 / 36 / 56 / 19 / 12 = 167 | 7 | 66,480 -> 41,056 B (62 %) |
| grid-level008 | 530 | 36 / 32 / 56 / 22 / 11 = 157 | 6 | 21,200 -> 11,464 B (54 %) |
| grid-dungeon | 487 | 36 / 28 / 52 / 18 / 11 = 145 | 6 | 19,480 -> 10,732 B (55 %) |
| grid-cave | 208 | 28 / 32 / 52 / 18 / 11 = 141 | 7 | 8,320 -> 5,432 B (65 %) |
| grid-sewers | 165 | 32 / 28 / 48 / 18 / 12 = 138 | 6 | 6,600 -> 3,800 B (58 %) |
| grid-west | 91 | 28 / 24 / 48 / 16 / 13 = 129 | 6 | 3,640 -> 2,036 B (56 %) |
| grid-starter | 120 | 28 / 20 / 48 / 12 / 11 = 119 | 5 | 4,800 -> 2,048 B (43 %) |

(Bit counts shown are of the last room of each level.) Cycle accounting **[estimate]**: a 40 B quad is 10 word loads; a 24 to 28 B packed quad is 6 or 7, so 3 to 4 fewer loads (21 to 28 cycles) against roughly 25 to 35 ALU ops of unpacking for the fields the select and emit loops read. On paper that is cycle neutral with 35 to 57 % less RAM for the quad table. It changes the hottest loop of the renderer (`P/render/select.rs`, `emit.rs`), so it is an experiment with a gate (section 8.3), not a commitment. Fields kept wide with a reason: `Vtx` at 8 B (GTE `LWC2`, `P/format.rs:479`), `Sector` corners at i16 (the collision family costs 19.6k cycles per tick on the Cathedral, `docs/grid/level-tools-design.md:65`; any packing there must be measured against that number).

---

## 3. The current representation, measured

### 3.1 Bytes per section (Cathedral, one room 33 x 26, 858 sectors, 111,300 B)

| Section | Records | Bytes | Share |
|---|---|---|---|
| QUADS (40 B) | 1,662 | 66,480 | 59.7 % |
| SECTORS (20 B) | 858 | 17,160 | 15.4 % |
| VERTICES (8 B) | 1,818 | 14,544 | 13.1 % |
| CELLS (8 B) | 858 | 6,864 | 6.2 % |
| WALLS (12 B) | 341 | 4,092 | 3.7 % |
| SECTOR_EXTS (8 B) | 173 | 1,384 | 1.2 % |
| LOD_BLOCKS (8 B), SPLITBS, MATERIAL_USE, header | 63, 7, 6 | 504 + 112 + 6 + 152 | 0.7 % |

All sections, all levels: `X/report.rs` output. Chunks: level008 36,988 B (378 sectors, 530 quads), dungeon 18,472 + 17,192 B (two rooms, core 2,768 B with 72 portals), cave 9,536 + 5,316 B (core 368 B), sewers 10,580 B, west 6,908 B, starter 7,420 B **[measured]**.

### 3.2 Per visible feature

| Feature | Today | Compact (bit packed / range coded) |
|---|---|---|
| Per sector (Cathedral) | 129.7 B (111,300 / 858) | 11.4 B / 6.5 B |
| Per sector, all seven levels | 122 B (223,712 / 1,832) | 11.8 B / 7.5 B |
| Per quad, all in (collision, cells, vertices included) | 67.0 B (Cathedral) | 5.9 B / 3.4 B |
| Per quad, render tables only | 52.9 B (vertices + quads + cells) | |
| Per authored wall record, render + collision | 125 B (2.83 course quads x 40 B + 12 B) | 2.2 B for the record (765 B / 341, range coded), plus about 1 bit per course piece in item symbols |
| Per floor quad | 40 B quad + about 1.1 vertices (8 B) + 8 B cell + 20 B sector | about 3 bits of geometry (sector layer 2.3 bits per floor, item symbol about 1); colours dominate |

### 3.3 What is redundant, with counts (Cathedral unless noted)

Derivable at install:

* **Plane word of every quad** (`d`, `n`, `flags`, `flags2`, 8 B x 1,662 = 13,296 B, 11.9 %). 1,512 quads (91 %) are axis aligned, so `n` is a sign and `d` a coordinate (`E/cook/render.rs:371-383`). Axis share: level008 52 % (238 of 530 are triangles on ramps), dungeon 85 to 91 %, cave 48 %, starter 100 %. Non-axis normals need a table or an integer normalisation (open question 3).
* **UVs** (8 B x 1,662 = 13,296 B). Predicted exactly from vertex positions by the default world mapping at 32 texels per sector (`E/faces.rs:259-285` for floors, `:334-364` for walls) plus the cook's tile rebase (`E/cook/render.rs:180`): Cathedral 1,547 of 1,662 (93.1 %), level008 528 of 530, dungeon 487 of 487, cave 208 of 208, sewers 153 of 165, west 90 of 91, starter 120 of 120 **[measured]** (the codec's own UV misses agree: 115, 2, 0, 0, 12, 1, 0). The Cathedral misses are the 115 chamfer and arcade quads on diagonal edges (u runs along an edge of length 1024 x root 2, not an integer texel count).
* **Vertex table and vertex ids.** Vertices are welded in first-use order over the ring corners of the quads in cell order (`E/cook/render.rs:254`), so the 14,544 B table and the 13,296 B of `Quad.v` are implied by the quad rings.
* **Cell table** (6,864 B): `first` is a running sum in every room (checked, true for all 9 rooms), `min_y/max_y` are the extremes of the cell's vertices, the count is the item terminator.
* **Extension, wall and split-level indexing**, the walkable bits, the three summary flag bits, `MATERIAL_USE`, `LOD_BLOCKS` (derived from the cell table, `E/cook/lod.rs:200-226`).
* **LOD quads.** With `PSXED_GRID_LOD_FAR=6` the Cathedral gains 201 LOD quads, +8,040 B (+7.2 %); level008 +1,600 B. Stored raw they would exceed the whole compact chunk (5,591 B), so they must be regenerated at install by the cook's merge rule, or not shipped.

Duplicated across sections:

* **Render wall quads versus collision walls.** 964 of the Cathedral's 1,662 quads (58 %, 38,560 B) are wall course pieces of 341 solid wall records that also sit in `WALLS` (4,092 B). 956 of the 964 are generated from the collision records by the codec; 8 need an explicit ring. Course pieces are cut at absolute multiples of one sector in height (`E/faces.rs:457`), so a 5.5 sector wall is seven quads with predictable edges.
* **Floor and ceiling quads versus sector corners**: 660 floor quads, 32 triangles (16 split pairs) and 6 ceiling quads of the Cathedral are all generated from the sector records, none stored.
* **Free-plane collision copies.** The planes demo holds 50 `ColPlane` copies (1,400 B, 8.8 % of its chunk) of 7 planes, one copy per overlapped sector (`P/format.rs:380-396` and `level-format-and-runtime.md:685`).
* **Portals are stored in both directions** (36 B each). Dungeon: 72 portals, 2,592 B, 30 wall portals (all axis aligned rectangles) and 42 floor or ceiling openings (horizontal quads); one record per pair at 52 bits is 252 B (10.3x less) **[bit width from the measured ranges, total derived]**. Cave: 216 B to 18 B. For the compact dungeon (2,244 B of chunks) the 2,768 B core is bigger than all chunks, so portals are not optional to compact.

Over wide or padding **[measured]**: `Vtx.pad` 3,636 B (3.3 %); the unused top byte of every colour word 6,648 B (6.0 %); `ColWall.flags` and `reserved` 1,023 B; `SectorExt` plane fields 519 B; `Quad.flags2` is 1 bit in an 8 bit field.

### 3.4 Heights, colours and the lattice

* Every vertex and sector corner height of all seven levels is `rt(64 k + y_offset)` for an integer `k` (0 of 3,452 vertex heights off the fine grid, 0 sector corners). The fine step is 5.25 ru at S = 84, 2.25 at S = 36, 5 at S = 80, so the mapping is injective and exact in both directions. Every vertex x and z is a multiple of the click (S/4) (0 off).
* Colour is a pure function of (vertex id, quantised normal): in all rooms, 0 keys with more than one colour. Distinct colours per room 32 (starter) to 292 (Cathedral); keys 171 to 2,626; uses per key 1.28 to 2.64. Only 59 of 1,662 Cathedral quads have four equal corner colours. Light does not fit 555 losslessly: 6,642 of the Cathedral's 6,648 corner slots use the low three bits **[measured]**.

---

## 4. The design

Three forms with three jobs. The disc form is as dense as decode time allows. The install step expands it, once, when a unit lands. The resident form is what the renderer and collision read, and changes only where a cycle count allows.

```
 DISC  PGC chunk per decode unit  ->  INSTALL (sliced decode, scratch)  ->  RESIDENT  PGRM tables (or T10 leaves)
 bit packed predict-and-correct       welding buckets, colour map,          zero-copy, as designed; optional
 stream, per-room width descriptor    derive plane/uv/cells/exts             packed quads; collision first
```

### 4.1 The disc form: predict and correct

A chunk is a header plus two layers coded in z-major cell order inside the unit. The prototype (`X/codec.rs`) is the normative description until a spec replaces it.

**Header.** y offset in fine units (signed), two flags (repeat enabled, course cut walls), the distance LOD fields, min/max y, the sector flag palette (entries x 16 bits), derived wall height. The width descriptors of rule 2 go here. 7 to 21 B in the prototype.

**Collision layer, per sector.**
* flag word: palette index, coded against the previous index.
* floor and ceiling corners in fine units, each predicted from already decoded neighbours: NW from the left sector's NE, else the upper sector's SW, else the upper-left's SE; NE from the upper SE or upper-right SW; SW from the left SE; SE by planar continuation (NE + SW - NW). Mode: exact (1 symbol), flat (one residual), general (four residuals). A ceiling with no decoded neighbour is predicted from its own floor plus the last headroom.
* split-level corners as residuals from the surface they override.
* wall count; per wall: edge (3 bits), flags, four heights as residuals against the best of four predictors (floor/ceiling at the edge's ends, continuation of the previous wall in this sector, the wall at the same ordinal in the previous cell along the run, a copy of the previous wall).
* Install derives: `SectorExt` and `SplitB` tables (an extension exists exactly when a sector has walls or a split level, `E/cook/collision.rs:257-294`), the two walkable bits (`triangle_normal_q12`, normal y >= 2867), the summary bits.

**Render layer, per cell.** A list of items, each a symbol (4 bits through an adaptive tree, context the previous symbol):

| Symbol | Meaning | Cost when it fits |
|---|---|---|
| FLOOR_Q, FLOOR_T0, FLOOR_T1, CEIL_Q, CEIL_T0, CEIL_T1 | the floor or ceiling of this sector, ring built from the decoded corners | the symbol and a 3 bit orientation variant |
| WALL | next course piece of the next unconsumed collision wall (rectangular walls in rooms with courses are cut at multiples of one sector, `E/faces.rs:457`) | symbol and variant |
| DER | next derived wall slot (riser, drop, closure) in cook order, computed from neighbour profiles (`E/derive.rs:219`) | symbol and variant |
| SKIP, DSKIP | step over a wall or slot that is not drawn | symbol |
| ESC | explicit ring: corner deltas | about 4 B |
| END | cell done | symbol |

The variant (8 values: rotation x reversal) makes the prediction independent of the cook's winding convention, so the format does not break when `faces.rs` changes how it winds. Then, per item: material as a move-to-front rank, one bit for an extra DOUBLE, the UV mode (explicit, floor mapping, wall mapping with either sign; the mapping is the integer world mapping with the room's origin and y phase), the normal when the quad is not axis aligned (index into a per-room table), and a colour per corner whose (vertex, normal) key is new.

**Colour.** A corner already seen under the same (vertex, normal) costs nothing. A new one is coded as: equal to a predicted colour (previous corner, or the nearest decoded corner with the same normal, `PaletteNear`), a rank in a 16 entry recently-used list, an index in the room palette, or a delta of the three channels from the prediction. Measured against raw 24 bit and against delta only (section 5.3).

**Robustness is the point of ESC.** The encoder verifies every item against the prediction; a quad the predictors cannot explain is an ESC and costs bytes, never correctness. When the cook later learns diagonal risers, jitter or bands, new geometry first appears as ESC and the cook report shows the explicit share, which is the signal to teach the predictor. The round-trip gate is the guard against silent drift.

### 4.2 What the quadtree contributes, measured

* **On disc: nothing beyond prediction.** Sector layer: 193 B for the Cathedral's 858 sectors (1.8 bits per sector), 225 B for level008's 378 (4.8 bits), 24 B for west's 64 (3 bits). A quadtree leaf flag per 4 by 4 block would not do better where the predictor already says "exact" for 1 bit per sector or less, and it cannot express a slope as cheaply as planar continuation does. Planar nodes stored once with children implicit, corner sharing on the lattice, heights as bit-packed deltas: all three are what the predictor does without a tree.
* **Resident collision: modest here.** World-aligned exact merge (identical flag word, no extension, one plane through every corner of floor and ceiling), `X/tree.rs`:

| Level | Sectors | Leaves (4x4 / 2x2 / 1x1) | Dense 20 B/sector -> leaves + 2 B index |
|---|---|---|---|
| cathedral | 858 | 477 (10 / 77 / 390) | 17,160 -> 11,256 (-34 %) |
| level008 | 378 | 288 (2 / 20 / 266) | 7,560 -> 6,516 (-14 %) |
| dungeon r0, r1 | 133, 170 | 115, 134 | 2,660 -> 2,566 (-4 %), 3,400 -> 3,020 (-11 %) |
| cave r0, r1 | 91, 48 | 67, 42 | 1,820 -> 1,522 (-16 %), 960 -> 936 (-3 %) |
| sewers | 54 | 51 | 1,080 -> 1,128 (+4 %) |
| west | 64 | 46 | 1,280 -> 1,048 (-18 %) |
| starter | 36 | 33 | 720 -> 732 (+2 %) |

  Room-aligned rather than world-aligned blocks change little (a handful of leaves). The rooms are small and hand edited; a 4 x 4 planar block with no walls is rare. The estimate in `level-tools-design.md:760` (128 by 128 flat room: 327,680 B dense to 53,248 B) is arithmetic about a room none of the references resembles; the study cannot confirm or refute it.
* **Quad merge for rendering: nothing at zero tolerance.** A 2 x 2 block of floor quads merges only when material, coplanarity, default UV and the baked light (bilinear from the four outer corners) all agree. At tolerance 0 of 255: 0 blocks in every level. Counting aligned 2 x 2 blocks of floor quads that could become one quad: at tolerance 4, 5 blocks (Cathedral, 15 quads removed of 1,662) and 4 (level008); at 8, 29 and 13; at 16, 56 and 23 (Cathedral 168 quads removed, 10 % of its quads; level008 69, 13 %); no 4 x 4 block at any tolerance. This is the evidence behind the T14 risk "merge disappoints" (`level-tools-design.md:849`): lit levels do not merge; unlit or ambient-only areas would.
* **Where it should pay**: as the independent decode block (see 4.4), as the unit of culling and LOD (T15, section 10.4 there), and for large flat terrain. Keep the T8 to T10 plan for those reasons.

### 4.3 Walls: authored versus derived

Derived-wall rooms are smaller in every form: the Cathedral converted with `PreserveLook` is 80,932 B raw (-27 %), 6,917 B packed, 3,867 B range coded (-31 % against 5,591 B). Seven reference levels together: raw 182,216 B, packed 17,989 B, range coded 11,426 B **[measured, `GRID_COMPACT_DERIVE=1`]**. In derived rooms wall quads are not a course of 7 pieces per record, and the closure walls are the only collision records. A cheaper collision wall list is still open: the closure walls of derived rooms are not yet generated from the DER slots, so they still pay count, edge and predictor mode (the height predictor "floor and ceiling at the edge's ends" already makes their heights free). Not built, not measured.

### 4.4 Units and independent decode

Prediction crosses cell borders (left and upper neighbours, colour neighbours, adaptive models). For streaming a unit must decode alone. `grid_compact units` re-runs the codec cutting the room into blocks of 16, 8 or 4 sectors a side, with neighbours, models, palettes and welding cut at every border and 4 B of coder flush plus a 4 B directory entry per block:

| Level | Whole room | Block 16 | Block 8 | Block 4 |
|---|---|---|---|---|
| cathedral, range coded | 5,591 | 6,839 (+22 %) | 8,567 (+53 %) | 11,426 (+104 %) |
| cathedral, bit packed | 9,779 | 10,716 (+10 %) | 11,879 (+21 %) | 13,754 (+41 %) |
| level008, range coded / packed | 2,490 / 4,015 | 2,943 (+18 %) / 4,314 (+7 %) | 3,741 / 4,855 | 5,000 / 5,691 |
| dungeon, rc / packed | 2,244 / 3,260 | 2,430 / 3,377 | 3,018 / 3,775 | 4,050 / 4,513 |
| cave, rc / packed | 1,631 / 2,099 | 1,647 / 2,107 | 1,734 / 2,130 | 2,319 / 2,524 |

The bit packer pays only for the lost neighbours; the range coder also relearns its models at every unit (adaptive models are the expensive part of independence). With 16 sector blocks the range coder's advantage over packing falls from 1.75x to 1.57x on the Cathedral. A shared prior for the models, or a level-wide palette in the core, would recover part of it (not measured). The measurement orders cells block by block; its reconstruction is not compared (size only).

---

## 5. Prototype results

All rows decoded from the produced bytes and compared table by table [measured]. "RC" is the adaptive range coder (LZMA style, 11 bit probabilities, shift 4 or 5, best of each reported), "packed" the same traversal with every binary decision costing exactly one bit and no adaptation.

### 5.1 Sizes

| Level | Raw chunk B | Packed B | RC B | Packed % | RC % | Deflate-9 on raw | LZ4-style on raw |
|---|---|---|---|---|---|---|---|
| cathedral | 111,300 | 9,779 | 5,591 | 8.8 | 5.0 | 34,543 (31.0 %) | 56,533 (50.8 %) |
| level008 | 36,988 | 4,054 | 2,490 | 11.0 | 6.7 | 12,419 (33.6 %) | 20,499 (55.4 %) |
| dungeon (2 rooms) | 35,664 | 3,298 | 2,244 | 9.2 | 6.3 | 11,200 (31.4 %) | 19,472 (54.6 %) |
| cave (2 rooms) | 14,852 | 2,186 | 1,631 | 14.7 | 11.0 | 5,868 (39.5 %) | 9,176 (61.8 %) |
| sewers | 10,580 | 1,193 | 894 | 11.3 | 8.4 | 3,753 (35.5 %) | 6,361 (60.1 %) |
| west | 6,908 | 620 | 460 | 9.0 | 6.7 | 2,180 (31.6 %) | 3,744 (54.2 %) |
| starter | 7,420 | 517 | 344 | 7.0 | 4.6 | 2,198 (29.6 %) | 4,054 (54.6 %) |
| total | 223,712 | 21,647 (9.7 %) | 13,654 (6.1 %) | | | 72,161 (32.3 %) | 119,839 (53.6 %) |

Deflate applied to the range coder output adds 11 B on the Cathedral (5,602): the stream has no redundancy left for a general compressor. The generic compressors lose by a factor 3 to 10 because they cannot see that a floor quad follows from a sector or that a wall course follows from a wall record; they only find literal repeats.

Derived-wall variants (every room converted): packed 17,989 B, RC 11,426 B, raw 182,216 B (section 4.3). Cores are separate: dungeon 2,768 B, cave 368 B, others under 200 B.

### 5.2 Where the bytes go (Cathedral, RC; level008 in brackets)

| Part | Bytes | Share |
|---|---|---|
| colour | 2,878 (1,463) | 51.5 % (58.9 %) |
| UV (115 explicit quads; level008 2) | 995 (34) | 17.8 % (1.4 %) |
| walls (collision records) | 765 (177) | 13.7 % (7.1 %) |
| item symbols | 325 (149) | 5.8 % (6.0 %) |
| material | 277 (48) | 5.0 % (2.0 %) |
| sectors | 193 (225) | 3.5 % (9.1 %) |
| normals (150 stored; level008 256) | 100 (374) | 1.8 % (15.1 %) |
| explicit rings (8) | 31 (0) | 0.6 % |
| header | 21 (14) | 0.4 % |

Item coverage, Cathedral: 660 floor quads, 32 floor triangles, 6 ceiling quads, 956 collision wall pieces, 8 explicit rings. Level008: 213 floor quads, 238 triangles, 79 wall pieces, 0 explicit.

### 5.3 The colour experiments

* Raw 24 bit for new colours: Cathedral 5,816 B; as a delta from the previous corner 5,609 B; from the nearest same-normal neighbour 5,591 B. Delta-only (no palette/recent list) 8,069 B. The palette matters, the neighbour prediction hardly.
* Quantising light before coding (round trip against the quantised source): 6 bit 4,800 B, 5 bit 4,400 B, 4 bit 3,994 B for the Cathedral; all levels 5 bit 10,625 B (-22 % of 13,654), 4 bit 9,494 B (-30 %). Banding on dark gradients is not evaluated; a frame comparison would be required.
* Re-lighting at install is rejected: the bake traces shadow rays through the collision world per corner and per light (`E/light.rs:158-205`, f64), 2,626 keys in the Cathedral alone, with no floating point on the device.

### 5.4 Decode work and cycles **[estimate]**

Counted from the decoder: binary decisions and raw bits, Cathedral 57,367 and 20,864 for the whole chunk. Cost model, every constant an assumption: 30 cycles per range-coder decision (about 20 instructions, one prob load at 7, one `mult` at 6 to 9), 8 per raw bit group, 25 per welded corner (bucket probe), 120 per quad (cross product, UV rebase, flags), 40 per cell, 50 per sector (walk bits, summary).

| Room | Cycles (range coder) | Per quad | Per cell | Frames of 1,128,960 |
|---|---|---|---|---|
| cathedral | 2,329,782 | 1,402 | 2,715 | 2.06 |
| level008 | 927,730 | 1,750 | 2,454 | 0.82 |
| dungeon r0 | 447,662 | 1,652 | 3,366 | 0.40 |
| dungeon r1 | 358,058 | 1,658 | 2,106 | 0.32 |
| cave r0, r1 | 285,542, 182,650 | 2,131, 2,468 | 3,138, 3,805 | 0.25, 0.16 |
| sewers | 266,588 | 1,616 | 4,937 | 0.24 |
| west | 148,924 | 1,637 | 2,327 | 0.13 |
| starter | 140,670 | 1,172 | 3,908 | 0.12 |

With the bit packer a decision is a shift and mask from a bit buffer, about 6 cycles: the Cathedral's entropy decode falls from about 1.9M to about 0.37M cycles and the room to 813,880 cycles (490 per quad, 949 per cell, 0.72 frames), with the 0.44M fixed part (welding, derivation, cells) unchanged. Packed, all rooms: 430 to 698 per quad, 713 to 1,573 per cell **[estimate]**. The range coder is therefore the choice only if bytes matter more than CPU. The measurement that replaces these numbers is a no_std port of the decoder into `psx-grid` run headless on the Cathedral (gate in section 9).

Scratch at install **[estimate]**: welding by sector-lattice buckets (918 lattice points of the Cathedral x 2 B head + 2 B chain per vertex, about 5.4 KB), the colour map (about 2,626 entries x 8 B, 21 KB), a normal table and a 16 entry recent list. It runs between frames, so it can borrow the renderer scratch (27,748 B for the Cathedral, `level-format-and-runtime.md:630`).

---

## 6. Candidates evaluated

| Idea | Verdict | Evidence |
|---|---|---|
| Planar quadtree nodes, children implicit, on disc | Not needed on disc; keep as the unit hierarchy | Prediction already costs 1.8 bits per sector (4.2) |
| Heights as click or fine deltas, bit packed, one value per lattice corner | Done, inside the prediction | 4 to 7 bit ranges; sector layer 193 B (Cathedral) |
| Materials and UV modes as per-room palettes and inheritance | Done: material rank 277 B; UV is predicted, not inherited | 93 to 100 % of quads need no UV bits |
| Derived walls regenerated at load | Done for course pieces (from collision records) and slots (DER); strongest when rooms are derived | 956 of 964 Cathedral wall quads; converted rooms -27 % raw |
| Light per lattice corner or 555 plus palette | Per (vertex, normal) with a palette: done. 555 lossless: impossible. 555 lossy: -22 % | 4.3, 5.3 |
| Collision derived from compact heights | Yes, and the reverse: render walls derived from collision walls and floors from sector corners | 4.1 |
| Render vertices generated per frame from the lattice | Rejected for now | Per-frame decode of 1.2k to 2.5k cycles per quad against 800 to render it; an unshared corner loses the 3.65 uses per vertex of the stamp table (`level-format-and-runtime.md:392`) |
| Bands as per-run anchors | Out of scope (none in the references); the item list absorbs them as DER slots once `faces.rs` bands are predicted | none |
| Split/void/diagonal flags as bits | Done for split level and diagonal edges (3 bits edge id); void triangle (natural-terrain.md N1) needs a flag bit per sector | `X/codec.rs` |
| Morton order | Used as block order only; z-major inside a block is what the predictors need | 4.4 |
| Generic compressor over the raw tables | Rejected | 31 % (deflate), 51 % (LZ4-style) against 5 to 9 % |
| Colour-aware floor quad merge | Rejected for lit levels | 4.2 |
| Relighting at install | Rejected | 5.3 |
| Zero-copy of the compact form | Rejected: prediction needs a sequential decode | 4.1 |
| Range coder instead of bit packing | Optional squeeze | -43 % bytes at about 5x the entropy-decode cycles; irrelevant for bandwidth (section 7) |

---

## 7. Streaming consequences **[derived from measured bytes per cell and the drive numbers of streaming-design.md:46-52]**

Per cell, all seven levels: raw 122 B, bit packed 11.8 B, range coded 7.5 B. A 16 x 16 sector unit (256 cells):

| | Raw | Packed | RC |
|---|---|---|---|
| Unit bytes | 31 KB | 3.0 KB | 1.9 KB |
| Sectors to read | 16 (104 ms) | 2 (13 ms) | 1 (6.5 ms) |
| With one 137 ms seek | 241 ms | 150 ms | 143 ms |

The streaming doc's limit is `rho x v_max <= U x B_eff`, U = 0.5. At 16 sector units that is 531 cells per second raw, about 90 cells per sector at 5.9 sectors/s (the doc's 92, from bytes, `streaming-design.md:223`). Compact units are seek bound: 256 cells per 0.15 s gives 853 cells per second, 145 per sector (+57 %), not the 12 to 19 times the bytes would suggest. Chaining units that sit together on disc and a larger disc unit fix it: a 64 x 64 sector unit (4,096 cells) is about 48 KB packed (24 sectors, 156 ms plus one seek, 0.29 s), 7,000 cells per second, about 1,190 cells per sector at U = 0.5. Its resident size at today's 122 B per cell would be 500 KB, which does not fit, hence the split: disc unit 64 x 64, decode block 16 x 16 (+10 % bytes packed), install on demand.

Decode load at walking speed (4.7 sectors/s, `streaming-design.md:80`): a frontier of 40 cells per sector (a 40 sector wide draw disc) is 190 cells/s, times 2.7k cycles per cell = 0.51M cycles/s, 1.5 % of the CPU (0.5 % bit packed at about 950 per cell); an open frontier of 90 cells is 420 cells/s, 1.1M cycles/s, 3.4 % (1.2 % packed) **[estimate]**. The PIO read tax stays what it is (1.286 ms per sector, `streaming-design.md:49`) but is paid for 12 times fewer sectors.

Two-tier residency, as arithmetic on measured bytes per cell (cell density 1 per sector area; real fill is lower, the Cathedral has 676 floors in 858 cells): an expanded disc of radius 24 sectors is 1,810 cells x 122 B = 221 KB (168 KB at the 93 B per cell of the packed-quad plus leaf resident form, Cathedral: 111,300 - 25,424 - 5,904 = 79,972 B over 858 sectors); a compressed ring out to radius 64 adds 11,058 cells x 11.8 B = 130 KB. The same radius 64 all expanded is 1.57 MB. The compressed tier has no seek to hide: expansion is CPU only, 0.4 to 1.1M cycles/s while moving. This is the only configuration in which the format reduces the RAM wall rather than the disc time. It needs collision for entities outside the expanded disc to stay "solid box" as the streaming design already specifies (`streaming-design.md:265-272`).

---

## 8. Recommended forms

### 8.1 On disc
Bit-packed predict-and-correct chunks (`PGC1`), one per decode block, grouped in disc units of up to 64 x 64 sectors, `layout_hash` of the resident format they expand to, width descriptor in the header, no reserved or padding bytes. The range coder backend behind a flag, decided later by disc size. Portals stored once per pair. Planes stored once per plane with the cell range derived. LOD quads regenerated at install. Expected: 8.8 % (Cathedral) to 14.7 % (cave) of today's chunk, bit packed; 5.0 to 11.0 % range coded.

### 8.2 Streaming-in decode
Sliced across frames, collision layer first (the unit is collidable before the render layer lands), scratch borrowed from the renderer's per-room scratch, output written straight into the pool pages so no staging copy exists, then the existing view check. Cost estimate 0.05 to 2.3M cycles per room (0.8M for the Cathedral packed, 2.3M range coded; section 5.4), per frame budget under 10 % so one Cathedral takes about 20 frames; a 16 x 16 block (256 cells, about 500 quads) takes about 0.7M cycles range coded (0.24M packed), 2 frames at 30 % (1 frame).

### 8.3 Resident
Phase 1: unchanged `PGRM` tables, T10 leaf collision, and 32 B quads with a room colour palette (lossless; +1 palette load per corner, about 28 cycles per visible quad or 3.5 % of the 800 cycle packet, **[estimate]**): -16.2 % on the Cathedral (111,300 -> 93,268 B), -4.8 to -11.9 % on the others (`grid_compact tree`). Phase 2, experiment: packed quads at measured widths (2.3), vertex pad reuse if the GTE ignores the upper half of `VZ` (hypothesis, `level-format-and-runtime.md:325`), the two-tier ring. Gates: world cycles and collision cycles on the Cathedral route tape within 2 % of the unpacked layout; any larger delta is reported, not accepted silently.

---

## 9. Phases

| Phase | Work | Gate |
|---|---|---|
| T8 (leaf storage) | none for this study; keep fine-unit heights in the document | unchanged |
| T9, T10 | leaf collision as designed (`level-tools-design.md:840-841`); in the same change make the cook compute `n` by an integer normalisation (open question 3) and drop `reserved` bytes from the new records (rule 4) | leaf lookup equals dense; `grid_compact fields` printed in the PR |
| C1 (new, after T7, before streaming) | `psx-grid::compact`: no_std decoder, bit-packed backend, PGC1 writer in the cook with the round-trip gate on every cook; `grid_compact` stays the measurement tool | decode(encode(x)) bit-equal on the seven references and the derived variants; headless frames of a decoded level equal the cooked level frame for frame; cycles per cell measured on the Cathedral to replace section 5.4 |
| C2 | installer in `GridRuntime` (decode into pool pages, collision first), compact chunks in `WORLD.PAK`, the unit planner budgets with compact bytes for disc and resident bytes for the pool | Cathedral plays from a compact chunk, route tape unchanged |
| C3 | portals once per pair, planes once per plane, LOD quads by merge at install | dungeon core 2,768 B -> about 300 B; planes demo chunk smaller |
| C4 (streaming, T16) | disc units 64 x 64, decode blocks 16 x 16, optional compressed ring | streaming simulator (`psx-residency-sim`) with the compact byte sizes; no deadline miss at the open-field frontier of 90 cells |
| C5 (optional) | range coder backend; shared model priors; packed resident quads | disc size or RAM wall only |

T14 (cook merge) should be re-scoped by section 4.2: expect gains from unlit or ambient areas, not from lit floors. T15 node LOD keeps its place.

---

## 10. Risks

* **Predictor drift.** Natural-terrain features (diagonal risers, void triangles, jitter, `natural-terrain.md` N1, N2) and bands change the geometry. They show up first as ESC items, not errors; the cook report must print the explicit share and the gate must fail above a set share (a proposal: 5 % of quads) so the predictors are kept current. Jitter (N2) moves wall vertices off the lattice by up to A_max (4 ru proposed): positions off the click lattice cost explicit deltas, so a fully jittered rock level would see walls fall back to ESC; the jitter field is a pure hash of position and could be regenerated (an idea, not measured).
* **Decode cost is an estimate.** 1,400 to 2,500 cycles per quad could be off by 2x either way. If the real figure is high, install slices get longer, not the frame.
* **Independence overhead.** +22 % (range coded) and +10 % (packed) for the Cathedral at 16 sector blocks; larger for the smallest blocks.
* **Scratch RAM at install** (about 27 KB, 5.4) in a RAM-tight guest (`cortex-guest-heap-headroom`, `ram-floor-dropped` notes: the 16 KB floor is gone, a stack and heap safety margin remains).
* **Light stays the floor.** Half the bytes are the baked colour field; only a lossy quantisation moves it and that is a visual decision.
* **Dungeon and cave cores.** Portal records in the core are not part of chunk numbers; for multi-room levels the core is as large as the compact chunks.
* **Two forms to keep in sync.** The PGC decoder, the resident format and the cook's face code must agree; the round-trip gate and `layout_hash` are the controls.

---

## 11. Open questions for Manny, with recommendations

1. **Bit packing or range coding on disc?** Bit packed is 1.75x larger, decodes about 5x faster in the entropy part, and its 9.8 KB Cathedral is already 8.8 % of today's. Recommendation: bit packed first; keep the range coder as an option.
2. **Lossless or lossy light?** 5 bit light saves 22 % of the compact bytes and the cook gets a visual risk. Recommendation: lossless; revisit only if disc space matters, with a frame comparison first.
3. **Integer normals in the cook** (define the 8 bit normal from the ring by an integer algorithm shared by cook and decoder) so non-axis quads store no normal: level008 would drop 374 B of 2,490 (15 %). Small change in `E/cook/render.rs:371`; recommend yes, with the leaf format change.
4. **Decode block and disc unit sizes.** Recommendation: decode blocks of 16 x 16 sectors, disc units up to 64 x 64, ask the streaming simulator before fixing.
5. **Packed resident quads** (24 to 28 B, section 2.3). Recommendation: do the 32 B palette quad now, run the packed quad as an emulator experiment before touching the renderer.
6. **Compressed RAM ring.** Recommendation: do not build until the pool size of the grid guest is measured (`streaming-design.md:63` still lists it as unknown).
7. **Portals and planes in the core and chunk**: store pairs and plane originals once. Recommendation: yes, in C3.
8. **Legacy levels.** Converting to derived walls saves 27 % raw and 31 % compact on the Cathedral and is look preserving by construction. Recommendation: convert the reference levels when T1's Convert command is trusted (the standing rule in `level-tools-design.md` section 4.7 says never behind the author's back).
9. **Wall course cutting** (legacy) exists only for byte identical old output (`level-tools-design.md:651`). Recommendation: keep it for the reference levels; nothing new depends on it.

---

## Appendix: reproducing

```
cargo run --release -p psxed-project --example grid_compact -- all          # everything below
... -- breakdown | probe | probe2 | fields | rquads | portals | tree | codec | codecv | units | baselines
GRID_COMPACT_DERIVE=1 ... -- codec        # derived-wall variants
PSXED_GRID_LOD_FAR=6 ... -- breakdown     # with the distance LOD
```

Files: `X/main.rs` (modes), `load.rs` (full Play cook and cache), `report.rs` and `probe.rs` (sections, redundancy), `fields.rs` (the rule tables), `tree.rs` (leaves, merge), `bits.rs` (range coder and bit packer), `codec.rs` (the traversal), `codec_report.rs`, `baselines.rs`. The tool is example code, not shipped.
