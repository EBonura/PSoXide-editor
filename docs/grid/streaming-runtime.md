# Grid streaming runtime (T16 phase 1, as built, 2026-10-19)

`streaming-design.md` is the plan. This is what landed on `grid/main` for the
first phase: the level leaves the executable and the rooms are read off the
disc as the player moves. Numbers here are measured unless they say otherwise;
the evidence files are named where they matter.

## What is on the disc and what is not in the executable

`WORLD.PAK` has one chunk per thing the level owns:

| Chunk | Content |
|---|---|
| 0 | the level core (`PGL1`), byte for byte what `grid_level.pgl` was |
| 1 | the level's own records (`psx_level::wire`): rooms' atmosphere, sky and camera, lights, particle emitters, interactables and their messages, vitality circles, logic, enemies, entities, destructibles, world objects, image/box/cylinder/arch props, model instances, the player's spawn and controller, the world message and ambient |
| `i + 2` | room `i` (`PGRM`) |

Room records still count their `pack_sector` from the start of the room blob;
the guest adds the sector of chunk 2 (read from the pack header). The core and
room bytes are identical to what the cook wrote before (`cookhash` core and
rooms columns did not move). Level textures are `PersistentGameplay` assets
read off `UI.PAK` with the clips.

The executable no longer has `grid_level.pgl`, `grid_rooms.pgr`, the cooked
record tables, or the grid material textures. `build.rs` embeds none of them.
The generated manifest keeps the project-wide data (UI nodes and paints, fonts,
characters and their clip tables, weapons, flow, the asset table) and, for each
table of chunk 1, one `LEN_<TABLE>` count.

Why counts: a 2 MB machine cannot size every array for the largest level, and
dropping what a level does not have (no enemies, no cylinder props) removes
code. With every table sized by a fixed cap and no compile-time knowledge, the
gate project's image grew by 430 KB of text and 77 KB of bss and the cathedral
no longer linked (`.bss` overflowed `IMAGE` by 111,864 bytes). The counts are
the level's shape, not its contents; `LevelTable<T, N>` is the empty constant
slice when `N` is zero, and the guest refuses a chunk whose table lengths
disagree with them. `psx_level::wire::caps` holds the fixed ceilings the cook
checks (`GAME_ENTITIES` 64, `LOGIC` 64, `MODEL_INSTANCES` 64, `INTERACTABLES`
32, `BOX_PROPS` 8, `ROOMS` 65534); a level over one fails the cook naming it.

Still embedded, level-scoped: one 8,252 byte texture per reference project
(`flags: 0`), made by `cook_props_lights.rs` for prop and light materials.
Moving it needs the prop texture paths to resolve bytes from the persistent
arena the way `grid_materials` does.

## Boot, then streaming

1. First `on_enter_state` (the engine enters the front end's resource sets
   before the scene's `init`, so this is the first hook): `GridRuntime::read_level`
   reads the pack header, the core and chunk 1, decodes chunk 1 and installs
   the tables (`level_data`), keeps the core. Idempotent; `init_gameplay`
   calls it again before it touches a table.
2. `init_gameplay`: `load_manifest` builds the runtime over the kept core. No
   room is resident.
3. Every tick `GridRuntime::stream` asks `psx_grid::stream::StreamWindow` for
   the wanted rooms (Need inside the draw radius, Lead inside the lead radius,
   by walk distance over the portal graph), hands them to `GridStreamer`, which
   drives `psx_residency` over a static pool of 2 KiB pages
   (`GRID_ROOM_POOL_PAGES`, sized by `plan_pool` from the busiest position of
   the level: peak wanted pages plus half again plus the largest room). Reads go
   straight into pool pages through `psx_cdstream` (interrupt driven, one sector
   per interrupt, direct to the destination), installs happen when a read
   lands, eviction follows the residency tiers.
4. The loading screen waits for `rooms_ready()` (the rooms around the spawn) and
   the persistent assets. After that, play never waits for the disc: a room that
   is not there yet is not drawn or walked into, and `need_missing` counts it.

## The CD Pause rule

`psx-cdstream`'s engine pauses only from the interrupt handler (end of a
transfer, a cancel, a recovery), then waits for the second response (INT2) as
an event, with a 600 vblank watchdog. It issues nothing while the Pause is
pending and starts the next transfer only once the drive reports it stopped.
Nothing in it assumes how soon the second response arrives, so the measured
window (1.2 to 3.9 M cycles after the acknowledge) and the longer silicon
figures are both inside the wait. Verified by running the corridor route on
the emulator build `psoxide-emulator` of `dddd2814` (same bytes as
`campaign-2026-10-15/demo-v047/bin/psoxide-emulator`): 16 installs, 9
evictions, no failed reads, no missed Need rooms, sim state identical to the
frontend's emulator.

## Measured

Executable file size, bytes, `grid/main` e0e98d60 against this branch (both
`cd-stream-bench sim-trace` builds). The file no longer holds the 428 KB
runtime arenas (they are `.bss`):

| Project | Before | After |
|---|---:|---:|
| grid-gate | 1,273,856 | 1,038,336 |
| grid-starter | 1,257,472 | 1,038,336 |
| grid-tower | 1,329,152 | 1,038,336 |
| grid-corridor | 1,454,080 | 1,038,336 |
| grid-cave | 1,536,000 | 1,224,704 |
| grid-dungeon | 1,568,768 | 1,224,704 |
| grid-sewers | 1,538,048 | 1,226,752 |
| grid-west | 1,542,144 | 1,226,752 |
| grid-level008 | 1,556,480 | 1,226,752 |
| grid-cathedral | 1,634,304 | 1,226,752 |

RAM is the number that matters: file plus `.bss`, against the 2,000,896 byte
`IMAGE` region. Heap headroom (what is left before the heap starts, before the
core and decoded records are allocated into it):

| Project | Before | After |
|---|---:|---:|
| grid-gate | 684,408 | 562,584 |
| grid-tower | 629,112 | 509,368 |
| grid-cave | 410,008 | 261,224 |
| grid-level008 | 389,528 | 238,824 |
| grid-cathedral | 311,704 | 160,888 |

It went down by 120 to 150 KB per project: the streaming code (`psx_residency`,
the streamer, the wire decoders, `LevelTable`) is larger than what left, and
the pool is sized for the busiest position rather than the level (cathedral:
112,640 bytes of pool, the same as the rooms it replaced, so streaming saves
RAM only on levels bigger than the pool). The cathedral boots and plays with
160 KB of headroom; its core and records are 1,268 bytes on the heap.

Boot to the first simulation tick grows by 32 to 59 ticks per project (gate
123 to 155, cathedral 190 to 249): the core, the records and the rooms around
the spawn come off the disc behind the loading screen.

Corridor (16 halls, draw radius 10 sectors, lead radius 6; `editor/projects/grid-corridor`,
a recorded walk down its length, 8,500 polls), emulator with the Pause
model, telemetry build:

| | grid/main | this branch |
|---|---:|---:|
| display flips at 2 vblanks | 3,499 of 3,499 | 3,499 of 3,499 |
| guest frame cycles, p99 | 830,271 | 827,731 |
| guest frame cycles, max | 920,547 | 942,863 |
| `update`, mean per frame | 144,508 | 167,745 |

The streamer's own stage (`cd_room_chunk_load`) costs 8,971 cycles a frame on
average and 39,013 at worst. The level is 96 pages; the pool is 42 and was
full at peak. 16 installs, 9 evictions, 0 Need rooms missing, 0 failed reads. Sim state identical to
`grid/main` on 7,398 judged ticks. The worst boundary frames are 22 K cycles
over the worst frame of `grid/main`, inside a 2 vblank budget of about 1.14 M.

Nine reference projects (gate, cathedral, cave, dungeon, tower, level008,
sewers, west, starter): the east walk is sim-state identical to `grid/main`
over 1,897 judged ticks each, and the last frame differs by 23 to 541 bytes
of 230,400 (scattered pixels, nothing structural; not traced further, the two
builds start rendering at different boot ticks). The first comparison, before
the records were read ahead of the front end, showed the level008 sky missing
(189,408 bytes); the frames are kept as evidence in
`grid-streaming/review/caught-regression/`.

## Gates

* `cookhash.sh` against `cookhash-golden.txt`: core and rooms for all 10 levels
  unchanged; the manifest column was refreshed on purpose (see the commit).
* `simgate`/`cmp-sim.sh` on the east tape for each reference project, and the
  corridor route with `stream-state` lines.
* `every_reference_projects_records_survive_the_chunk`: the chunk decodes to
  what the cook built, table by table, for every project in `editor/projects`.
* `psx-grid` (`stream_plan_tests`), `psx-game-runtime` (`grid_stream` with a
  scripted transport, `cd_stream::world_pack`, the arena zero-image test),
  `psx-level` (`wire`).

## Not done, and what blocks 30 fps

* Props' and lights' textures stay in the executable (8,252 bytes per
  reference project).
* Project-wide tables (UI, characters, clips, weapons, flow, the asset table)
  are still in the executable; they are what is left of the 430 to 500 KB of
  generated manifest.
* The executable is specialised per level by the `LEN_` counts. A single
  executable for every level needs about 120 KB more RAM than the cathedral has
  left.
* The pool is sized for the busiest position of the level, so it is not smaller
  than the levels that fit whole.
* The music yield around reads (`cd_drive`) is used; the SDK helpers named in
  the design (`stop_proved`, an idle hook, the `psx-loading` corner widget,
  `Player::suspend`/`resume`) are not in the SDK pin `629a222f`. A loading
  corner widget is not needed: the only wait is behind the loading screen.
* Steady 30 fps in the larger levels is not blocked by streaming (display
  cadence above) but by frame cost in the draw and player stages; that is a
  separate lane.
