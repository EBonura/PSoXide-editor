# Changelog

## Unreleased

- The grid line is the editor now. The branch that held the grid-world editor,
  `psx-grid` runtime and the Citadel is the mainline, and the BSP-based Cortex
  engine and its authoring (brush editing, the PXBSP world cook, the region
  streamer) are gone from it; `archive/bsp-engine-2026-10-24` keeps the last
  commit that had them. `psx-bsp` and `psx-goldsrc` stay because Quake,
  Half-Life and Counter-Strike build against them from this repository. The
  Cortex-only parts of `psx-bsp` (the world streamer behind the `streaming`
  feature, destructible submodels, the collision-provider adapter and the PXBSP
  toolchain probe) are removed; its sky functions now live in `psx-engine` and
  `psx_bsp::sky` re-exports them.

- The far vista ring is gone. It was an old feature: a ring of textured cards
  around the camera, set on the World node. The Far Vista section of the World
  form, the preview ring, the cooked room field and the runtime draw are all
  removed. Projects that still hold a `far_vista` block load as before and drop
  it on the next save; none of the shipped projects had it switched on. The
  room records chunk is version 2 (24 bytes smaller per room), so discs need a
  fresh cook.

- Grid worlds can gather their light. With `global_light` on, every face corner
  samples the hemisphere above it on the host: rays that meet a nearby wall
  darken the ambient, rays that leave the world bring in a sky colour, and
  rays that land on a lit wall bring that wall's colour back, scaled by the
  average colour of its texture (two bounces by default). It is all baked into
  the vertex colours, so the console pays nothing, and a quad's corner order is
  turned where the other diagonal fits the light better. New projects start
  with it on; a project saved before the setting keeps the flat ambient. The
  editor preview now bakes the same shadows a Play build does (it used to cast
  none), and refines the gathered light on a background thread.

- Music nodes now play XA-ADPCM from the disc instead of CD-DA tracks. The cook
  encodes each Music WAV to 37.8 kHz stereo XA, four songs to a file
  (`MUSIC1.XA`, ...), and writes a song table the engine plays by track number,
  so existing Music nodes keep their meaning. Discs carry one data track and no
  audio tracks. A looping song restarts by seeking, so each loop has a short
  gap. Fades are unchanged; a data read still pauses the song and restarts it
  from the point the head had reached.

- The shared chain loader hands the CD drive to the payload quiet. The last
  stop of a load is now `Reader::stop_proved` (the SDK's `SectorReader::stop_proved`:
  wait for the Pause's second response, 1.2 to 3.9 million clocks after the first
  on the console, settle by status if it never comes), and the loader acknowledges
  every controller flag, drops the data request and closes `I_MASK` before the
  jump. Before, the payload could receive that second response (a stray INT2) in
  its first moments. Intermediate chunk stops are unchanged, and a stop that
  cannot be proved fails the payload stage and retries like any other.

- The shared chain loader can animate its loading screen. `Observer::tick` and
  `Presentation::tick` (both default to nothing) are called after every payload
  sector, once per spin of a seek wait and every 4 KiB of the checksum pass, and
  `paint::take_vblank` counts VBlank edges while interrupts are masked. A
  presentation that polls the SDK's `psx-loading` indicator there keeps its
  text moving for the whole load. The transport itself (reads, stops, events,
  checksum) is unchanged: the legacy-oracle traces still match.

- `psxed-audio` can cook sounds straight from WAV files or in-memory bytes
  (`cook_sounds`, `import_files`), sharing the decode, resample, normalize and
  encode path with `import_pack`, so the `.psau` output is byte-identical.
  Callers that already hold the WAVs no longer have to build a zip archive and
  a manifest first. The zip sound-pack importer now sits behind the default-on
  `pack` feature, so those callers can drop the zip dependency.

- Walls and floors close to the third-person camera are now drawn where they
  really are. The GTE's perspective divide pulled anything nearer than about
  50 units toward the screen centre, which warped near walls and let sky show
  through gaps at the screen edge. Faces that close are now projected exactly,
  and polygons the GPU would drop for their size are clipped instead of lost.

- The game runtime reads the disc through the shared psx-cdstream transport: an
  interrupt-driven, seek-first PIO reader that replaces the polled DMA path.
  CD-DA music now shares the drive through an audio lease, so a data read pauses
  a playing track where it stands and the track resumes from that position
  afterwards. The SDK pin moves to the psx-cdstream revision.

- The emulator now walks GPU linked lists through the GPU's command FIFO and
  charges drawing at costs fitted to console captures (hardware tests v1.23
  and v1.24), so headless frame rates sit closer to a console. Save states
  move to format 7: save states from earlier builds no longer load.

- Dead enemies now dissolve over roughly five seconds from the uppermost
  polygons down, with fragments rising and fading before the body is removed.

- The initial welcome panels now hold player controls until dismissed and include a guide to both analog sticks. Other messages remain interactive.

- New Game opens with a cinematic introduction: two views of the level, then Aletha rising from the ground. Hold X for half a second to skip.
- The opening includes smooth fades, a sound cue for each shot, and a burst of polygons as Aletha strikes the ground.

## Demo disc v0.35

- Enemies change stance to match the distance: melee up close and ranged attacks when you pull away, while respecting the swap cooldown.
- Combat music starts at a different point for each encounter and keeps looping.
- Inactive stance health recovers more slowly.
- The heavy enemy has three new attack animations, longer melee contact windows and a chest-fired projectile.
- The animation editor now supports separate hit windows, named combat volumes, and visibility controls for hurtboxes, hitboxes and projectiles.

- Dashes scatter Aletha's polygons behind her, leaving a wireframe that gradually rebuilds as she moves.
- Releasing the stick now starts the transition to idle without an extra stride.
- Phase swapping now recharges in five seconds for both the player and enemies.
- Stance changes burst the original mesh outward, then rebuild the body from translucent polygons over a wireframe, assembling from feet to head and holding the new colour until complete. The stance colour then fades smoothly back to Aletha's normal shading.
- Sprint starts immediately, and evades cover less ground with a committed direction.
- Sword sounds, trails and damage follow each swing, including both cyan heavy strikes.
- Reworked the heavy enemy's two melee combos, attack tells and weapon contact.
- Corrected all three light-enemy swings so their damage, trails and sounds follow the claw.
- Improved the camera around pillars, low ceilings and fully compressed wall views.
- Added pickup and gameplay-entry sounds, clearer module instructions, and quicker message transitions.
- Combat music continues while the inventory is open. Default Cortex brightness is now 2.
- Fixed Select detaching the camera and stopping player movement in Cortex Ignition.
- New Game now starts a fresh run after returning to the title screen.
- Raised the default menu music volume; saved volume settings are preserved.

## Source 2026.09.05

This source snapshot is tagged `source-2026.09.05`. Download versions are
listed separately below; source cleanup does not replace an already published disc.

- The editor, engine and Cortex Ignition now live in one repository with pinned SDK and emulator dependencies.
- Removed unused renderer experiments and their public feature switches; the renderer paths used by Quake, Half-Life and editor projects remain.
- Cleaned host and guest compiler warnings and strengthened instruction-hazard checks.

## 0.4b-split.20260905 | 2026-09-05

Cortex Ignition published on itch.io.

- Light enemies charge one ranged shot; their melee attack has three separate damage/trail windows.
- Player and enemy swaps share a longer delay. Hits can interrupt attacks through the new poise rules.
- Updated music, combat/footstep audio, enemy awareness, lock-on camera and the closing demo message.
