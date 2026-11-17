# Dual-stance combat duels

Open Graybox Reach in Play. **Hold Select and tap L2** to reset the encounter and give Aletha an AI controller. Both Horizon and Zenith are enabled. Press any physical button after releasing the start chord to take control. The result pauses the fight at death, a three-minute limit, or thirty seconds without damage. Start a fresh duel by leaving the result and pressing the chord again.

This is a training capability on flagged encounters. It does not change saved health, weapons, damage, animation timing or the scene. It requires a ranged-equipped player and a ranged-capable training opponent. Graybox Reach currently has one such opponent. Other actors are not disabled; use an isolated encounter when comparing results.

## Shared policy and normal combat

`psx-game-runtime::combat_policy` supplies stance distance hysteresis, pressing after repeated shots, Energy-aware stance selection, and the existing light/heavy weighting and repetition cap. Tactical NPCs and the player bot call these same functions. NPC steering and the player's controller adapter remain separate.

The player adapter observes visible positions and animation state every twelve simulation ticks. It retains the last seen position when sight is blocked. It presses R3 for normal lock acquisition, R2 for direct ranged attacks, R1/R2 for melee, Triangle for stance changes, and taps Circle for dodge. Movement goes through camera-relative stick conversion and the normal collision motor. Cooldowns, readiness, stamina, hitboxes, poise and animation permissions retain authority. The adapter does not set health, trigger damage directly, teleport during a fight, grant invulnerability, or bypass action locks.

Movement follows the intended weapon phase during a stance swap, so changing weapon does not require an artificial stop. Ranged escape has hysteresis: start below the near edge, settle in the middle of the firing band. Every 24 ticks, short width-aware point probes check the retained retreat lane and alternatives; the normal body motor still resolves movement. An obstructed lane or a three-second failed escape leads to the shared melee response. These probes are local steering, not navigation or a guarantee of camera visibility.

The enemy receives the player's visible stance for the shared decision policy; hidden targets do not supply that policy observation. Legacy callers without an observation retain distance-based stance choices. The prototype chooses by distance, resources and recent openings. Opposite colour adds damage but does not force either actor to switch.

## MCP

`combat_duel {"seed":1,"polls":11400,"skip_build":false}` builds the saved project and runs a real PS1 disc. Use `skip_build:true` after building the current code. Seeds are 1..255 except 128, which is reserved for the centered physical controller's default seed 1. The input tape encodes the seed in the right stick only while starting; it is never used as combat camera input.

The result includes:

- Winner, double KO, timeout, no-damage stall, manual takeover or incomplete launch.
- Time and switches in each stance for both actors, including reasons.
- Observed damage to each actor's one health bar and damage-event counts. A log from before the one health bar (2026-11-16) reports each actor's two pools summed.
- Requested player attacks versus actual animation starts; enemy light/heavy/ranged windups.
- Approach, retreat and ranged decision counts, plus coverage warnings.
- Distance occupancy and completed close-far-close excursions, separate from stance changes.
- Completed stance visits with no attack and enemy movement-goal changes, to expose wasted switches and repeated replanning. Initial selection and the unfinished last stance visit are excluded.
- Sampled Energy for both actors and timed shot-interruption events. See [combat flow](combat-flow.md) for tuning and controls.
- A directory under `validation/combat-duels/` containing the input tape, full guest log, JSON trace and final image.

The complete trace remains in `report.json`; MCP returns its compact summary. Coordinates are signed world units. Ticks are fixed simulation ticks. Replay the saved tape against the same disc to reproduce decisions; record the disc hash when comparing builds. To re-analyse a saved log:

```
cargo run --release -p psxed-mcp --example duel-report -- path/to/guest.log
```

To run several seeds against one disc and keep the numbers, use the sweep example. It builds the disc once (or reuses the newest with `--skip-build`), records the disc and frontend SHA-256, replays each seed and writes `baseline.json` next to the per-seed artifacts:

```
cargo run --release -p psxed-mcp --example duel-baseline -- \
    --frontend target/release/psoxide-emulator --project projects/grid-dungeon --seeds 1,2,3,4,5,6,7,8
```

A death is **completion**, not a balance or flow pass. The report warns when an actor never used a stance, an actor never took damage, or the player never fired. Long chip-damage loops can reach the time limit without triggering the no-damage watchdog. Damage totals measure observed health loss; they are not raw authored damage or exact hit/miss attribution. AI samples visible commitments, but it is a test controller, not a model of human skill.

Duels log only while enabled and add no geometry. Normal Play uses the same shipping build, with the controller inactive until requested. Hardware frame cost has not been measured. Incremental compilation was disabled during verification to avoid recreating the debug caches removed during disk cleanup.

## Host duels (any two fighters, any controller)

The disc duel is always the player (driven by the duel bot) against the training enemy (driven by the enemy AI), because the player body still lives in the playtest scene while the enemy AI drives entity slots. The SDK's `tools/psoxide-duel` fights on the host instead, without a disc build: each fighter is an entity of its own table, driven by the enemy tactical AI or by a scripted pad through the possession seam (`FighterWorld::controller_intent`), on the game's own rules, motor and projectile pool. Its fighters come from the project's own cook: the training enemy as authored, and Aletha cooked as an entity in the enemy's place (her Character, controller settings, stats and action clips; the enemy's AI persona). Contact uses the authored hitbox windows on each clip's clock, but the arena is a plain walled room (optionally with pillars), not the level, so a host fight is not the disc fight of the same seed.

```
psoxide-duel sweep --project projects/grid-dungeon --seeds 200 --out DIR
psoxide-duel sensitivity --project projects/grid-dungeon --seeds 500 --out DIR
psoxide-duel log --project projects/grid-dungeon --seed 1 --matchup aletha-ai:enemy-pad > host.log
cargo run --release -p psxed-mcp --example duel-report -- host.log
```

`log` writes the same `duel:` lines as the disc (side A in the player's columns, side B in the enemy's), so the summariser and `duel-report` read host logs unchanged. Run the SDK build of the tool from the editor checkout (projects are found under `projects/`); its symmetry tests read `PSOXIDE_PROJECTS_DIR`.

## Entity against entity (disc)

The player's body and fighter now live in the last slot of the entity table (the actor slot: `GameEntities::ACTOR_SLOT`, read through `actor_motor` and `actor`), and an entity can fight another entity: `GameEntities::set_rival(index, Some(slot), team)` points its tactics at that slot instead of the player, puts it on a side of its own for bolts (`CombatTeam::Rival` against `Enemy`), and lets it hold its own attack slot (the shared director only arbitrates the fighters that engage the player). The game resolves its melee token against the rival with the same pose-based contact the player's defence gets (authored capsules on both poses, the legacy arc only when a side has none, world occlusion, the hitbox window mask) and lands it through the rival's own hit path; a bolt hits any fighter on another side, and a parry staggers the attacker or the shooter. A spawn clears every rival.

On the disc: open `projects/grid-rivals` (grid-dungeon with a second training enemy in sight of the first), **hold the right stick up while pressing Select and L2**. The two training entities fight each other and the player is left on her own pad. The log carries `duel:rivals A B`, then `duel:rival-sample` rows (tick, A x/z, B x/z, health A/B, stance A/B, state A/B, energy A/B) and the usual `duel:event` and `duel:end` lines, actor 0 being A. The end codes are those of the player duel with A in the player's place (1: A won, 2: B won, 3: both down, 5: time, 6: thirty seconds without damage).

What an entity reads of its rival differs from what it reads of the player in one place: the rival's attack phase and the ticks to its swing come from its state clock and the record's windup, where the player's come from her pose. That gap goes when every fighter runs the one action system.
