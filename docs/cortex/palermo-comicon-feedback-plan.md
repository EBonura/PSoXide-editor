# Cortex Ignition — Palermo Comicon combat feedback plan

This is a planning document for the next combat-polish pass. It records the
feedback gathered from the Palermo Comicon demo and maps each item to the
current runtime. It does not copy code from external projects; the external
projects are architecture and gameplay references only.

## Current findings

### 1. Enemy death dissolve — performance investigation and optimisation (P0)

The dissolve is implemented in
`engine/crates/psx-game-runtime/src/model_rendering/death_dissolve.rs`.
`draw()` currently walks every face, unprojects every triangle, computes a
height delay, optionally rotates/translates it, reprojects it, changes the
material, and submits it. The effect lasts five seconds after the death clip.
Moved fragments disable culling, so the cost is paid by every visible corpse.

There is an existing diagnostic receipt under
`editor/projects/cortex-ignition-tech-demo-0.4b/review/death-dissolve/` showing
only a two-death capture, a 2.64% peak overhead, and no physical-console
verification. That result is not enough to explain the Comicon report.

Plan:

1. Add a deterministic stress capture with one, two, and three simultaneous
   visible corpses, and record submitted/dropped triangles, ordering-table
   usage, frame bus cycles, and worst frame gap on emulator and hardware.
2. Keep the current five-second visual as the reference, then test shorter
   durations and a bounded fragment budget. The duration and budget must be
   authored knobs rather than scattered constants.
3. If geometry remains the bottleneck, precompute a compact per-face height
   bucket/index during model load and avoid per-frame unprojection for faces
   that have not entered the dissolve window. Preserve the top-to-bottom read
   and deterministic motion.
4. Add a multi-corpse budget/fallback: once the configured triangle budget is
   exhausted, retain the pose for that frame and skip low-priority fragments,
   rather than overflowing the primitive arena or stalling the frame.

Acceptance: no guest faults or primitive overflows; a documented hardware
frame-time target with one and multiple corpses; the visual remains a clear
top-down dissolution.

### 2. Locked attack facing after a dash (P0)

The likely cause is in
`engine/examples/editor-playtest/src/playtest_update.rs`: the lock target yaw
is calculated before attack input, but `update_attack_input()` then replaces
the motor input with `CharacterMotorInput::default()` when an attack starts.
The motor therefore never receives the locked target yaw at attack commit.
Locked evades intentionally preserve body facing while changing the travel
direction, which makes this failure especially visible after passing the enemy.

Plan:

1. At attack start, snapshot the live hard-lock bearing and commit it to the
   player's combat-facing yaw before the first attack pose is sampled.
2. Keep free movement and unlocked attacks unchanged; only a valid hard lock
   may override attack facing.
3. If the target disappears between input and commit, retain the current yaw
   and never turn toward stale coordinates.
4. Add a deterministic regression test: lock, evade past the target, press R1/R2
   on the first available tick, and assert the authored melee arc/capsule uses
   the target-facing yaw and connects when otherwise in range.

The clean separation to preserve is the same one used by the public reference
projects: input selects an action, then the action owns a committed facing and
active/recovery window.

### 3. Melee weapon range (P1)

Player melee currently resolves authored combat capsules first and falls back
to the legacy arc in
`engine/examples/editor-playtest/src/game_logic_runtime.rs`. The weapon records
in `editor/projects/cortex-ignition-tech-demo-0.4b/project.ron` use an arc reach
of 640 and a 60-degree half-angle for both light and heavy energy weapons.
The cooked manifest shows the expected unit conversion, so this is an authoring
and contact-volume issue rather than a PSX-angle bug until proven otherwise.

Plan:

1. Build a range sweep test around the light enemy and the heavy enemy, testing
   current reach, a modest increase, and the capsule-only path at the exact
   active frames.
2. Tune the blade capsule endpoints/radius and arc reach together; do not fix
   the symptom by widening every attack's angle.
3. Preserve one-hit-per-swing latching and BSP occlusion.
4. Recook the manifest and add a receipt that records the chosen reach in both
   authored and cooked units.

Initial candidates to playtest are 640, 768, and 896 authored units. These are
test points, not a final balance decision.

### 4. Make the opposite-stance rule unmistakable (P1)

The rule already exists in `CombatStance` and `RuntimeGameEntities`: matching
the target's active channel receives the guarded multiplier and the opposite
channel receives the exposed multiplier. Cortex 0.4b currently authors these
as 2048 (50%) and 6144 (150%) Q12 respectively. The target HUD already shows
both pools and the active colour, but it does not explicitly say “use the
opposite stance,” and a 50/150 split may be too subtle during a first play.

Plan:

1. Add a short tutorial/message page and a compact target-HUD cue stating that
   the player's attack colour must be opposite the enemy's active colour.
2. Add distinct hit feedback: a muted/guarded spark and very small damage
   number for the wrong channel; the existing strong-hit/poise feedback for
   the exposed channel.
3. Test a much stronger contrast. A reasonable first experiment is a guarded
   multiplier around 6–12% (roughly 1–3 damage from a 25-point base hit) while
   retaining 150% exposed damage. Keep both values authored and reversible.
4. Verify that typed enemy projectiles and untyped legacy contact damage retain
   their intended semantics; untyped damage should not pretend to communicate
   a readable colour.

Acceptance: a first-time player can infer the rule from one locked encounter
without external explanation, and the combat replay proves the wrong-channel
hit still connects but is clearly inefficient.

### 5. Stance-swap invulnerability (P1)

`CombatStanceConfig` currently labels `swap_duration_ticks` as presentation
only. Player enemy-contact/projectile checks only query
`CharacterMotor::is_action_invulnerable()`, so a stance swap grants no i-frames.
Player/environment damage also routes through `apply_stance_damage()` and needs
one shared guard if the swap is meant to protect against every combat source.

Plan:

1. Add a separate authored `swap_invulnerable_ticks` field, clamped to the
   swap presentation duration. Do not make the full 72-tick player colour
   assembly invulnerable by accident.
2. Expose `CombatStance::is_swap_invulnerable()` and combine it with motor
   i-frames in one `Playtest::player_is_invulnerable()` helper.
3. Use that helper for enemy melee, projectile target creation, and the chosen
   environmental-damage policy. A whiffed attack must not consume its one-hit
   token while the player is in the protected window.
4. Start with an 8–12 tick window for playtesting, then tune against the
   dash's existing 25-tick invulnerability without making stance swapping a
   replacement for evasion.
5. Add unit tests for damage at swap frame 0, the last protected frame, and the
   first unprotected frame; include projectile and melee cases.

### 6. Stance colour readability (P2; partly already implemented)

The current source already has the requested foundations:

- player phase assembly uses the active stance colour;
- enemy swaps use a rising `ModelTintSweep`;
- the target HUD exposes the active pool colour.

The remaining design question is persistence. `ModelPhaseAssembly` holds the
colour briefly and then fades back to the normal material, while the enemy tint
is only active during the swap window. That may be why the tell was not retained
by every Comicon player.

Plan:

1. Keep the current full-strength swap animation and enemy sweep.
2. Add an optional low-strength persistent accent on a small texture/material
   region while a stance is active (rather than tinting the entire model).
3. Make the accent use the same Horizon/Zenith palette as the blade trails and
   HUD, with a conservative mix so it reads as a state marker rather than a
   full material replacement.
4. Test the accent with the camera at the normal combat distance and during
   lock-on; remove it if it competes with hit sparks or the target HUD.

## Reference projects used for design study

- [WarriOrb](https://github.com/NotYetGames/WarriOrb): MIT-licensed Unreal
  Engine 4 C++ source for a Souls-inspired action platformer. Useful for
  inspecting a shipped-style action/state split; its commercial assets are not
  part of the license.
- [UE5 Action RPG](https://github.com/ilchul1/UE5ActionRPG): MIT-licensed,
  source-only combat architecture with explicit ability lifecycle, hit traces,
  animation integration, StateTree AI, and hitstop. Useful as a structural
  reference, not as code to port to PSX.
- [Cat's Godot Souls-like Template](https://github.com/catprisbrey/Cats-Godot4-Modular-Souls-like-Template):
  permissive template demonstrating root motion, lock-on, dodge/parry windows,
  and enemy state handling. Useful for interaction and readability checks.

These references support explicit action phases, authored hit volumes, clear
state feedback, and bounded presentation work. They do not provide Dark Souls
3's proprietary logic and will not be copied into Cortex.

## Delivery order

1. Reproduce and profile the dissolve on hardware; land the smallest safe
   optimisation or budget fallback.
2. Fix attack-facing commit and add the regression replay.
3. Run the melee range sweep and choose authored values.
4. Implement the stance-rule messaging/feedback and retune the damage contrast.
5. Add and validate swap i-frames through the shared damage gate.
6. Only then decide whether the persistent enemy/player colour accent is still
   needed.

Every step ends with unit tests, a cooked-manifest check, a deterministic replay,
and a PS1 heap/primitive-budget receipt before it is considered ready for a new
demo disc.
