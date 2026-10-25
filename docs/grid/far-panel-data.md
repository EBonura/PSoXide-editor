# Far panel data on the disc (block format 3), and what the level load reads

The Citadel's far view is 149 bake eyes of 8 headings each, every panel cut into 6 tiles of 64 by 63 4-bit texels and a
palette row (one 2,048 byte atlas slot a tile). Before this format they were 12.0 MB of the 13.9 MB `WORLD.PAK`, read as
a list sector, one header sector per panel and one sector per tile, a seek for nearly every request. This is the format
that replaces them, how the guest reads it, and the other things the level open stopped reading.

## What is on the disc

One block per eye, in `WORLD.PAK` chunk `2 * rooms + 2 + eye` (code: `psx_grid::eye_block`, `psx_grid::tile_lz`).

| bytes | content |
|---|---|
| `HEADINGS * COLS` (16) | per heading and tile column, the mask of stored rows |
| 86 each | per present heading in heading order: the depth mesh (50 B), then the room of each cell (36 B) |
| 2 each | per stored tile in stream order: its length in bytes |
| (to a multiple of 4) | the tiles, one after another |

The stream order is the central rows of every present panel (all rows today), heading by heading and column by column.
The block is cut into groups of 6 sectors, the unit the guest keeps in a stage of its window: no tile crosses from one
group into the next (the stream skips to the next group instead, about 3 percent padding). A block is padded to whole
sectors.

A tile is an LZ4-style block (token, literals, 16-bit offset, match length; the last sequence has only literals); a tile
that does not shrink is stored whole, which is how the decoder tells (length 2,048). The cook picks the smallest block
with an optimal parse over a window of the whole tile. Fields are sized to what they hold except the tile length (16
bits, 12 would do: 2 KB over the whole disc, not worth a second unpack) and the mesh depths (8 bits, as the draw reads
them).

| | before | after |
|---|---:|---:|
| eye blocks | 6,012 sectors (12.0 MB) | 1,640 sectors (3.3 MB) |
| tiles | 4,712 x 2,048 B = 9.65 MB | 3.02 MB (mean 642 B) |
| headers and lists | 1,300 sectors, 336 KB of them used | in the index |
| `WORLD.PAK` | 6,764 sectors | 2,392 sectors |
| disc | 31,907,232 B | 21.6 MB |

Identical tiles are rare (67 of 4,712), so there is no sharing between blocks. A verification tool written for this
change (std only, not in the repository) reads both discs with its own reader and decoder: all 149 blocks, 4,712 tiles,
depth meshes, room cells and the 93 room chunks of the new disc equal the old disc's (0 differences).

## How the guest reads it (`psx_game_runtime::panel_stream`)

* The window the reads land in is two stages of one group each (24 KiB, was 33.5 KiB).
* The eye in use reads its index and everything after it: both stages when free, which for the average eye (10 sectors)
  is all its tiles for one seek. The index landing forces a pass, so the tiles are decided in the same tick; a landed
  group stays only until that tick's pass has taken the tiles it holds.
* An eye kept ready (the one the camera is about to cross to, and the one it walks towards) reads its index alone (one
  sector) and then only the sectors the tiles in view lie in. It brings the tiles the screen will show on arriving; the
  other tiles of it follow once the camera has crossed (the eye left stands in). An eye whose tiles are still in VRAM
  from an earlier visit reads its index alone again.
* A tile is found by adding the lengths before it, and decoded straight into the GPU upload nodes
  (`write_tile_uploads`, up to 6 a frame, 3 when the frame's list has no room). The decoder copies whole words with
  `lwl`/`lwr`, so its buffer has 4 spare bytes (`decode_tile_slack`).
* A read the eye in use waits for (its index, a tile of its view) goes ahead in the transport's queue, and takes a stage
  from tiles that are for another eye: landed ones give it up (asked for again later), reads still on their way are
  cancelled.
* `start_reads` costs a test when nothing changed (`reads_dirty`); it scanned the 130 slots every tick.

Why this matters on the console: reading a sector costs the CPU about 44,000 cycles (the transport copies it a byte at a
time, 1.3 ms at double speed), more than decoding a tile (about 30,000), so a tile of 642 bytes costs less than the 1.3
sectors it took raw, and sectors that are not read cost nothing.

## What the level load stopped reading

* The cube sky atlas (192 KiB, 97 sectors, about a second) is not read when panels or cards take its VRAM: the guest
  reserves the pages and the CLUT (`load_streamed_sky_from_cd`, `cube_atlas_is_overwritten`).
* `UI.PAK` is in load order: the sounds, the menu images, what the loading screen keeps, then what each gameplay entry
  loads, models in the order the loader asks for them (`ui_pack_chunks`). Reads that used to seek back and forth now
  start where the last ended.
* The pack ends with what the start reads (`pack_order`, `PackStart`): everything within the drawn radius plus 10 sectors
  of the player's start, and last the rooms the streamer asks for first (in the order it asks) and the nearest eyes, next
  to `UI.PAK`, which the load reads from as well. The core and the records go last of all, found from the manifest
  (`WORLD_PACK_FIRST_CHUNKS`) instead of a read of the pack's header.

## Measuring

The tapes are poll-bound: a build that opens the level sooner meets the tape's input earlier in its play. Cut the first
N polls of the tape (N = the difference of the polls of the first full view, 204 here) and the two builds walk the same
way: the same eye sequence on the four tapes, and frames at the same poll identical outside the player's rectangle (the
scarf is simulated per rendered frame, so it differs between builds of different speed). Level open and crossings are
read from the CD command log with one row per sector delivered; frame rate is flips over the polls after the first
full view. Tools were kept beside the lane notes (not in the repository): `waits.sh` (route log and CD command log per
tape), `cdstat` (level open, first full view, crossings, reads by kind), `fpsgame.sh`, `frames.sh`, `tilecmp`.

## Where the eyes stand, and what an eye leaves out

The cook (`playtest::panel_eyes`, `bake_panels`) puts one eye on each group of standable floor sectors of a
lattice cell and a storey, and bakes every panel of the eye without the rooms the camera draws whole.

* **Eyes beyond the edge.** The follow camera stands out over the edge of the level, up to seven sectors from the
  floor the player stands on. Lattice cells with no floor but within 0.7 of a spacing of one (`PANEL_CAMERA_OUT_TENTHS`)
  get an eye too, at the height of the nearest floor. On the 0851 tape the camera was more than 8 sectors from the
  eye in use for 36 percent of the ticks (plan distance beyond 6 sectors for 53 percent); with the extra eyes (149
  to 194, `WORLD.PAK` +0.8 MB) it is 3.5 percent.
* **What an eye leaves out.** The rooms the flood draws whole from wherever the camera is within a reach of the eye:
  the flood radius (12 sectors) less `PANEL_REACH_TENTHS` of a spacing (5, was 0.8). A room near the eye that stays
  in the bake is an occluder: when the camera walks on, what it hid is in no picture, and the panel's cells of the
  room are skipped as the room is drawn whole, so a half screen of tower showed sky (tick 2779 of the 0851 tape:
  15 cells of room 63 skipped, the rooms 80 and 81 behind it nowhere). A room left out that is not drawn whole is
  a hole the other way. Share of the far pixels the level shows and the game does not, on 0851 (`far_holes` in
  `editor/crates/psxed-project/examples`): old eyes and reach 6.7 percent, reach 0.5 5.2 percent, new eyes and
  reach 0.5 3.2 percent; the four other tapes 1.9 to 2.9 percent before and 2.0 to 2.1 after.

## Crossing from one eye to another

The camera crosses to a nearer eye at once. Before this change every tile of the new eye replaced the tile of the
eye left in the frame it landed in, so a crossing changed the whole far view over a few frames. Now a crossing is
faded in a room at a time (`panel_stream::Fade`, `psx_grid::render::PanelFade`):

* The cells of a panel name the one room they show, so the room is the unit. Rooms with a cell on screen start in
  the order of their depth, nearest first, `FADE_STARTS` rooms every `FADE_STEP_TICKS` ticks, once every tile of
  the new eye that shows the room (and is on screen) is in VRAM. A room off screen changes over at once.
* A room changes in two steps: the new eye's cells at half strength over the old eye's, then alone. The semi-transparent flag of a panel tile's palette entries is
  already set (`cards::encode`), so the draw only asks for `Blend::Average`. The old eye's cells go two ordering table slots
  under the new one's.
* The old eye's tiles stay in VRAM while the fade runs (`rebuild_show`); a cell with no picture to change over from
  or to is drawn as it is.
* Measured on the lockstep discs, 0851: the largest change of the far region between two presented frames in the
  12 ticks after a crossing, over what the walk alone does, 0.110 before and 0.050 after on average, 0.413 and 0.156 at worst (a walk with no crossing: 0.025 both).

The room table of the resident rooms (`ResidentRooms`) is made once at the size the streamer can hold
(`ROOM_ENTRIES`, 25): on the 0851 tape 25 rooms are resident at once and the table used to double from 16 slots,
7 KB of a heap that never gives a block of that size back.
