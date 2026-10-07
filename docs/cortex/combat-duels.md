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

The complete trace remains in `report.json`; MCP returns its compact summary. Coordinates are signed world units. Ticks are fixed simulation ticks. Replay the saved tape against the same disc to reproduce decisions; record the disc hash when comparing builds. To re-analyse a saved log:

```
cargo run --release -p psxed-mcp --example duel-report -- path/to/guest.log
```

A death is **completion**, not a balance or flow pass. The report warns when an actor never used a stance, a vitality channel never took damage, or the player never fired. Long chip-damage loops can reach the time limit without triggering the no-damage watchdog. Damage totals measure observed health loss; they are not raw authored damage or exact hit/miss attribution. AI samples visible commitments, but it is a test controller, not a model of human skill.

Duels log only while enabled and add no geometry. Normal Play uses the same shipping build, with the controller inactive until requested. Hardware frame cost has not been measured. Incremental compilation was disabled during verification to avoid recreating the debug caches removed during disk cleanup.
