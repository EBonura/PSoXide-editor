# Single-enemy behaviour study - Graybox Reach

Graybox Reach now cooks exactly one light Rust Mantis, using the approved body
and `light_walk_v5` animation. The cannon and the existing light/heavy attacks,
stance restrictions, hit windows and recovery animations are retained. Other
projects retain the previous behaviour unless **Tactical behaviour** is enabled
on their enemy controller.

## Play the repeatable encounter

Open `editor/projects/graybox-reach/project.ron` and run its playtest, or launch
`editor/projects/graybox-reach/baked/graybox_reach.cue`.

- **Select + L1:** reset the encounter, player/enemy health, positions, tactical
  random seed, combat state and animation epoch.
- **Select + R1:** reset into the blocked-route fixture. The same enemy is placed
  inside the small pen east of the arena; the player stands on its adjacent shelf.
  The pen walls exceed the motor's step height. This exercises real collision,
  wait/retry, reposition and home recovery without spawning a second enemy.
- The bottom-left overlay names the enemy's current movement/combat state.
- Let the enemy approach to inspect run-to-stalk transition and attack spacing;
  circle it to inspect turning and flanking; back away to inspect physical return
  home. Fight normally to inspect stagger, committed attacks and punish windows.

Training is explicitly enabled on this project's controller. The blocked-fixture
shortcut currently uses Graybox's authored coordinates; enable it in another map
only after providing equivalent geometry or changing those coordinates. The
runtime placement API rejects actors without the training flag. Normal combat
and damage remain active; reset is available after a defeat too.

## Behaviour and tuning

The tactical layer owns a persistent movement goal independently of the existing
combat state machine. Arrival, expiry, blockage and cancellation are distinct
outcomes; a wrapping generation counter makes repeated goals observable.

| Behaviour | Current tuning / contract |
| --- | --- |
| Alert | Existing 42-tick reaction, 0.7 seconds |
| Stalk / pursue | Walk 2, run 6 cooked units per simulation tick; enter run beyond 320 units, leave below 192 |
| Facing | 180 degrees/second; approach waits if more than 60 degrees off; attacks commit within 30 degrees |
| Attack selection | Distance-weighted light/heavy melee, no third consecutive identical melee; ranged remains stance/range gated |
| Commitment | Existing windup, authored active frames and 24-tick recovery; no spacing replan until recovery finishes |
| Close post-attack spacing | Within 64 units: 70% retreat, 20% circle, 10% hold |
| Middle post-attack spacing | Up to 192 units: 65% circle, 20% approach, 15% hold |
| Retreat | 3–4.5-second maximum lifetime, ends earlier at requested separation |
| Circle | Side selected once, 3.5–5-second maximum lifetime; cancels if target is lost or leaves its band |
| Cannon band | Approach separation 128, clamped to authored range/minimum; prevents minimum-range shuffling while stance is locked |
| Target memory | Initial sight/hearing location; later hidden motion uses remembered position; no hidden attack commitment |
| Disengagement | 4 seconds without sight, different room, or player beyond the 800-unit spawn leash |
| Home | Physically moves toward authored spawn; no teleport; 2-second reacquisition cooldown after arrival |
| Stuck detection | Physical travel and destination progress measured separately; 1-second no-travel or 3-second no-progress window |
| Recovery | 2-second wait, direct movement probe at 1 second, bounded lateral reposition; repeated failure escalates home |
| Steering | Existing bounded local collision steering; refresh every 0.3 seconds at either NPC cadence |

Distances are Cortex cooked world units. Timers use the 60 Hz simulation clock,
including when NPC logic runs every second tick. Per-actor tactical storage is
68 bytes on the host layout (4,352 bytes for a capacity of 64); policy adds no
heap allocation. No enemy model or animation triangles were added.

## What comes from the Bloodborne study

The clinic-beast investigation supports selecting attacks by distance, evaluating
spacing **after** an attack, retaining a chosen circle direction for seconds,
and separating movement goals from committed actions. The recovery investigation
supports independent route-query failure, physical progress checks, finite wait
and retry goals, and explicit goal outcomes. Relevant local research in
`bb-decomp/docs`:

- `movement-ai-policy-2026-10-05.md`
- `movement-obstacle-recovery-2026-10-06.md`
- `movement-goal-lifecycle-2026-10-06.md`
- `movement-cortex-contracts-2026-10-05.md`

Cortex adapts these contracts to its motor and scale. The weights and thresholds
above are Cortex tuning; they are not claimed to be a byte-for-byte Bloodborne
port. Its bounded local steering does not implement Bloodborne's navmesh routes
or every recovered failure policy. The sealed pen deliberately demonstrates the
bounded failure path when there is no route.

## Reproduce validation

From the repository root:

```sh
cargo test --manifest-path engine/Cargo.toml -p psx-game-runtime --lib entities::
cargo test -p psxed-project graybox_single_enemy_tactical_encounter_cooks_with_approved_walk
EDITOR_PLAYTEST_FEATURES='cd-stream-bench emulator-telemetry' target/release/frontend build-project-disc --project editor/projects/graybox-reach
python3 editor/projects/graybox-reach/tools/enemy_behavior/replay.py --run --case reset
python3 editor/projects/graybox-reach/tools/enemy_behavior/replay.py --run --case blocked
python3 editor/projects/graybox-reach/tools/enemy_behavior/replay.py --run --case circle
python3 editor/projects/graybox-reach/tools/enemy_behavior/replay.py --run --case retreat
python3 editor/projects/graybox-reach/tools/enemy_behavior/verify.py
```

Tapes use pad-poll clock v2, with a fixed reset after boot. Outputs go to
`validation/enemy-behavior`: actual guest telemetry, sampled screenshots, CSV
traces, summaries and a combined verification report. These run the cooked PS1
guest through the emulator's real input/motor/render path. Host scenarios also
cover hidden-target memory, restored movement, lateral sliding, interruption,
attack variation and 30/60 Hz timing. The cook regression asserts exactly one
enemy and the approved walk asset.

To leave a normal playable disc without diagnostic telemetry:

```sh
EDITOR_PLAYTEST_FEATURES='cd-stream-bench' target/release/frontend build-project-disc --project editor/projects/graybox-reach
```

Validation on 2026-10-06: **83 enemy tests passed**, the actual Graybox cook
regression passed, and four guest replays produced 836 diagnostic samples.
Reset matched all 75 paired checkpoints. The blocked case reached three retries
without crossing the pen; the retreat case recorded 30 home-movement samples
and finished idle within 12 units of spawn. All nine goal values and all three
attack clips were observed. The guest hazard scan reported zero remaining
hazards and all scratchpad stack guards passed.

Current broader-suite limitations: two existing project tests reference the
removed `quake-e1m1-geometry` project. Strict all-dependency clippy also encounters
unrelated renderer/scarf lints. The targeted enemy tests, actual-project cook,
guest build hazard checks and scratchpad stack guards are the relevant checks
for this change. Console hardware performance has not been measured in this pass.

## Movement polish pass

The animation clock now advances from committed ground travel independently of
movement-goal lifetime. Replanning an unchanged gait preserves its phase. Partial collision movement
and diagonal travel advance the gait relative to its authored nominal speed.
The linked directional clips already use a slower 15 Hz sample rate, so their
50-percent spacing speed is treated as nominal rather than halving playback again. Stationary actors select idle/turn rather than
continuing a walking loop against a wall. The clock retains fractional ticks
and supports both NPC update cadences.

Post-attack circling now targets 96 cooked units (1.5 times the preferred
distance) instead of 64. Radial correction uses that goal's own separation.
The authored spacing-speed control governs lateral and backward movement;
the current light enemy uses 50 percent. Normal approach/run speed and authored
attack timing remain unchanged. The newer directional animation assets already
linked in the shared project are retained.

Four additional tests cover phase continuity, travel-based playback at both
cadences, diagonal distance and the wider circle. Together with the authored
spacing-speed test, the enemy suite passes 92 tests in this pass. The previous
video remains under `validation/enemy-behavior/video-review`; the same input
tape is used for `video-polish` so the behaviour can be compared.

The wider orbit exposed repeated cannon-only sequences. After two consecutive
cannon attacks, a nearby target now prompts a close approach once the stance
cooldown permits it. Short occlusion during a failed-route wait also retains
the existing four-second target memory for repositioning; it no longer returns
home immediately just because the target is briefly behind a pen wall.

Retry recovery now retains the best distance reached before a failure. A detour
followed by returning to the same wall does not reset the failure count. The
blocked replay runs for 66 seconds to include multiple complete waits and
reposition attempts at the slower gait, including escalation to return home.

Final polish verification: all 92 enemy tests passed; the four guest traces
contain 936 sampled states, cover every goal and all three attacks, and preserve
75 identical reset checkpoints. The extended blocked fixture escalates to three
retries and return-home. See `validation/enemy-behavior/video-polish` for the
recording, comparison and validation receipt.
