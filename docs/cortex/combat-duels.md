# Dual-stance combat duels

Open Graybox Reach in Play. **Hold Select and tap L2** to reset the encounter and give Aletha an AI controller. Both Horizon and Zenith are enabled. Press any physical button after releasing the start chord to take control. The result pauses the fight at death, a three-minute limit, or thirty seconds without damage. Start a fresh duel by leaving the result and pressing the chord again.

This is a training capability on flagged encounters. It does not change saved health, weapons, damage, animation timing or the scene. It requires a ranged-equipped player and a ranged-capable training opponent. Graybox Reach currently has one such opponent. Other actors are not disabled; use an isolated encounter when comparing results.

## Shared policy and normal combat

`psx-game-runtime::combat_policy` supplies stance distance hysteresis, switching to recover a weak active pool, pressing after repeated shots, Energy-aware stance selection, and the existing light/heavy weighting and repetition cap. Tactical NPCs and the player bot call these same functions. NPC steering and the player's controller adapter remain separate.

The player adapter observes visible positions and animation state every twelve simulation ticks. It retains the last seen position when sight is blocked. It presses R3 for normal lock acquisition, R2 for direct ranged attacks, R1/R2 for melee, Triangle for stance changes, and taps Circle for dodge. Movement goes through camera-relative stick conversion and the normal collision motor. Cooldowns, readiness, stamina, hitboxes, poise and animation permissions retain authority. The adapter does not set health, trigger damage directly, teleport during a fight, grant invulnerability, or bypass action locks.

Movement follows the intended weapon phase during a stance swap, so changing weapon does not require an artificial stop. Ranged escape has hysteresis: start below the near edge, settle in the middle of the firing band. Every 24 ticks, short width-aware point probes check the retained retreat lane and alternatives; the normal body motor still resolves movement. An obstructed lane or a three-second failed escape leads to the shared melee response. These probes are local steering, not navigation or a guarantee of camera visibility.

The enemy receives the player's visible stance and vitality for the shared decision policy; hidden targets do not supply that policy observation. Legacy callers without an observation retain distance-based stance choices. The prototype chooses by distance, resources and recent openings. Opposite colour adds damage but does not force either actor to switch.

## MCP

`combat_duel {"seed":1,"polls":11400,"skip_build":false}` builds the saved project and runs a real PS1 disc. Use `skip_build:true` after building the current code. Seeds are 1..255 except 128, which is reserved for the centered physical controller's default seed 1. The input tape encodes the seed in the right stick only while starting; it is never used as combat camera input.

The result includes:

- Winner, double KO, timeout, no-damage stall, manual takeover or incomplete launch.
- Time and switches in each stance for both actors, including reasons.
- Observed damage to both vitality channels and damage-event counts.
- Requested player attacks versus actual animation starts; enemy light/heavy/ranged windups.
- Approach, retreat and ranged decision counts, plus coverage warnings.
- Distance occupancy and completed close-far-close excursions, separate from stance changes.
- Completed stance visits with no attack and enemy movement-goal changes, to expose wasted switches and repeated replanning. Initial selection and the unfinished last stance visit are excluded.
- Sampled Energy for both actors and timed shot-interruption events. See [combat flow](combat-flow.md) for tuning and controls.
- A directory under `validation/combat-duels/` containing the input tape, full guest log, JSON trace and final image.

## Batches

`combat_duel_batch {"seeds":"1-20","parallel":2,"scenario":"graybox"}` replays a seed set (at most two emulators at once) and writes `batch.json` and `batch.md` under `validation/combat-duels/batch-<scenario>-<stamp>/`, next to one directory per seed. The same code runs from the shell, which is how baselines are taken:

```
cargo run --release -p psxed-mcp --example duel-batch -- \
  --project editor/projects/graybox-reach --frontend target/release/frontend \
  --seeds 1-20 --parallel 2 --scenario graybox --out <dir> --label <name> [--skip-build] [--polls 11400]
```

Build the frontend from the same checkout: its repository root is fixed at compile time and it builds the disc from that tree.

Scenario `heavy` derives a sibling project (`editor/projects/zz-duel-heavy-<project>`, git-ignored) in which the Heavy Enemy resource replaces the light enemy. The authored layout is not changed. Its placement values (poise 50, 200 health per channel, non-tactical, training flag) are comparison starting points copied from the Cortex Ignition 0.5 placement.

Each duel adds a `metrics` object to its report; the batch aggregates every numeric metric (n, sum, mean, median, min, max; missing measurements are skipped, never zero):

- outcome, duration, player and enemy health left
- damage and hits by source: light, heavy, shot, claw light, claw heavy, cannon (health actually removed, so overkill is excluded)
- poise damage, poise breaks inflicted and suffered, enemy flinches (read from sampled state as a cross-check), opposite-colour hits both ways, and claw light hits that break a fresh player alone
- i-frame avoids: enemy swings that overlapped the player while invulnerable, and bolts about to cross the player (the bolt count is a look-ahead estimate)
- enemy sidesteps attempted and how many ended without a player shot landing
- stance swaps, Energy spent and gained, shots fired and hit

The contract in `batch.json` records the source revision and dirty state, seeds, polls, parallelism, disc and frontend sha256, build flags and the launch command. Two aggregates are comparable only when the contract matches except for the change under test. The guest logs `duel:event` and `duel:totals` lines for this; they are written only while a duel is active.

The complete trace remains in `report.json`; MCP returns its compact summary. Coordinates are signed world units. Ticks are fixed simulation ticks. Replay the saved tape against the same disc to reproduce decisions; record the disc hash when comparing builds. To re-analyse a saved log:

```
cargo run --release -p psxed-mcp --example duel-report -- path/to/guest.log
```

A death is **completion**, not a balance or flow pass. The report warns when an actor never used a stance, a vitality channel never took damage, or the player never fired. Long chip-damage loops can reach the time limit without triggering the no-damage watchdog. Damage totals measure observed health loss; they are not raw authored damage or exact hit/miss attribution. AI samples visible commitments, but it is a test controller, not a model of human skill.

Duels log only while enabled and add no geometry. Normal Play uses the same shipping build, with the controller inactive until requested. Hardware frame cost has not been measured. Incremental compilation was disabled during verification to avoid recreating the debug caches removed during disk cleanup.
