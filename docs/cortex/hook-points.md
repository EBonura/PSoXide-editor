# Elevated hook traversal

Graybox Reach has three Hook Point nodes and three solid raised platforms near
the starting courtyard. Hook transforms are landing foot positions, in authored
world units. The runtime marker floats 96 units (1536 authored units) above each
landing. Add more with the editor's Hook Point placement tool and position them
at the centre of a clear, supported platform. The cook rejects more than 32 hooks.

In Zenith/ranged stance, hold L2, aim at the crystal, and press R2 when its white corner brackets appear. R2 fires the weapon when no usable hook is selected. Selection
uses a 30-pixel radius around the actual aiming reticle, a 1600-runtime-unit
maximum range, and a destination at least 32 units above the player. It checks
camera visibility, a full player-body path, walkable floor support and live
actor/prop blockers. Hooking requires a readied gun and an idle, uninterrupted
player; enemy lock-on alone cannot enable it.

The move shares the dash polygon departure, wire body, wake, and reconstruction.
It first plays the dedicated HookLaunch anticipation for 12 simulation ticks
(about 0.2 seconds), then bursts and holds three capture ticks before flying.
The wire body plays the forward dash animation, flies toward a waypoint above the landing,
and descends to it at tick 42. Reconstruction follows for 36 ticks.
Travel checks body clearance again each tick, so a newly blocking actor or door
aborts the move at the last safe position. A hit/death cancels traversal. Holding
R2 does not retrigger it, and the movement does not refund stamina or retain fall
velocity. The camera follows the ascent while holding the travel heading and initial aiming pitch.
Cloth resets its physical origin during flight and shares the wire/reassembly
presentation, preventing the fast ascent from stretching the scarf.

| Node | Authored landing X/Y/Z | Runtime platform height |
| --- | --- | --- |
| Hook / Arrival | -4096 / 2048 / -2048 | 128 |
| Hook / Mid tower | 0 / 4096 / 2560 | 256 |
| Hook / High overlook | 6144 / 6144 / 5632 | 384 |

The first implementation uses the project's BSP collision backend. Hooks remain
ordinary editable scene nodes and cook to typed entity records; their names do
not control gameplay. Existing projects without hooks retain their input mapping.

A close-range crystal projectile crash exposed by the elevated views was also
fixed: widening a bolt could make the lower bound of its glow-size clamp larger
than its upper bound. Crystal width now stays within the screen-size limit.

Hook Launch (slot 41) reuses the existing `aletha_dash_fwd` clip (resource 36)
and the forward dash's playback options, including speed 123/256 and its full
frame range. The custom jump/flight prototypes are no longer bound to traversal.
The dedicated action slot lets hook movement retain its own collision and
breakup timing while sharing the dash animation asset.

The lens is an additive focal-length multiplier over the selected camera profile:
+8/256 during anticipation, -52/256 during flight, easing by at most 6/256 per
tick. With the current 43-degree vertical FOV this is roughly 42 -> 53 -> 43
degrees. Landing or interruption eases back to the unmodified profile; reset
clears the effect. Free and locked base camera settings remain editable.

Validation: all three updated landings replayed successfully in PSoXide; 316 runtime
tests, three project hook tests, and the motor teleport regression pass. The final
guest has zero remaining scanned load hazards and passes its scratchpad stack
guard checks. This build has not been tested on a physical console. Review media,
input tapes, logs and the shipped disc hash are in `build/graybox-reach/hooks/`.


Text-free signposting uses a mint octahedron inside four separated shards.
These render in the world ordering table, visible outside aiming mode and
occluded by level geometry. Selected hooks brighten and gain four white screen
corner brackets; the normal aiming reticle yields to those brackets. There is
no hook text prompt, landing-pad outline, vertical guide or ledge arrow.
The beacon has a two-unit bob and remains visible after landing. Decoration
never bypasses range, LOS or body-clearance validation.

Current signposting review media and the shipped disc hash are in
`build/graybox-reach/hooks/crystal-only/`.


During traversal the camera aligns behind the fixed departure-to-landing
bearing, preserving the initial aiming pitch. Right-stick and recenter inputs
are ignored until landing/cancellation. Anticipation permits a short heading
alignment; the flight holds that heading through the vertical descent without
flipping. The FOV swell and collision handling remain active. A deterministic
neutral-stick versus full-stick replay produced identical flight screenshots.
Flight-v2 review media and validation logs are in `build/graybox-reach/hooks/flight-v2/`.

Latest dash-reuse build and replay evidence: `build/graybox-reach/hooks/dash-reuse/`.
