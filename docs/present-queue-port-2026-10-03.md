# Present queue: Quake's VBlank-kicked presentation, moved into the shared SDK (2026-10-03)

## Why

Cortex's stress runs (docs/engine-stress-limits-2026-10-03.md) spend 3-19% of every frame idle
outside any work. With one enemy on screen, the work is 1.39M cycles but the frame takes 1.71M: the
CPU finishes, then waits for the next VBlank edge before frame N's GPU work can even start.
Today, frame N's chain is kicked only after:
1. frame N-1 has finished drawing (`draw_sync`),
2. the HUD has been drawn on N-1 with immediate GP0 commands,
3. N-1's flip has landed,
4. N's draw target has been set and its buffer cleared.

quake-psx removed exactly this serialisation in its own `game/src/platform.rs`
(3ca915c, 16e27a9). It gained +25.5% on the E1M1 chain and +46.1% on the monster route,
against its old blocking path. Per the shared-engine rule, the mechanism moves into the SDK, and
Quake switches to it and deletes its copy.

## Design

### SDK (PSoXide)

1. **Present queue, psx-rt.** The queue is a one-slot chain head plus a GP1(05h) word. psx-rt's
   own VBlank path, still using only `$k0`/`$k1`, handles it on any edge where both hold:
   - GPUSTAT bit 24 is set: the previous chain's closing GP0(1Fh) has run;
   - DMA channel 2 is idle.

   On such an edge it:
   1. writes the flip word (which shows the previous, finished frame);
   2. acknowledges the flag with GP1(02h);
   3. sets the DMA direction with GP1(04h)=2;
   4. enables channel 2 in DPCR;
   5. kicks the chain;
   6. clears the slot.

   It counts kicks and skipped edges. The API is:
   - `start()`, which raises the flag once at start-up;
   - `publish(head, display)`;
   - `slot_full`, `wait_slot_empty`, `wait_arena_free`;
   - a bounded stall recovery after 8 edges, counted.

   With an empty slot the handler pays one load.
2. **GP0 capture, psx-io.** While recording, `write_gp0` appends to a caller buffer laid out as
   DMA linked-list nodes, each a header plus at most 16 payload words. Silicon v1.24 showed that
   nodes over 16 words lose words. `wait_cmd_ready` and the other ready-waits return immediately
   while recording. Every psx-gpu immediate function, and so every HUD helper, can then be
   recorded unchanged. GP0 image uploads are not allowed inside a recording.
3. **`OrderingTable::end_with_chain(head)`, psx-gpu.** This links slot 0 to an arbitrary chain:
   the recorded HUD, which itself ends on `DRAW_DONE_NODE`.

### Engine (PSoXide-editor, `psx-engine` feature `present-queue`)

Per visual frame N:
1. Call `wait_arena_free()`: wait only while N-1 is still queued and channel 2 is busy, meaning
   N-2's walk is still running. This keeps the paired packet arenas' "one frame in flight" rule.
2. `scene.render(ctx)` builds N's ordering table, as now.
3. Record the preamble: N's draw target and clear, ending on a link to the ordering table's head.
4. Record the HUD (`render_overlay`) and link the ordering table's slot 0 to it.
5. Call `wait_slot_empty()`, then `publish(preamble head, display word of N-1)`. The CPU goes
   straight on to the next simulation ticks and frame.

At the edge where the handler kicks N, the display shows N-1. N draws into the other buffer, and
N+1 draws into the buffer that just left the screen.

Immediate GP0 writes outside a recording (texture uploads) must first wait for the queue and the
GPU to be idle.

### Quake

`platform.rs` calls the SDK queue, and its own handler and statics are deleted. This needs a
re-pin onto the new SDK.

## Validation

- **Emulator, both DMA models** (default, and `PSOXIDE_EXPERIMENTAL_DMA_FIFO=1`):
  - Cortex: stress scenarios e1, fix_melee2, base and rsmall.
  - Quake: the E1M1 chain bench. It must not regress against its own queue.
  - Images: per-checkpoint frames must match the blocking build one checkpoint later, as in
    Quake's acceptance.
- **Console:** a burn and recorded run on Manny's PS1 before anything ships.
