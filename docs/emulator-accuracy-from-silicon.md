# Emulator accuracy gaps confirmed against silicon

Findings where a real console and PSoXide provably disagree. Each entry records
what was measured, on what, and how to reproduce it, so a fix can be verified
rather than argued about.

A gap only belongs here once it has been observed on hardware. Suspicions from
reading code belong in the relevant subsystem doc instead.

## The first complete capture (2026-07-26)

`docs/hardware-refs/px7-silicon-2026-07-26.txt`, HWTEST v1.4, recovered from a
console recording via `tools/hwtest-video-qr.py` and CRC-valid. 158 values differ
from PSoXide. Compare with `make hwtest-silicon SILICON=<that file>`.

The harness validates itself on this capture: console single-speed reads measure
13.20 ms/sector against a 13.33 ms spec, and double-speed 6.49 against 6.67. The
CD numbers below are therefore trustworthy in absolute terms, not just relative.

### GPU rasterisation is pixel-exact

All 22 bit-exact raster hashes match silicon, every primitive family and edge
case. Whatever else differs, the rasterizer draws the right pixels.

### CD-ROM seek: far too fast, and distance-independent

| Seek | Console | PSoXide |
|---|---|---|
| +1 sector | 181 hblanks (12 ms) | 12 |
| +16 | 1239 (79 ms) | 13 |
| +128 | 5679 (361 ms) | 761 |
| +512 | 3022 (192 ms) | 763 |

The console cost tracks head travel; PSoXide returns essentially two values
regardless of distance. Any game whose streaming budget depends on seek cost is
being modelled optimistically by one to two orders of magnitude.

#### The seek data does not support a model yet

Attempted 2026-07-26, not implemented. Four distances is too few, and they are
non-monotonic: +128 sectors measures 361 ms while +512 measures 192 ms. Neither
a linear nor a square-root fit through the endpoints gets within 2x of the
middle points (both land ~0.2-0.5x at d=16 and d=128). No monotonic physical
model reproduces this, and baking a non-monotonic table into the emulator would
encode one disc's anomaly as hardware behaviour.

What it needs: more seek distances, several repeats each, ideally on more than
one disc, so an outlier is visible as an outlier. That is a guest-side change to
records `0x90`-`0x93`, so it wants doing before the next burn rather than after.

### CD-ROM read: four times too SLOW

Console 13.20 ms/sector single speed, 6.49 double, both within 1% of spec.
PSoXide takes 53.5 ms/sector, so a data read costs 4x what it should. This is the
opposite direction to the seek error, so the two do not cancel.

The constants are NOT the problem: `CD_READ_TIME` is 451,584 cycles, exactly
13.33 ms, and `sector_read_cycles()` halves it for double speed. Both are right.
The 4x is somewhere in the DataReady scheduling. `cdrom.rs` pushes a due
DataReady event out by `CD_READ_TIME / 2` whenever a CD IRQ is still pending
unacknowledged, which is a plausible contributor but accounts for 1.5x at most on
its own.

Confounder to rule out first: the probe acknowledges INT1 without ever reading
the sector data. Real software drains the sector; if the emulator's pacing
depends on that, the 4x is partly the probe's own doing and the fix belongs in
the guest. Settle this with emulator-side instrumentation of actual INT1
deltas before changing any scheduling.

### CD-DA contention: no measurable effect, on either

`0x9B` (read with audio playing) against `0x9C` (audio stopped) measures 1647
against 1657 hblanks on console: no contention at this granularity. A negative
result, and worth recording as such, since the premise of the probe was that
hardware would show a penalty here.

### GPU fill: the emulator rasterises on the CPU's timeline

Console fills are 3x to 80x faster than PSoXide for small primitives, and
SLOWER for large ones (`gpu_fill_rect_mono_16x32`: console 24021, PSoXide
13804). The two are not measuring the same thing. Hardware absorbs packets into
the FIFO and draws in the background, so the CPU-side interval reflects
submission until the FIFO backs up; PSoXide appears to rasterise during the GP0
write, putting draw cost on the CPU's clock.

`gpu_fill_quad_flat_4x64` matches almost exactly (9987 against 10114) because at
that size the console genuinely blocks, which is consistent with this reading.

This is the data that was missing to model GPU timing at all, and it says the
current model overstates CPU cost for small draws and understates it for large.

### SIO: one pad pacing does not work on hardware at all

| Variant | Console | PSoXide |
|---|---|---|
| 0 | 7516 | 375 |
| 1 | **no response** | 1391 |
| 2 | 9587 | 3439 |
| 3 | 12304 | 6521 |

Console polls take 2x to 20x longer, and variant 1 gets no valid reply at all
while PSoXide answers happily. That is the SCPH-1200 setup-delay family of
problem, now a standing measurement rather than a session of guesswork.

### MDEC

Table uploads cost the console about 1.6x what PSoXide charges (99 against 66
cycles for a 16-word luma table). Decode is roughly linear on console (4205 for
one macroblock, 7463 for two) and wildly non-linear in PSoXide (3119, then
39374), which points at the emulator's lazy decode-on-read rather than at
hardware.

## SIO: the pad's setup delay is not modelled at all

**Status:** open, fully characterised. **Found:** 2026-07-26, HWTEST v1.5 capture
(`docs/hardware-refs/px7-silicon-v1.5-2026-07-26.txt`).

A twelve-point sweep of the setup delay between selecting the controller and
clocking the first byte. The console has a hard threshold; PSoXide has none.

| Setup spins | Console | PSoXide |
|---:|---|---|
| 0 | **no reply** | 378 |
| 64 | **no reply** | 891 |
| 128 | **no reply** | 1403 |
| 192 | 8587 | 1915 |
| 256 | 8771 | 2432 |
| 512 | 10541 | 4475 |
| 1024 | 14077 | 8571 |
| 1536 | 17615 | 12667 |

Two separate defects. PSoXide **answers a poll that real hardware ignores** at
every delay below 192 spins, so guest code with too short a setup delay works in
the emulator and fails silently on a console. And once the pad does reply, the
console takes 2-5x longer per poll than PSoXide charges.

This is the SCPH-1200 controller problem, previously a remembered anecdote that
cost a debugging session, now bracketed to between 128 and 192 spins. The SDK's
`DEFAULT_SETUP_SPINS = 1024` is safely above it, and now demonstrably so rather
than by luck.

One caveat: an earlier capture saw setup 0 reply once. The boundary is not
perfectly repeatable, which is expected of a physical handshake, but everything
below 192 failed in the sweep run.

## CD seek is variance-dominated, not distance-dominated

**Status:** closed as "do not model as f(distance)". **Found:** 2026-07-26.

Ten distances, five repeats each, min/median/max retained:

| Distance | min | median | max |
|---:|---:|---:|---:|
| 1 | 11.3 ms | 11.5 | 12.6 |
| 4 | 51.2 | 51.4 | 51.6 |
| 8 | 104.9 | 105.1 | 117.9 |
| 16 | 104.5 | 104.7 | 105.3 |
| 32 | 289.3 | 289.4 | **552.8** |
| 64 | 159.7 | 173.6 | 318.1 |
| 128 | 137.3 | 137.6 | 137.7 |
| 256 | 90.7 | 91.1 | 91.6 |
| 512 | 151.8 | **310.2** | 310.5 |

Still non-monotonic with ten points, and now the reason is visible: the spread
within one distance (32 sectors ranges 289 to 553 ms) is larger than the spread
between distances. Seek cost here is dominated by rotational position and head
settling, not by how far the head travels. Backward seeks confirm it, differing
from forward in opposite directions at 64 and 256 sectors.

**Consequence:** the earlier plan to replace `SEEK_SECOND_RESPONSE_CYCLES` with a
distance model was wrong, and would have encoded noise. The defensible change is
to keep a constant and raise it: PSoXide's 53 ms sits below almost every console
sample, whose typical cost is 100-300 ms. That is a one-line change with a
measured target, and it wants checking against game boot paths since seeks
getting several times slower can expose timeouts.

## SPU: an all-zero ADSR does not decay

**Status:** open. **Found:** 2026-07-26, console recording, HWTEST v1.2 disc.

Writing `ADSR = 0` (both halves) sets sustain level 0. On real hardware the
envelope therefore runs attack, then decays to silence shortly after key-on. In
PSoXide the voice holds its key-on level indefinitely.

Measured on the same guest binary, RMS per second of the hardware-test audio
readout, which keys one voice on and lets it run:

| | Envelope |
|---|---|
| Console | 863, 452, then silence. Gone in ~3 s |
| PSoXide | ~897 flat, unchanged after 44 s |

The console recording is `2026-07-26 12-29-39.mov`; the emulator side
reproduces with any build of the disc predating v1.3, since v1.3 stops using
that ADSR:

```sh
make hwtest-audio     # writes build/hwtest-audio.wav
```

then take RMS per second of the result.

**Why it went unnoticed:** the emulator's SPU parses ADSR into phases and
carries a `sustain_level` field, so the configuration is decoded; what does not
happen is the decay running down to a sustain target of zero. Anything relying
on a held voice therefore behaves in the emulator and fades on hardware.

**Blast radius:** any guest that keys a voice on with a zero ADSR expecting it
to hold. The SDK's `Adsr::passthrough()` documented itself as exactly that
("voice stays at key-on volume until key-off"), which is how the hardware-test
disc came to use it and lose 80% of its payload on the first console capture.
That doc comment is corrected as of the same change.

## Conformance: 13 observations diverge

**Status:** open. **Found:** 2026-07-26, partial console capture, HWTEST v1.3.

Only page 1 of 5 was recovered, which carries the header and the first 138 of
173 conformance observations. Thirteen of those 138 differ from PSoXide. The
timing records and precision values live on the unrecovered pages and remain
unknown.

| Case | Group | Test | PSoXide | Console |
|---:|---|---|---|---|
| 10 | IRQ | GPU IRQ visible through I_STAT | `0x0000000C` | `0x0000000A` |
| 17 | GPU | GP0 IRQ set + GP1 ack | `0x00000001` | `0x00000000` |
| 23 | SPU | SPUSTAT readable | `0x00000800` | `0x00000000` |
| 32 | TMR | mode read clears sticky flags | `0x00000003` | `0x00000001` |
| 44 | SIO | direct port 1 pad poll stability | `0x00737373` | `0x00414141` |
| 46 | GPU | DMA direction mode latch | `0x00000004` | `0x0000000B` |
| 49 | GPU | GPU IRQ1 flag settle latency | `0x00010001` | `0x00010000` |
| 52 | GPU | DMA-direction readback values | `0x000000A0` | `0x000000D6` |
| 121 | GTE | LZCR settle +1 | `0x0000001F` | `0x00000008` |

Cases 14, 15, 38 and 51 also differ but are raw timer counts, where a small
difference is expected rather than a defect; they are listed here only so a
later capture is not mistaken for a new finding:

| Case | Test | PSoXide | Console |
|---:|---|---|---|
| 14 | timer2 free-run increments | `0x00009251` | `0x00009253` |
| 15 | timer1 scanline range | `0x00000070` | `0x00000049` |
| 38 | timer1 HBlank clock advances | `0x00000228` | `0x00000227` |
| 51 | timer0 dot/system tick counts | `0x92A71CB6` | `0x925D1CC4` |

The three startup scan digests (CPU, GTE, SPU register-behaviour fingerprints)
match exactly, so those subsystems agree at the level those scans probe.

Case 15 is worth singling out: `0x70` against `0x49` is a 27-line difference in
the scanline range, far larger than counter jitter, and case 23's `0x800`
against `0x000` is an entire SPUSTAT bit that PSoXide reports and the console
does not.

## Triage of the 13 conformance divergences

Investigated 2026-07-26. Most are NOT quick wins, and it is worth being precise
about why rather than filing thirteen tickets.

**Already known, blocked on FIFO/latency modelling (cases 10, 17, 46, 49, 52).**
The suite itself marks these `info` and says so in the source: *"Racy on
silicon: both the GPUSTAT.24 and the I_STAT observations race the GPU command
FIFO and flip run-to-run"* and *"the GPUSTAT bits 29-30 readback lags the GP1(04)
write through the FIFO"*. They flip between runs on hardware, so the console
values recorded above are one sample of a race, not a target to match. Closing
them means modelling GPU FIFO latency, which is gated on CPU cycle accuracy.

**Instantaneous snapshots of toggling state (case 23).** `SPUSTAT readable`
reads the register once. The difference is bit 11, "writing to first/second half
of capture buffers", which toggles continuously as the SPU runs. A single read
landing on a different phase is not a defect.

**Environmental (case 44).** `direct port 1 pad poll stability` depends on the
physical controller and what it reports; an emulated pad answering differently
from an SCPH-1200 is expected.

**Raw timer counts (cases 14, 15, 38, 51).** Small differences are jitter. Case
15's 27-line gap in the scanline range is larger than that and may be real, but
it needs a second capture to separate from a one-off.

**Genuinely actionable: case 32, `mode read clears sticky flags`.** See below.

## Timers: registers are read without catching up first

**Status:** open, root-caused, not yet fixed. **Found:** 2026-07-26.

The test sets Timer 2 to target 24 with reset-at-target, lets it free-run, then
reads the mode register twice. Reading mode clears the sticky reached-target
flag, so the expected result is "set on the first read, clear on the second".
PSoXide gives exactly that (`0x3`). The console gives `0x1`: set on the first
read, and *still set* on the second.

The console is right, and for a mechanical reason. The timer is free-running at
the system clock with a target of 24, so it reaches target every 24 ticks, which
is fewer cycles than two consecutive MMIO reads take. The flag is genuinely
re-latched between the two reads. It cannot be otherwise on hardware.

PSoXide never sees this because `Timers::read32` returns the register value
without advancing the timer to the current cycle first, and the bus timer-read
paths (`bus.rs`, the 8/16/32-bit branches) call it directly. Between two
adjacent reads the emulated counter does not move at all.

The code already anticipates the behaviour it does not implement:

> Reading mode acknowledges both sticky reached flags. If the counter is still
> at its terminal condition, hardware may latch the target flag again on a later
> timer clock.

**Fix shape:** advance the timers to the current cycle before servicing a timer
register read, the usual catch-up-on-access pattern. `Timers::advance_to` already
exists and `bus.rs` already calls `advance_to_video` elsewhere, so the pieces are
there; the read paths need the current cycle and video parameters threaded in.

**Expect fallout.** Making timers advance on access will move timing-derived
numbers across the suite, so the emulator baseline will need re-pinning and the
change wants checking against the timing records rather than landing blind.

## The first FULL characterisation capture (2026-08-07)

HWTEST v1.17, all five PX8 pages plus SB4 recovered from one console
recording. This is the capture the note at the bottom of this file was
waiting for, and most of what it found has already been fixed in v1.18.
What it leaves open:

### SPU RAM uploads do not land on silicon (the WRITE, not the read)

The v1.18 capture settles which half is broken, and it is the write.

Precision 002-017 are the raw words of a 64-byte DMA upload read straight
back. Undoing the documented unstable-read shape (one `0xFFFF` inserted
per DMA block, everything after it shifted by a halfword) reconstructs
SPU RAM as `0000 C0DE 0111 9C00 DD65 ...` against an uploaded `0000 C0DE
0111 C0DE 0222 ...`. **Only the first 6 bytes of 64 arrived.** Precision
039-042 read back a region written through the manual FIFO and match it
in **zero of 8 halfwords**.

Both readings are trustworthy: precision 037/038 hash the same region
through two different block shapes with the override armed and agree
exactly (`0x083B6E3D`), so the read path is self-consistent. A read that
were itself corrupting would not reproduce the same content twice at two
shapes.

This is the same fault as the demo disc's NitroXide tone. psx-sfx writes
a 16-byte parking block after every sample; a voice that finishes parks
on it and loops it forever. Emulator-side tests
(`a_correct_parking_block_is_silent_however_long_it_loops` and its
corrupted counterpart) show a correct block renders an exact 0 while one
whose header was replaced by `0xFFFF` self-loops audibly at 44100/28 Hz
= 1575 Hz -- the tone the console recording carries.

What is still open is WHY the write stops. `upload_adpcm` picks the
largest DMA block size dividing the payload, so 64 bytes go as a single
16-word block, which is the SPU's entire 32-halfword transfer FIFO with
no DRQ pause inside it. Conformance `0xBC`-`0xBF` ask the four questions
that separate the candidates: is the read stable, do the DMA and FIFO
writes agree with each other, does writing the transfer address after
arming the mode fix it, and does pacing the same payload as four
4-word blocks fix it.

### SPU RAM readback is lossy on silicon even with the DMA override armed

Conformance `0xA6` (DMA round trip) and `0xA7` (manual-FIFO round trip)
upload a known block to SPU RAM and read it back. Both still FAIL on
console (`B00FF59A` / `736B7C0F` against the analytic `D24F8305` /
`574B8A35`) after the v1.17 fix that arms the memory controller's SPU DMA
timing override (`1F801014h` bits 24-27) around the readback. The
emulator now PASSES both, so it is the permissive one.

The override was the right fix for the emulator, and PX7 precision values
036-038 show stable-mode reads being faithful on silicon, so the residual
corruption is NOT the unstable-read shape this suite already models. The
write side is the remaining suspect. The observed hashes move between
builds while staying stable within one, which points at content rather
than timing.

### NCLIP positive-winding anomaly (`0x8B`)

The one conformance case still failing in the emulator, and the console
passes it. The console computes the full cross product in both phases of
the controlled scene-C replica; the emulator's hazard model substitutes
an old Y in the settled reference phase, so its two phases disagree.
Two narrow fixes were tried and reverted: keying the spaced-cadence rule
on the SXY1->SXY2 gap alone, and restricting history establishment to
RTPT. Each repaired `0x8B` and broke the small-value settle cases
`0x74`-`0x78`, which the console passes. The model needs the
positive-winding anomaly itself, not another calibrated special case.
The v1.17 discriminators `0xAD`-`0xB0` were added for exactly this and
their console values are now in hand, including the read-interlock
result below.

### The CPU does not stall on COP2 reads (SCPH-9902; see the 2026-09-17 note)

`0xB0` brackets `nclip` plus an immediate `mfc2` against a two-nop
baseline on Timer 2 and reads `lo=8, hi=9`: reading MAC0 the instruction
after issuing NCLIP costs no more than two NOPs. psx-spx and DuckStation
both describe a read interlock that stalls until the command completes;
this console has none, which is precisely why partial MAC0 values are
observable at all. Any future GTE latency work has to start here.

### The demo-disc glyph corruption is in the glyph rect path

`0xB3`-`0xB5` all fail on console. `0xB4` (blit of the render-to-VRAM
text cache) and `0xB5` (the same text drawn directly) return the SAME
hash on silicon, exactly as they do in the emulator, so the cache round
trip is faithful and the corruption happens when the 4bpp glyph
rectangles are rasterised. Per-glyph probes `0xB6`-`0xBA` landed in v1.18
to name the guilty glyph; alignment alone does not explain it, since 'r'
shares 'f''s atlas row and its `u&3==3` start and renders correctly on
screen.

## How these get found

The hardware-test disc is the instrument: `make hwtest-silicon SILICON=<pages>`
diffs a console capture against the emulator record by record. A gap that shows
up as a moved number there is far cheaper to act on than one inferred from a
game misbehaving.

A full characterisation capture was finally recovered on 2026-08-07 (all five
PX8 pages plus SB4, from one recording, decoded by `hwtest-video-qr.py` in a
single pass). The older findings above predate it and came from a single
recovered QR page; the SPU ADSR gap came from the *shape* of a failed capture
rather than its contents.

## 2026-09-17: hwtest v1.22 performance captures, launch PAL console

Three captures from one session, archived as
`docs/hardware-refs/px8-silicon-2026-09-17-v1.22-{full,perf-sweep,perf-ab}.txt`.
The console identifies itself as BIOS 2.2 of 1995-12-04, so it is not the
SCPH-9902 the earlier captures came from. `PERF A/B` completed: no register
flip hung the machine. The sweep and the A/B run agree with each other to a
few cycles on every CPU record.

**The warm harness works on silicon.** Records `0x72`-`0x8D` and the sweep read
with zero or near-zero jitter, where the older CPU records swing by 100 cycles
or more in the same capture (`nop_block` 170-294, `taken_branch_delay`
325-415). The layout-dependent refill tax described in
`hardware-test-disc.md` is a property of the hardware run too, not only of the
emulator. Calibrate against warm records only.

**Confirmed to the cycle** (emulator already right): every warm GTE command
latency and both GTE gap knees; `mtc2`/`ctc2`/`mfc2` at one cycle; six `mtc2`
behind a running RTPT for free; a multiply issued behind a running one for
free; the divide knee at 36; the multiply bands, including `rs`-only selection
and the signed small-negative case; scratchpad, RAM and I/O load costs; the
SPU read delay and its shortening through `SPU_DELAY` (3527 to 2247).

**Folded into the emulator** (PSoXide-emulator branch
`silicon-timing-on-d366cd0`, each with a unit test pinned to the number):

| Finding | Silicon | Emulator before |
|---|---|---|
| Multiply read one cycle before it retires does not stall (`7A`/`7E`/`82`) | 110 / 158 / 222 | 126 / 174 / 238 |
| `swl; swr` pair on RAM is two ordinary stores (`CD`) | 266 for 64 | 902 |
| `lwl; lwr` pair on RAM is two plain loads (`CC`) | 901 for 64 | 1166 |
| Instruction fetched through KSEG1 (`1E`) | 6.1 cycles | 7.1 |
| I-cache fill streams at two cycles a word; NOSTR makes it blocking (`E9`) | 13 a line | 9 |
| Two-word refills: the half-valid line waits for its leading words (`EA`) | 14 a line | 12 |
| A RAM load during a fill waits for the bus (`3C`) | 24 a line | 21 |
| `RAM_SIZE` bit 7: one cycle when a RAM load shares the bus with a RAM fetch (`DC`/`DD`, `3C`/`3D`) | 1193/1120, 6242/5958 | not a register |
| Jumping out of a streamed line waits for the fill plus two cycles (`8C`, and `42`-`45` on both consoles) | 8 / 7 / 5 over a warm entry | 5 / 4 / 3 |
| SPU store pays the bus write delay (`FD`) | 14.2 cycles | 1 |
| Linked-list DMA arbitration per node (`33`/`34`, and `6B`) | 10.27 a node | 16 |

Summed absolute error against the A/B capture, GPU records excluded, went from
13,454 cycles to 845.

**Measured, not yet modelled**, because one point does not give the shape.
hwtest v1.23 adds the missing points:

* The write queue: a store followed by three independent instructions costs
  nothing (`1F`: 254 for 64, four cycles a turn); the emulator charges the
  store its two. v1.23 `129`-`12E`.
* The load shadow: a load followed by four independent instructions costs 8.9
  cycles against 8.0 with one (`CE`: 571); the emulator adds them (710).
  v1.23 `125`-`128`.
* The multiply interlock between k = 0 and k = m - 1 is assumed flat. v1.23
  `120`-`124`.
* A GTE read may wait for the command after all. The lerp through GPF reads
  MAC1-3 straight after the command and costs 12.75 cycles a turn, five more
  than its eight instructions, which is GPF's latency (`F8`: 150 against the
  emulator's 110). The SCPH-9902 finding above was MAC0 after NCLIP. Either
  the registers differ or the consoles do. v1.23 `130`-`134` separates them.
* RAM loads during a linked-list DMA are half again as slow (`FE`: 770 against
  510 idle), while register-only code barely notices (`36`: 133 against 126).
  The CPU gets the bus between nodes rather than losing it for the walk.
* A store to GP0 costs two cycles, like a RAM store (`FA`: 124 for 64).
* Whether a jump out of a streamed line costs the same when it lands in cached
  code. v1.23 `135`/`136`.

**Register A/B results.** `RAM_SIZE` bit 7 is real and worth about one cycle
per contended RAM load; psx-spx says clearing it hangs CD loading on PU-8
boards, so it is not a free switch. Cache-control NOSTR set disables streaming
(a cold sweep goes from 2356 to 3376). RDPRI, NOPAD, LDSCH and BGNT flipped
change nothing these workloads can see. The two-word refill size is slower.
The BIOS values are already the fast ones.

**The GPU fill battery is not measuring what it says.** `A0`-`AF` and the
v1.22 sweep's GPU records write primitives to GP0 unpaced (64 to 144 words into
a 16-word FIFO while the GPU draws) and treat GPUSTAT bit 26 as completion,
which pulses between primitives. On this console textured triangles read
faster than flat ones (2284 against 2837) and a VRAM fill slower than a
rectangle. Their textures were also mostly transparent texels. v1.23 replaces
the sweep's GPU records with DMA-list batches ending in a GP0(1Fh) interrupt
request, with opaque textures (`100`-`114`). The standing `A0`-`AF` records are
left as they are and should not be used.

**Open: the disc did not boot after a reset.** After the full run, two presses
of the reset button each reached the licensed PlayStation screen and then
stayed black. It booted again later in the session (both PERF captures were
taken afterwards); how is not recorded. The full run does not touch
`RAM_SIZE`, cache control or the display range. Not reproduced or explained.

## 2026-09-17, later: hwtest v1.23 captures, same console

`px8-silicon-2026-09-17-v1.23-{perf-sweep,perf-ab}.txt`. A/B completed again.
A press of the reset button after the sweep booted back to the menu, so the
failed warm boot earlier in the day is specific to what the FULL run leaves
behind, not to warm boots in general. Still unexplained.

**Predictions the emulator got right.** The multiply interlock is flat from
k = 1 to 4 (126), the divide follows the same read-one-early rule (294 at
k = 35), and the alias pair costs the same from a cached caller (763 against
766), which settles the assumption about jumping out of a streamed fill.

**Folded in** (PSoXide-emulator `silicon-timing-on-d366cd0`, unit-tested):

| Finding | Silicon | Emulator before |
|---|---|---|
| A coprocessor read waits for the running command, whichever register (`130`-`133`) | 638: 37 clocks a turn | 398: 22 |
| The write buffer: four entries, first write lands after 5 clocks, then one every 2 (`129`-`12D`, `1F`, `76`, `FA`) | store + 1 instruction 129, + 2 190, bursts of 2/4/8 190/192/223 | 192, 255, 254/254/254 |
| The load shadow: the third to sixth instruction behind a RAM load are free (`125`-`128`, `CE`) | 573, 573, 574, 579, 699 for 2, 3, 4, 6, 8 behind | 582, 646, 710, 838, 966 |

This corrects the SCPH-9902 note above: reads do wait. What a read of MAC0 or
LZCR *returns* can still be stale, which is what that capture actually showed
and what the conformance cases pin; the emulator decides that by instruction
count and is unchanged there.

Summed error against this A/B capture, GPU records aside: 2,688 cycles before,
620 after, of which 257 is the one known gap below. The standing spin-loop
records, which no change here targeted, fell into line with it:
`spin_4096_system` 41,621 to 37,461 against 37,458 on silicon. They are loops
around a volatile store, and the emulator had been overcharging every store
that had work behind it, which is most stores in real code.

**Still not modelled.** RAM loads during a linked-list DMA are half again as
slow (`FE` 770 against 510), where register-only code barely notices. And the
older cold store records (`0B`, `50`, `51`) read about 110 cycles higher on
silicon than the emulator now gives: an I-cache refill and a draining write
buffer interact on the bus in a way nothing here measures directly.

**GPU, first trustworthy numbers** (`100`-`114`, DMA list to GP0(1Fh)
interrupt, opaque textures; sixteen 32x32 primitives unless noted). The
emulator has no draw-time model and reads about 20 for all of them.

| Batch | Cycles | Reads as |
|---|---|---|
| flat triangles | 5,613 | the floor |
| Gouraud / textured / Gouraud-textured triangles | 9,870 / 9,952 / 10,067 | anything but flat costs about the same |
| raw texture, dithered Gouraud | 9,841, 9,961 | no measurable difference |
| translucent textured / flat | 10,454 / 9,461 | blending is cheap on textured, costly on flat |
| flat / 4bpp / 8bpp rects (1,024 px each) | 9,552 / 10,047 / 10,022 | rects are not faster per pixel than triangles here |
| 8bpp rects, a CLUT change on each | 14,214 | +42%: about 260 cycles a reload, as nocash has it |
| textured triangles, a texture page change on each | 16,836 | +69%: about 430 cycles a change |
| UV span 63 instead of 32 | 9,841 | no texture-cache penalty at this size |
| triangles clipped away entirely | 804 | 50 cycles each: leaving culling to the GPU is nearly free |
| letterboxed display range | 9,939 | no gain |
| VRAM fill, 32x32 | 3,720 | 2.6 times faster than a flat rect |
| VRAM copy, 32x32 | 31,814 | 3.3 times slower than a textured rect |
| 64 two-pixel triangles: flat / textured / Gouraud-textured | 2,817 / 8,589 / 17,231 | setup of about 44 / 134 / 269 cycles a triangle (nocash: 10 / 100 / 250) |

For the engine: sorting by texture page and CLUT inside a depth bucket is worth
real GPU time, flat-textured in place of Gouraud-textured halves the setup of
a small triangle, and GPU-side clipping is cheap enough that CPU culling only
pays for the packet and the transform it saves.


## 2026-09-23: hwtest v1.24 full characterisation, launch PAL console

`docs/hardware-refs/px8-silicon-2026-09-23-v1.24-full.txt`, all six PX8 pages
recovered from `~/Movies/2026-09-23 08-31-55.mov` with `hwtest-video-qr.py`;
image PSoXide-editor `hwtest/perf-probes` 691680b1, same console as the v1.22
and v1.23 captures (BIOS 2.2, 1995-12-04), NTSC video. Compare with
`python3 tools/hwtest-report.py --baseline <emulator pages> <that file>`.
Every number below is from that capture unless it is labelled as the emulator.

### Interrupts land on GTE commands, and returning to EPC runs them twice

Cases `0xC8`-`0xCB` loop over RTPS under Timer 2 and VBlank interrupts, with
psx-rt's handler returning to EPC and then with psx-spx's fix (return to EPC
+ 4 when the word at EPC is a GTE command). Silicon: 38 interrupts had EPC on
an RTPS and all 38 ran it twice (`0xC9` FAIL, 38 doubled, none lost); with
the fix 61 were stepped over, none doubled or lost (`0xCB` PASS). So the
command has already executed when the interrupt is taken with EPC on it.
Not measured: a GTE command in a branch delay slot, which the fix cannot
cover.

The emulator deferred every such interrupt past the command (0 hits in about
2,900 interrupts). PSoXide-emulator `c7e3ea8` now takes it DuckStation's
way: an interrupt raised during the instruction before a GTE command is taken
after the command, with EPC on it. Emulator after: `0xC8` 72 hits, `0xC9`
FAIL with 72 doubled, `0xCA` 62 skips, `0xCB` PASS. The kernel handler and
the HLE interrupt path step over the command, as the retail BIOS does.

### GPU DMA channel 2 stays busy while the list draws

Cases 211-226 kick four linked lists ending in GP0(1Fh) and stamp CHCR
clearing, GPUSTAT bit 24 (the 1Fh), and the start of the final high run of
bits 28 and 26 (clocks from the kick):

| List | CHCR | 1Fh | bit 28 | bit 26 |
|---|---:|---:|---:|---:|
| empty (16 empty nodes) | 199 | 199 | 18 | 18 |
| cheap (16 two-pixel Gouraud triangles) | 2,996 | 2,996 | 2,996 | 2,996 |
| expensive (16 half-screen Gouraud triangles) | 586,354 | 625,348 | 586,354 | 625,348 |
| packed (the expensive triangles, 24-word nodes) | 275,124 | 314,075 | 275,124 | 314,075 |

CHCR stays busy until the last packet is in the GPU, so the FIFO DMA model
is the right one and the word-count model (CHCR clear at 283 clocks for
cheap and expensive alike, 1Fh 6 clocks after the kick) is not. Every
overlap gain measured only under the word-count model (hl-psx's one-list
overlay chain, +21%) does not exist on silicon. Bit 28 comes back with CHCR,
one large primitive (38,994 clocks) before the drawing ends, while bits 24
and 26 follow the drawing: bit 28 is not a drawing-complete test. The CPU
keeps its speed during a drawing-bound walk (cases 228-233: ALU loop 29.01
clocks an iteration walking against 28.97 idle, RAM loop 76.83 against
76.77).

Emulator: the FIFO model is the default since `d684f43`
(`PSOXIDE_EXPERIMENTAL_DMA_FIFO=0` for the old model), and `561bd2c` lets the
final 1Fh leave the FIFO while the last primitive draws, which brings bit 28
back with CHCR. Before (0d3f4c9, word-count default) and after (561bd2c):

| Case | Silicon | Before | After |
|---|---:|---:|---:|
| 211 empty CHCR | 199 | 225 | 226 |
| 216 cheap 1Fh | 2,996 | 6 | 2,968 |
| 217 cheap bit 28 | 2,996 | 4,906 | 2,814 |
| 219 expensive CHCR | 586,354 | 283 | 585,656 |
| 220 expensive 1Fh | 625,348 | 6 | 624,731 |
| 221 expensive bit 28 | 586,354 | 314,366 | 585,656 |
| 222 expensive bit 26 | 625,348 | 314,366 | 624,731 |
| 224 packed 1Fh | 314,075 | 19 | 624,773 |
| 227 packed pixels | PASS | PASS | PASS |

### Large fills cost twice what the emulator charged, tiny primitives less

The expensive list draws about 39,084 clocks a half-screen Gouraud triangle
(38,120 px, about 1.02 clocks a pixel) where the FIFO model gave 19,574; the
cheap list's sixteen tiny triangles take 2,996 where it gave 4,937. Together
with the v1.23 batches (records `100`-`114` above) this fits one shape per
primitive: the longer of its setup and its fill, the fill being a per-pixel
rate plus a per-scanline term. Setup overlaps the fill, which is why a 32x32
Gouraud-textured triangle costs no more than a textured one although its
setup is twice as long.

| Term | Clocks |
|---|---|
| setup: flat / textured / Gouraud / Gouraud-textured | 44 / 134 / 195 / 269 |
| flat pixel (triangles and all rectangles) | 0.53 |
| interpolated pixel (Gouraud or textured triangle) | 1.07 |
| per covered scanline | 2.23 |
| untextured semi-transparent pixel floor, and its scanline term | 0.78, 5.96 |
| GP0(02h) fill start / VRAM copy start | 149 / 608 |

Emulator 561bd2c against silicon (the fit landed in `eca4677`); "before" is
0d3f4c9 with the word-count default, whose 1Fh fires 6 clocks after the
kick:

| Record | Silicon | Before | After | Residual |
|---|---:|---:|---:|---:|
| 100 flat triangles | 5,613 | 14 | 5,539 | -1.3% |
| 101 Gouraud | 9,870 | 14 | 9,959 | +0.9% |
| 102 Gouraud dithered | 9,959 | 14 | 9,959 | 0.0% |
| 103 textured | 9,936 | 14 | 9,959 | +0.2% |
| 104 raw texture | 9,841 | 14 | 9,959 | +1.2% |
| 105 textured translucent | 10,424 | 14 | 9,959 | -4.5% |
| 106 Gouraud-textured | 10,067 | 14 | 9,959 | -1.1% |
| 107 flat translucent | 9,467 | 14 | 9,465 | 0.0% |
| 108 flat rects | 9,552 | 14 | 9,894 | +3.6% |
| 109 / 10A 4bpp / 8bpp rects | 10,047 / 10,036 | 14 | 9,894 | -1.5% / -1.4% |
| 10B 8bpp rects, CLUT alternating | 14,214 | 14 | 9,894 | -30.4% |
| 10C textured, page alternating | 16,836 | 14 | 9,959 | -40.8% |
| 10D UV span 63 | 9,841 | 14 | 9,959 | +1.2% |
| 10E clipped away | 804 | 14 | 742 | -7.7% |
| 10F letterboxed | 9,939 | 14 | 9,959 | +0.2% |
| 110 VRAM fill | 3,720 | 14 | 3,732 | +0.3% |
| 111 VRAM copy | 31,814 | 14 | 31,656 | -0.5% |
| 112 / 113 / 114 two-pixel flat / textured / Gouraud-textured | 2,817 / 8,589 / 17,231 | 14 | 2,848 / 8,620 / 17,252 | +1.1% / +0.4% / +0.1% |
| 216 cheap list 1Fh | 2,996 | 6 | 2,968 | -0.9% |
| 220 expensive list 1Fh | 625,348 | 6 | 624,731 | -0.1% |

Not modelled: CLUT reloads (10B, about 260 clocks each) and texture-page
changes (10C, about 430). Both look like cache refills, and so does the
largest remaining gap: ps1-tests `gpu/bandwidth` (third-party silicon,
full-screen primitives, converted at 2,172 clocks an HBlank) measures a
textured quad sampling a 256-texel-wide 15bpp texture at 2.82 clocks a pixel
and a sprite spanning 320 texels (draw-mode default depth, taken as 4bpp) at
1.07, where this model charges 1.08 and 0.54. Estimated per 8-byte cache
line refilled, the two and 10C all come out at roughly 7 to 9 clocks. A
texture-cache miss term is the next step; until then large textured spans
that miss the cache are too cheap in the emulator. The same third-party numbers for flat fills (rectangle,
quad, clipped quads, semi-transparent rectangle and quad) agree with this
model within 2%.

### Nodes larger than the FIFO very likely lose words, but not the 1Fh

The packed list sends the expensive triangles four to a node (24 words, over
the 16-word FIFO). Silicon finished it at 314,075 clocks, about 8 of the 16
triangles' worth, with the same one-triangle drain after CHCR as the
expensive list (38,951), so roughly half of the drawing never happened. Its
1Fh still arrived, and case 227 passed only because each half's sampled
pixels come from the last triangle drawn there. Which words are lost is not
established (next burn: give each packed triangle its own region and hash
them all). The emulator's FIFO model dropped words, lost packet sync and
swallowed the 1Fh (224 "never", 227 FAIL); since `b9bcc79` the channel waits
for room instead and loses nothing (224 at 624,773, 227 PASS), and
`PSOXIDE_GPU_DMA_OVERFLOW=drop` keeps the old rule, which reproduces the
Celeste 0.2.3 corruption.

### RAM loads slow down during an empty-list walk; the scratchpad does not

Records `137`-`13A` time one compiled call (515 stack loads, 899 stores) on
a RAM stack and a scratchpad stack, idle and while channel 2 walks a
2,048-node empty list: 13,618 / 10,806 idle, 21,249 / 11,084 during the walk
(emulator 561bd2c: 13,958 / 10,762 and 13,939 / 10,722). The RAM stack slows 56%,
the scratchpad 2.6%. The scratchpad premise holds on silicon: 5.46 clocks
saved a stack load idle (emulator 6.2 to 6.3), 48% of the call during the
walk.
A drawing-bound walk slows nothing (above), so the cost is the DMA actually
moving words. Not modelled: with `FE` (770 against 510 idle for 64 loads, +4
clocks a load) and 139 (+14.8 clocks a stack load, with stores interleaved)
the per-load penalty is not one number yet.

### The CPU loop around I/O reads is slower than modelled

Cases 228/232 idle: 28.97 clocks an iteration on silicon against 24.97 in
the emulator; the RAM loop (230) 76.77 against 73.98. Beyond the loop body
these loops read DMA CHCR (1F8010A8h) and the Timer 2 counter (1F801120h)
back to back, and no record calibrates those two reads yet. Not modelled;
v1.25 wants warm probes for both.

### Timer 1's VBlank reset sits 26 lines after the interrupt

Case 206 reads Timer 1 (HBlank clock, sync mode 1) in the VBlank handler at
each of 119 present-queue flips: 237 every time, no spread. On a 263-line
field that puts the reset 26 lines after the VBlank interrupt (the SCPH-9902
profile measured 29). The emulator read 240 or 0: a counter read's hold
window that covered the VBlank edge made the lazy catch-up skip that field's
reset. PSoXide-emulator `8eae633` keeps the reset through the hold and uses
26 lines as the default phase. Case 206 before 0x000000F0 (latest 240,
earliest 0), after 0x00D200ED (latest 237, earliest 210, the earliest being
a flip whose count started at the probe's own mode write). The 0x1E2 (482)
in the capture's own emulator run did not reproduce with the standard
recipe on 0d3f4c9.

### Present queue

Case 204 on video: 120 of 120 frames presented, no tear, no missing
triangle, all flips at one beam line, display cadence exactly as the chain
costs predict (207: 30 two-vblank frames). Case 205 passed only because no
chain ended inside the window where bit 28 is already high and the drawing is
not done; a game's varying frame cost will land there. Flip on GP0(1Fh) and
GPUSTAT bit 24, not bit 28.

### SPU conformance: the v1.22 outcome, unchanged in v1.24

The v1.22 full capture (`px8-silicon-2026-09-17-v1.22-full.txt`) answered
the four questions posed under "SPU RAM uploads do not land" above, and
v1.24 repeats it on the same console: `0xA6` (DMA round trip) PASS,
`0xBC` (read is repeatable) PASS, `0xA7` (manual-FIFO round trip) FAIL
0x09FA4EF2 both times, `0xBD` (DMA and FIFO writes agree) FAIL 0xA0A10634
both times, `0xBE` (address written after the mode) FAIL and `0xBF` (four
small blocks) FAIL with hashes that change between runs (v1.22
0xDB9A456D / 0x53C8E737, v1.24 0x38FD746D / 0xB0526890). So on this console
the read path is stable and the DMA upload round-trips (the "0xA6 still
FAILs" of the v1.17 section above no longer holds), the manual-FIFO write is
what is wrong, and neither reordering the address write nor splitting the
block fixes it. The emulator passes all six, so it is the permissive one.
`0x8B` still fails only in the emulator (the NCLIP gap above).

### Consequence for measured game numbers

Emulator timing changes move every game's numbers, so the perf tracker needs
a re-baseline on the new emulator. Same images, same inputs, PSoXide-emulator
frontend 0d3f4c9 (which reproduces the tracker's frozen frontend exactly on
Cortex) against 561bd2c:

| Game | Before | After | After, `PSOXIDE_EXPERIMENTAL_DMA_FIFO=0` |
|---|---:|---:|---:|
| Cortex 0.4b tracker disc, polls 1410-5340 | 25.564 fps | 22.147 fps | 25.689 fps |
| Quake E1M1 chain bench disc (exe 25ab8b66) | 26.793 fps | 27.180 fps | 27.180 fps |

Cortex does the same work (RAM, I-cache and multiply stalls unchanged over
the window) and spends 275M more issue cycles and 121M more MMIO stall
cycles polling: it waits for the channel-2 walk, which under the FIFO model
lasts as long as the drawing. Its GPU busy share falls from 49.8% to 21.3%
with the new draw costs, because its textured triangles are cheaper than the
old 2.8 clocks a pixel.
