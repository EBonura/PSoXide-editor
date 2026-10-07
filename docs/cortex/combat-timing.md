# Player combat timing

Graybox Reach uses authored animation permissions for dash-to-attack and hit-to-dodge recovery. This extends the existing attack combo and damage tracks. Enemy AI and enemy damage windows retain their existing implementation.

The Bloodborne research supplies the separation between input arming, retained requests, and action permissions. The values below are Cortex tuning. Reference: `bb-decomp/docs/movement-gameplay-2026-10-05.md`, especially the released R1 request during a roll and the independent attack/dodge/movement permissions.

## Authoring

In Animation Studio, choose the player character and action, then open **Player combat timing**. Windows belong to the character's Animation Set. Each channel has one half-open source-frame interval: start is included and end is excluded. Equal endpoints explicitly close a channel. Removing an override restores the previous behavior.

- `AttackBuffer` / `DodgeBuffer`: accept one press and retain it after release.
- `Attack` / `Dodge`: consume the pending request and replace recovery. Without a separate buffer window, a press is accepted inside the permission window itself.
- `Movement`: permit movement input to replace recovery; does not cancel the animation while standing still.
- `Invulnerable`: override the motor's dodge protection for this action. Stance-swap protection remains independent.
- `Armored`: override the normal heavy-attack active-hitbox armour interval. Armour keeps the existing finite poise resistance; it is not invulnerability.

Damage/projectile capsules, movement pushes and R1 combo handoffs retain their existing tracks and are included in the MCP report. Existing combo links keep priority for their attack-to-attack continuation. Death, intro and hook-travel clips cannot receive timing overrides. Poise break has no early-exit overrides in Graybox Reach.

Requests belong to one action instance, including its start tick. Hits, death, accepted transitions, and stance changes clear the corresponding pending intent. Holding a button cannot queue extra actions. A pending press can cross a narrow permission window even when playback skips a sampled frame; a new late press cannot retroactively enter that window. Playback rate and selected frame range use the same phase calculation as the visible player.

The cooker rejects reversed, duplicate, out-of-range, looping, and unreachable buffer tracks. It also rejects events that only target the final stored endpoint sentinel. Source frames are mapped into cooked samples after trim/resampling. Backstep and side dashes can author timing against their existing forward-dash visual fallback, without adding a new clip or changing motor duration.

## Graybox Reach tuning

| Action | Buffer | Permission | Other |
| --- | --- | --- | --- |
| Forward dash / Roll | Attack frames 1..7 | Attack 4..7 | Dodge buffer 3..7, dodge 5..7, movement 6..7 |
| Left/right dash | Attack 1..7 | Attack 4..7 | Movement 5..7 |
| Backstep | Attack 0..7 | Attack 2..7 | Existing short motor recovery retained |
| Hit reaction | Dodge 24..81 | Dodge 60..81 | Movement 72..81 |
| Heavy attack | Existing combo/input policy | Existing recovery | Armour 23..30, matching its damage interval |

Dash samples are 12 Hz: attack permissions nominally open at 0.333 seconds forward/sideways and 0.167 backward. Hit samples are 60 Hz: dodge opens at 1.0 second, movement at 1.2 seconds. Simulation sampling quantizes the boundary; the forward replay hands off 21 fixed ticks after its roll-start log. The 2.067-second poise-break motion remains committed. Invulnerability remains on the previous motor/stance rules until explicitly overridden.

## MCP

- `combat_timeline {"animation_set":61}` reports authored windows, bindings/speeds/ranges/pushes, damage/hurtbox volumes, combos, cook errors, and actual cooked intervals. It includes cooked sample rate and nominal seconds; attack-speed bonuses can alter elapsed time.
- `set_combat_windows {"animation_set":61,"windows_ron":"[...]"}` replaces the complete window list for the active player's set. It cooks a cloned candidate before staging. A rejected edit does not dirty the project. Call `save` to persist, then reload the editor project.
- An empty list removes the overrides. NPC permission editing is rejected by this MCP setter because the new runtime channels currently govern the player controller.

Example:

```ron
[
    (action: HitReact, kind: DodgeBuffer, start: 24, end: 81),
    (action: HitReact, kind: Dodge, start: 60, end: 81),
]
```

Runtime data uses at most 32 six-byte intervals per character, plus two fixed-size pending requests for the player. It allocates no heap memory and adds no rendered geometry. No hardware performance claim is made from this storage bound.

## Verification

Evidence is in `editor/projects/graybox-reach/validation/combat-windows`. Nine input-tape replays cover released/held/too-early dash attacks, stance cancellation, all locked dash directions, accepted hit-to-dodge and a rejected early dodge. Actual guest action and animation-phase counters are retained in the summary; the hit-to-dodge event occurs 60 ticks after impact. A separate host suite checks interval boundaries, empty overrides, action-instance changes, skipped windows, PAL/NTSC sampling and two playback-speed changes.

The project cook test rejects invalid tracks, and the MCP test verifies cooked timing and atomic rejection. Normal Play is rebuilt without diagnostic telemetry after validation. Graybox's training flag remains enabled for Select+L1/Select+R1 fixture resets; that flag does not disable combat damage.
