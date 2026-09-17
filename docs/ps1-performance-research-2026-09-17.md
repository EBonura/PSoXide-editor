# PS1 performance research sweep (2026-09-17)

What the public record says about getting more out of the machine, checked
against what the Cortex path already does, and tied to the hardware-test
record that will confirm or refute each claim on a console. Numbers are the
sources' own unless marked as measured here. "clk" is one 33.87 MHz cycle.

Starting point, from `demo-disc-optimization-survey-2026-09-01.md`: Cortex is
memory-bound. About 35% of every vblank is RAM load stall, 10% is stack load
stall, 10% is I-cache refill. Anything that removes a RAM access is worth more
here than anything that removes an instruction.

## Corrections to earlier assumptions

- **RAM_SIZE bit 7 is not a free win.** psx-spx (memorycontrol) says clearing
  it hangs many games during CD loading on early and late PU-8 boards, and
  works on PU-18 through PM-41. The A/B probe is still safe, because the
  flipped state lasts one interrupt-masked assembly block, but a shipped game
  cannot clear it at boot unless it restores it around every CD access or
  refuses on PU-8.
- **The cache-control bits are documented after all.** LSI's L64360 datasheet
  embeds the same CW33300 register block. NOPAD clear adds a wait state to the
  end of every bus transaction; LDSCH lets the core keep going after a load
  until the data is needed; RDPRI gives loads priority over queued stores;
  NOSTR, BGNT and INTP are marked unused on that part. The BIOS value
  `0x1E988` already has LDSCH, RDPRI and NOPAD set, so the default is the fast
  setting. Records `E2`-`EC` now have predictions: clearing NOPAD slows every
  bus access, clearing LDSCH removes the load shadow, clearing RDPRI slows
  store-then-load.
  Source: https://www.digchip.com/datasheets/download_datasheet.php?id=488501&part-number=L64360
- **The emulator has no write queue and no load shadow.** Sony's D-cache
  technote, nugget's measured memcpy/memset kernels and the pcsx-redux
  load-timings test agree that stores go through a four-entry queue (a store
  followed by three independent instructions costs nothing; back-to-back
  stores about 2 clks each) and that a load hides part of its wait behind
  following independent instructions. In PSoXide a store then three
  instructions costs 326 for 64 (additive) and a load then four instructions
  710 (additive). Records `1F` against `76`, and `CE` against `74`.
  Sources: https://psx.arthus.net/sdk/Psy-Q/DOCS/TECHNOTE/DCACHE.PDF,
  https://github.com/grumpycoders/pcsx-redux/blob/main/src/mips/common/crt0/memory-s.s,
  https://github.com/grumpycoders/pcsx-redux/blob/main/src/mips/tests/load-timings/load-timings.c

## Untried on the Cortex path, in the order the profile suggests

1. **`$gp`-relative small data.** `_start` never sets `$gp`, the EXE header
   carries GP = 0 and `psoxide.ld` has no `.sdata`/`.sbss`, so every global
   costs an address build. Sony's 1996 optimisation deck lists this first.
   LLVM needs the hidden `-mgpopt` with `-mips-ssection-threshold=N`
   (noabicalls is already the case), `_gp` at small-data start + 0x8000, and
   `$gp` loaded in `_start` and in the exception entry. A static given an
   explicit `.sdata` section counts as small at any size, so hot statics can
   be placed by hand. No measured PS1 gain was found anywhere; the survey
   counted 3,337 to 6,638 `lui` sites.
   Sources: https://psx.arthus.net/sdk/Psy-Q/DOCS/CONF/SCEE/96April/optimize.pdf,
   https://github.com/llvm/llvm-project/blob/main/llvm/lib/Target/Mips/MipsTargetObjectFile.cpp
2. **Transform each world vertex once.** The PXBSP path re-materialises and
   re-projects every face's vertices; the indexed machinery exists in
   `world_render/indexed_cache.rs` and is proven on the grid path. Driver 2
   goes further for quads: RTPT on three corners, NCLIP, and the fourth
   vertex only if the face survives.
   Source: https://github.com/OpenDriver2/REDRIVER2 (`draw.c`)
3. **Shrink or splice the ordering table.** 2,048 slots for about 364
   primitives. Sony's analyser note puts an empty entry at 8 clks of DMA and
   recommends a coarse main table with fine tables spliced in where depth
   resolution matters, plus a static one for the HUD. DuckStation charges 8
   per header; PSoXide 16. Records `33`/`34` settle the number, and `ot-1024`
   already exists unmeasured.
   Source: https://psx.arthus.net/sdk/Psy-Q/DOCS/TECHNOTE/ordtbl.pdf
4. **Stack on the scratchpad for the hot kernels.** Sony claims 10 to 15%;
   hl-psx already ships the mechanism. The constraint is that `psx-bsp`
   already owns most of the 1 KiB for data, so it needs a phase-disjoint
   reservation. A related SN Systems note pins a register to a scratchpad
   context struct; Driver 2 keeps its whole plot context there.
   Sources: https://psx.arthus.net/sdk/Psy-Q/DOCS/CONF/SCEA/adv_gte.pdf,
   https://psx.arthus.net/sdk/Psy-Q/DOCS/TECHNOTE/Glblreg.PDF
5. **Build packets for the write queue.** Whole-word sequential stores, never
   byte stores (four byte stores fill the queue), and independent work between
   bursts of four. Record `37` prices byte stores, `1F` the spaced store.
6. **Load the next RTPT's inputs behind the current one.** psx-spx's GTE
   pipeline page (measured on one SCPH-5501) says `mtc2` does not stall while
   a command runs and RTPT has latched everything within about four cycles.
   `rtpt_kick` interleaves packet writes today but loads the next triple only
   afterwards. Record `3A` against `28`.
   Source: https://psx-spx.consoledev.net/gtepipelinetimings/
7. **memcpy in batches of eight.** Measured on hardware by nugget: 8.72 clks
   per word with eight loads then eight stores, 9.18 with four, 12.80 with
   two. `psx-rt`'s memcpy uses four; its memset is already at the measured
   floor.

## GPU side (matters when a scene turns fill- or setup-bound)

nocash's own psx-spx has a hardware-measured rendering-timings chapter that the
consoledev mirror lacks (https://problemkaputt.de/psx-spx.htm).

- Per-triangle setup: 10 clks, plus 90 if textured, plus 150 if Gouraud; a
  quad pays it twice. Setup of the next triangle overlaps the current one's
  fill, so it only bites on runs of small polygons. Cortex's mean extent is 29
  pixels. Flat-textured where the vertex colours are equal saves setup and two
  packet words per triangle. Records `3B` against `38`.
- Fill: about 0.5 clk per pixel flat or rect, 1.0 textured or Gouraud, more
  when semi-transparent; rects have no setup at all. `prim::Sprite` and
  `RectFlat` exist with no callers while particles go out as quads.
- Texture cache: 2 KB, fixed mapping, flushed by any texpage change, any
  4bpp/non-4bpp switch and any VRAM upload or copy. A CLUT change reloads 16
  entries at 4bpp and all 256 at 8bpp (256 clks). The engine sorts by depth
  only: within an OT bucket, sort by texpage then CLUT, and never interleave
  uploads with drawing. Record `CB` against `AF`.
- The GPU renders at full speed only while it is not fetching the picture;
  nocash says a one-line vertical display range removes nearly all of that.
  A letterboxed picture buys GPU time. Record `F6` against `A2`.
- Fill (`GP0 02`) is about six times faster than a flat rect for clears.
  Record `BE` against `AE`.
- Spyro shipped two worlds per level, the far one untextured; Andy Gavin
  measured untextured at twice the speed of textured and built Crash's
  characters from it. There is no LOD of any kind in the engine today.

## Bus ownership during DMA

psx-spx and Sony's 1996 deck both say the CPU keeps running during a DMA only
while it stays in registers, I-cache, scratchpad and GTE (plus up to four
queued stores), and stalls on the first RAM or I/O access until the transfer
yields. PSoXide lets it run freely. If true, the code that runs while the
previous frame's list drains should be cache-resident with scratchpad data,
and GTE work can overlap uploads. Records `35`/`36` (nops) and `9F`/`FE` (RAM
loads).

## Content techniques with a record of paying off

- Crash: visible set and draw order precomputed per camera position, stored
  as small deltas; the world needs no ordering table at all. For static BSP
  leaves the order per leaf is exact and could be cooked; the OT would be left
  for dynamic objects.
  Source: https://jackpal.github.io/2015/03/27/Crash_Bandicoot_Dev_on_rendering_techniques.html
- Driver 2: face normals quantised at load into an index into a 32-entry
  colour table, so no lighting call at draw time; 64 yaw buckets sharing
  camera-composed matrices computed at most once per frame.
- Cull before lighting: RTPT, NCLIP, and skip the 39-clk NCCT for culled
  faces. (NCLIP's MAC0 staleness is why the world path culls on the CPU; this
  applies where the wait can be scheduled.)
- WipEout and CTR: three texture detail levels chosen by distance. Sony's
  analyser note says a 3x3-pixel polygon sampling a 128x128 texture reads
  eight texels per pixel drawn.
- Hard-code sector locations instead of searching directories, keep seeks
  within about 100 sectors, avoid speed changes (Sony's CD deck).

## Checked and not worth pursuing

- Cache-control bits as a speed-up: the default is already the fast setting.
- Seeding the I-cache by hand (LSI's "IRAM" mode) and D-cache mode: marginal,
  and the second deadlocks the bus on scratchpad access.
- `GP0(03h)` render timing control: the default is already the fastest.
- 8 MB RAM window, hardware breakpoints, 24-bit mode: no speed relevance.
- Software overclocking: there is no clock control in the I/O map.

## Not found

Primary technical material for Tekken 3, Tobal 2, Vagrant Story, THPS and the
demoscene; any measured PS1 gain for small-data addressing; anything on
steering LLVM away from `lwl`/`lwr`.
