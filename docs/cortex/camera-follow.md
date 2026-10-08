# Cortex gameplay camera

The Bloodborne-inspired gameplay camera combines independent vertical follow,
accelerated orbit, retained-pitch recenter, blended free/lock profiles, and
height-aware enemy framing. The controls are enabled in Graybox Reach. The
sections below document the implementation and its validation history.

## Independent vertical camera follow

The first camera refinement separates vertical follow speed from horizontal
follow speed. It is enabled on the Camera child of the player in
`editor/projects/graybox-reach/project.ron`.

The starting settings are:

| Stage | Horizontal lag shift | Vertical lag shift |
| --- | --- | --- |
| Camera position | 2 | 3 |
| Look-at focus | 2 | 4 |

A lag shift divides the remaining error by a power of two each display tick.
These settings make vertical tracking gentler while retaining the existing
horizontal response. They are initial Cortex tuning, not a reproduction of
another game's numerical defaults. Playtest stairs, ledges, descending movement
and close walls before treating the values as final.

## Authoring

Select the player's Camera node. Under **Follow smoothing**, enable **Position
vertical** or **Focus vertical** to set an independent vertical speed. Higher
speed values catch up faster. Disabling either checkbox restores its shared
Position or Focus setting. The same controls are available in Character camera
presets and are copied when placing a new player.

The RON fields are optional `position_vertical_lag_shift` and
`focus_vertical_lag_shift`. Character presets use the `camera_` prefix. An absent
field retains the shared lag setting, including in old projects with customized
lag values. Values are normalized to 0–6. Cooking resolves the optional fields
to concrete shifts and writes them into `LevelCameraRecord`; the playtest runtime
passes them into `ThirdPersonCameraConfig`.

## Runtime behavior

Only the vertical focus approach and vertical base-position approach change.
The smoothed focus is still checked against collision, and its translation still
carries the camera so the boom does not collapse during player movement. Wall
pull-in and floor clearance take priority over lag. Lock-height transitions,
manual orbit, recentering and boom-extension delay retain their existing rules.

The existing display-tick catch-up loop handles the new settings too. No new
collision queries, allocations or floating-point operations are introduced.
Opening shots construct their own config without overrides and retain their
existing behavior.

When vertical focus lag is slower than the shared focus setting, its displacement
from the desired focus is limited to a quarter of the authored target height
(at least one runtime unit). The bound is applied before the focus collision
query, so geometry still takes priority. Small steps retain their easing; a sharp
fall cannot leave the look-at point far above the player while the camera drops.
Absent overrides and vertical settings equal to the shared setting retain the
previous behavior.

## Verification

Camera tests cover ascending/descending height changes, independent position
lag with unchanged focus, horizontal response, eventual convergence, shared
fallback, grouped display ticks and immediate obstruction pull-in. Project tests
cover old RON, new-field round trips, normalization and manifest export. Editor
tests include copying the overrides from a Character preset into a placed Camera.

Useful checks from the repository root:

```sh
cargo test --manifest-path engine/Cargo.toml -p psx-engine third_person_camera --lib
cargo test -p psxed-project --lib camera
cargo test -p psxed-ui --lib camera
cargo test -p psxed-ui --lib dropping_player_profile_applies_camera_preset_and_replaces_player_source
cargo run -p psxed-project --bin cook-playtest -- editor/projects/graybox-reach/project.ron
make build-editor-playtest
```

The cooker rewrites ignored generated playtest assets. The guest build also
checks MIPS load-delay hazards and scratchpad stack usage. These checks establish
implementation and build correctness; in-game camera comfort remains a playtest
judgment.

On 2026-10-06, 48 engine camera tests, four project camera tests, 27 editor
camera tests and the player-preset placement test passed. Strict library clippy
passed for psx-engine, psxed-project and psxed-ui. The debug editor and PS1 guest
built successfully; the guest scan patched 17 load-delay hazards with zero
remaining, and all scratchpad call trees fit their regions.

Graybox Reach's exported disc also completed an image-free headless smoke run:
forward held, Cross held over polls 200–440, stop requested at poll 900, final
count 901 after the display flip. The final display was 320×240 with hash
`5a051ce9a6201ab2`. No guest frame telemetry was enabled, so this is a boot/input
smoke check, not a frame-rate measurement or visual camera assessment. No
screenshots were generated. Local logs are `/tmp/graybox-camera-*.log`.

The cooker also reports a missing Aletha turn clip and a conservative packet
envelope above capacity. The camera change adds no render geometry; those
content/performance warnings still need their own recorded-view validation.

## In-game validation and sharp-drop correction

The follow-up pass uses a temporary copy of Graybox Reach under
`/tmp/graybox-camera-validation/project`, sharing the project's assets. Only the
copy's initial player placement changes for individual scenarios. The working
project's scene, models, enemy setup and camera tuning are preserved.

Diagnostic builds use `cd-stream-bench emulator-telemetry`, controller tapes
clocked by pad polls, and the existing player/camera counters. Captures are
inspected and then deleted. These are emulator observations, not hardware or
frame-rate measurements. Fixture positions below are authored project units;
the cooked world uses a 16:1 conversion.

| Scenario | Initial player position / yaw | Observation |
| --- | --- | --- |
| Ramp and upper landing | (14336, 0, -9216), 180 degrees | Player Y rises 0 to 128 runtime units; stable follow and settling |
| Low steps | (-5120, 0, 19456), 90 degrees | Player climbs from Y 0 to 32 runtime units |
| Ledge/rail drop | (14336, 2048, 4096), 90 degrees | Player crosses the rail at Y 160, then falls to Y 0; ledge obstructs the trailing camera |
| Wall orbit | (23552, 2048, 4096), 0 degrees | Stationary player, held right-stick orbit; camera compresses and releases around nearby walls |

The drop exposed a flaw in unrestricted slow vertical focus: after the player
fell, collision moved the camera down while its focus remained above the actor.
In guest-frame samples 600–650, the unbounded version's view pitch reached
18.65 degrees upward. With the previous shared smoothing, that same scenario
stayed between level and 10.81 degrees downward. The new focus-displacement bound
restores that pitch range while retaining independent vertical easing.

The regression test `vertical_focus_lag_stays_near_the_player_after_sharp_height_changes`
fails before the correction and passes afterward, for both positive and negative
height changes. All 49 camera tests pass, as does strict engine library clippy.
The corrected ramp replay has identical recorded player positions, camera
positions and pitch basis values across all 1,498 samples compared with the
unbounded version. The bound therefore leaves this ordinary ramp response intact.
The corrected low-step replay likewise matches all 1,198 recorded pose/basis
samples, and its inspected frames retain the same framing.

An extreme ledge obstruction still produces a short boom and temporarily hides
the player model under the existing near-camera visibility rule. This also
occurs with the previous shared smoothing. The correction removes the upward
view swing; it does not promise full-body visibility when the camera is against
the player. Runtime logs, input tapes and the numerical drop comparison remain
under `/tmp/graybox-camera-validation/`; temporary images are cleared.

After validation, the normal Graybox Reach disc and debug editor were rebuilt
successfully. The final guest uses only `cd-stream-bench`, with the original
player spawn (0, 0, -768), yaw 2048, in runtime units. Diagnostic placement and
telemetry are confined to the temporary fixtures. The final guest SHA-256 is
`ffc73b0c7a95292d341dfa24734b7b5f4daa109194b363a6778e0a7e1dbeb628`.

## Accelerated orbit and retained-pitch recenter

Graybox Reach now also enables `accelerated_orbit: true` and
`recenter_preserves_pitch: true` on the player's Camera node. Both flags default
to false in older projects and Character presets. The inspector exposes
**Accelerate held orbit** and **Keep pitch on recenter**; presets copy both
settings into newly placed player cameras. Cooking preserves them in each room's
`LevelCameraRecord`.

Accelerated orbit uses the squared speed option to interpolate the researched
normal/fast rate ranges (80–240 and 180–500 degrees/second). Cortex's existing
supported speed range remains 1–7. At Graybox's level 3, integer Q0.12 rounding
produces 18 to 40 turn units per tick, approximately 94.92 to 210.94 degrees per
second at 60 Hz. Both axes use the same rate curve. Each has an independent
36-tick hold ramp; reversal and release reset its timer and fractional carry.
Fractional accelerated deltas accumulate rather than being lost each tick.
Releasing the stick stops manual rotation immediately.

The optional automatic alignment policy waits 120 ticks after manual input,
then restores its steering rate over 60 ticks. Graybox continues to leave
movement-driven automatic alignment disabled, so releasing the stick does not
turn the camera behind a moving player. These timings follow the existing
60 Hz camera/control tick convention; they are not host wall-clock timers or
render-frame counts.

With retained pitch enabled, recentering takes the shortest yaw path behind the
player over 18 ticks and preserves the selected orbit pitch. Collision can still
change the final rendered view or prevent a clear trailing position. Manual
orbit cancels recenter; lock-on takes priority. A recenter edge is consumed once
when processing a batch of catch-up ticks. In gameplay, press R3 with no eligible
lock target to request recenter; an eligible enemy still makes R3 lock on.

The engine checks cover ramp endpoints, release, reversal, signed fractional
symmetry, grouped versus individual updates, recenter timing above and below
the default pitch, the legacy pitch reset, and manual-control wait/fade. Older
project loading, RON round trips, cook/manifest propagation and preset placement
are also covered. No collision queries or heap allocations were added.

This is an adaptation of observed camera policies to Cortex's integer camera.
The profile and target-height work below completes the planned Cortex gameplay
camera adaptation. Bloodborne-specific boss scripts and cinematics are not part
of this implementation.

The accelerated-orbit pass passed 54 engine camera tests, four project camera
tests, 27 editor camera tests, and the player-preset placement test (86 total).
Strict library clippy passed for psx-engine, psxed-project and psxed-ui, and the
debug editor rebuilt successfully.

Two diagnostic wall replays completed at polls 1400 and 1300. The second tape
includes separate stick holds, release, reversal, an upward orbit adjustment,
and R3 recenter at polls 920 and 1120. Telemetry records the stationary player
at (1472, 128, 256). After settling, released yaw remains constant. The raised
view pitch is -23.908 degrees before recenter (samples 860–919) and after it
(samples 1020–1110 and 1200–1290), while view yaw changes from 0 to 2048. These
are final rendered view measurements; the 18-tick internal orbit timing is
covered by engine tests. Inspected captures show the expected wall compression
and release. All temporary images were removed. Tapes, counters, summaries and
logs remain in `/tmp/graybox-camera-validation/` (the `orbit-*` files and
`wall-orbit/`). This is emulator evidence, not a console test.

The final normal disc restores the authored player spawn (0, 0, -768), yaw 2048,
and uses `cd-stream-bench` without telemetry. Both new camera flags are true in
the cooked room. The final guest scan patched 17 hazards with zero remaining;
all scratchpad stack checks passed. Guest SHA-256:
`63e122b3bfd89621f42dd60eebc7d3505d88e07eeadf6f75655efe62246d8018`.
The normal disc also passed an image-free boot/input smoke run through poll 902
(stop requested at 900), with a 320×240 display and hash `df067dda9ac2830f`.

## Camera profiles and target-height framing

The camera now supports an ordinary composition and an optional `lock_profile`.
Each lock profile carries `distance`, `height`, `target_height`, and
`fov_y_degrees`. The ordinary profile uses the existing offsets plus
`fov_y_degrees`; zero inherits the supplied projection. Profile lens angles are
38–48 degrees. `blend_profiles` enables smooth transitions, and
`lock_target_framing` enables the elevated target anchor. All new authoring fields
have legacy-compatible defaults. The controls are available on Camera nodes and
Character presets, and profile lengths use the same 16:1 BSP cooking conversion
as the ordinary camera.

The first implementation used these authored compositions (superseded by the
reference calibration below):

| Profile | Distance | Camera height | Focus height | Vertical FOV |
| --- | ---: | ---: | ---: | ---: |
| Free | 3500 | 1800 | 1160 | 43° |
| Locked | 4400 | 1900 | 1160 | 46° |

Profile values approach their selected values using 104/4096 per 60 Hz tick.
Distance then passes through another 210/4096 approach. These approximate the
60 Hz equivalents of the researched .05 and .1 coefficients at 30 Hz. The
internal blend retains fractional precision; only the final runtime distances
and lens are quantized. Authored height changes retain the user's manual orbit
adjustment. The collision solver receives the blended composition, and profile
changes force fresh collision work within the existing bounded solver. A failed
collision query rolls back the profile/lens state together with the camera pose.

The gameplay camera uses an enemy anchor at three quarters of its scaled model
height above its live root. Combat acquisition and movement continue using their
existing target positions. The initial framing mode used horizontal focus bias,
capped at one eighth of the current boom. Its vertical bias followed the target anchor
and was capped by both one sixth of the player focus height and a lens-dependent
allowance (45% of the boom's vertical half-frustum). The reference calibration
below replaces that focus bias with angular lock framing. Extreme geometry can
still invoke the existing near-camera player-hiding rule.

The current camera projection now drives BSP GTE projection, BSP frustum clipping,
world-object visibility, portal visibility, and sky drawing. Lens changes
invalidate reused BSP selections and cube-sky packets; portal visibility also
includes the focal length in its refresh key. This prevents a widened lens from
revealing missing world or sky geometry. The initial and respawn camera views
use the profile lens as well. Opening shots keep their separately authored
camera settings.

No heap allocations or floating-point calculations were added to the camera.
The camera's fixed state contains the profile interpolation values; a bounded
integer search resolves an inherited lens only when its source projection changes.
The existing free-orbit collision policy still preserves the player's yaw and
movement directions. Locked wall escape retains its existing bounded probes.

### Completion validation — 2026-10-06

The completed camera passed 96 targeted tests: 61 engine camera tests, six
project serialization/cooking tests, 27 UI camera tests, one Character preset
placement test, and one BSP projection-cache regression. Coverage includes
profile transitions, exact inherited-lens restoration, live lens overrides,
collision-error rollback, grouped versus individual ticks, and elevated/lower
target framing. Project/UI strict Clippy passed. Engine/BSP Clippy passed with
`clippy::type_complexity` allowed; the strict run reports that unrelated lint in
`render3d/world_pass_model.rs` at its test helper's face-list parameter.

Final-tuning emulator fixtures exercised repeated lock/unlock and target switching
on level ground and against targets above and below the player. The free lens
has focal length 305; the locked lens settles at 283, with boom distance moving
from 219 toward 275 runtime units. The upper-target fixture places the player at
Y=0 and target anchor at Y=206; locked focus settles at Y=85. The lower-target
fixture places the player at Y=128 and target anchor at Y=78; locked focus settles
at Y=189. Inspected captures retain the player in frame in both cases.

Wall, drop, and ramp regression runs also completed. The ramp follows player
Y=0 through Y=128. During the drop's critical guest frames 600–650, rendered pitch
stays between -10.807° and 0°, preserving the earlier fix for the upward camera
swing. A nearby ledge can still compress the camera and temporarily hide the
player through the existing collision/hiding policy. All temporary validation
screenshots were inspected and deleted. Tapes, counters, and full logs remain in
`/tmp/graybox-camera-complete/` and `/tmp/graybox-camera-validation/`.

The rebuilt debug editor exposes the new controls. The final normal Graybox Reach
disc restores the authored spawn `(0, 0, -768)`, yaw 2048, and builds with only
`cd-stream-bench`, without diagnostic telemetry. The cooked camera has all four
behavior flags enabled and the final locked composition `(275, 119, 73, 46°)`.
The guest build patched 20 MIPS hazards with zero remaining and passed every
scratchpad stack guard. Final guest SHA-256:
`6215db4c9e0e6833d305faef05a797ef4b3d405d78377c28d2aa41c31133af9c`.
An image-free normal-disc boot/input smoke run completed through poll 900 with a
320×240 display and display hash `3f8c4953e1970174`. This validates the emulator
implementation; physical-console behavior has not been tested in this pass.

## Standard-reference calibration — 2026-10-06

This supersedes the earlier custom free/lock compositions. Reanalysis of all
9,386 native rig observations establishes that ordinary free and enemy-locked
states in the recorded scene share row zero: distance 4, player offset 1.42,
and vertical FOV 43 degrees. All 358 locked observations keep that lens and
distance. Initial clear poses have zero pitch and a final 3.7-unit boom after
the native .3 safety shortening. Lock-on changes framing, not the profile lens.

The conversion uses controller height as an explicit scale reference: the
recovered native non-NPC initializer supplies 1.5, and Graybox's authored player
controller is 1024 (64 cooked units). At 64/1.5 runtime units per native unit,
the effective 3.7 boom rounds to 158, and 1.42 rounds to 61. The authored values
below are those integers multiplied by the 16:1 cook scale. This is controller
normalization, not a measured mesh-height match: Aletha's scaled visual height
is 90 runtime units, distinct from her collision height. A Hunter/Aletha visual
proportion match and identical screenshots are not established by this mapping.

| Current setting | Free | Locked |
| --- | ---: | ---: |
| Authored distance | 2528 | 2528 |
| Authored camera height | 976 | 976 |
| Authored player focus height | 976 | 976 |
| Vertical FOV | 43° | 43° |
| Legacy lock rise | 0% | 0% |

Orbit speed is now level 5, matching the option observed in the native trace:
nominal normal/fast rates 120/260 degrees per second, subject to Cortex's Q12
per-tick quantization. The reference framing mode also selects the researched
-40 to +70 degree ordinary orbit range.

The elevated-target mode now keeps the focus at the elevated player pivot.
It replaces the previous quarter-target focus translation and fixed lock lift
with angular framing. The integer goal combines negative target elevation,
`half_vertical_FOV * .45`, and an arcsine correction for camera-to-focus versus
focus-to-target distance. The native fulcrum/follow stages are approximated by
Cortex's current boom. The nominal lock range is -40 to +40 degrees, extending
the upper limit toward +70 as absolute target height difference moves through
.5 to 2 native units, scaled using the 1.42 focus reference.

Lock pitch approaches its goal at 669/4096 per 60 Hz tick, the rounded equivalent
of .3 at 30 Hz. Manual pitch is retained separately; unlocking eases the lock
adjustment back to zero. The collision solver receives the effective angle,
including during unlock, and failed queries roll back that state. Bounded trig
table searches avoid the general movement atan approximation's larger error at
small framing angles. No floating point or allocation is added.

This is a closer standard-reference composition, not complete native solver
parity. Enemy anchors still use model-height estimates, yaw chase and follow
smoothing retain Cortex policies, and collision uses Cortex's bounded queries.
The 3.7 native safety result is baked into the clear boom rather than reproducing
the complete native margin stage at every obstructed distance. Native 16:9 and
Graybox 4:3 views share vertical FOV but have different horizontal coverage.

Calibration validation passed 65 engine camera tests, six project camera tests,
and 27 editor camera tests (98 total). New checks cover the geometric lock angle,
camera-distance correction, singular/extreme coordinates, preserved manual pitch
after unlock, inherited projection, and projected enemy-anchor Y near 68 on a
240-line display across level/raised/lowered targets. The camera suite includes
the concurrent wall-orbit hold regression. Engine Clippy passes with only the
previously documented unrelated `type_complexity` lint allowed.

Six emulator fixtures completed: level-target lock/switch/unlock, upper target,
lower target, wall orbit, rail drop, and ramp ascent. The lock fixtures keep focal
length 305 and clear boom 158 in both modes; their focus stays at player Y+61.
The ramp traverses Y=0..128. With the newly level free camera, the drop's critical
frames 600–650 range from 0 to 2.533 degrees upward, rather than the earlier
downward-tilted preset's -10.807..0 range. Close ledges still compress the camera
and can hide the player. The closer reference composition also leaves little
space below the feet; full-body margins at extreme elevation are not promised.
All temporary screenshots were inspected and deleted. Numerical evidence and
logs are in `/tmp/graybox-camera-reference/` and the `*-reference` directories in
`/tmp/graybox-camera-validation/`; `native-profile-audit.json` records the native
sample sets and scale calculation.

The debug editor and normal project disc were rebuilt. The final cook preserves
the current authored spawn, now `(0, 0, -384)` in runtime units after concurrent
scene editing, and confirms both compositions as `(158, 61, 61, 43°)`, option 5,
and zero legacy lock rise. Features are only `cd-stream-bench`. The build patched
21 MIPS hazards with zero remaining and passed all eight scratchpad stack checks.
Guest SHA-256:
`fc0d4320d626d7be37fd4ba20ad5ac95f11c89bba21fe7ce15c45bbea925ef14`.
The normal-disc smoke run completed at poll 900 with a 320×240 display and hash
`2abe8b6d4aed57b1`. These remain emulator checks, not physical-console validation.


### Matched screenshot check — 2026-10-06

A subsequent visual comparison qualifies the composition claim above. A real
Bloodborne capture using the original camera solver, settled inside the clinic,
has a clear boom of approximately 3.70010, pitch approximately zero, FOV 43,
and no lock. An isolated copy of the research save was used; a position fixture
moved the player away from enemies, then ordinary movement turned the player
away from the camera. The capture was taken after settling. Global saves and
configuration were untouched.

Compared with the free frame at 0.3 seconds in `camera-reference-demo.mp4`, manual
silhouette measurements give Hunter height approximately 63% of the image and
Aletha approximately 67%, or about 7% greater relative screen height. Hood,
hair, idle stance and the 240-line source make these approximate measurements,
not a recovered native mesh height. Both level views put the lowest foot close
to the bottom edge (about 2% native margin versus about 1% Graybox).

The larger composition difference is aspect ratio. At 43 degrees vertical FOV,
16:9 covers approximately 70.01 degrees horizontally and 4:3 covers 55.42.
At the same depth, the 4:3 frame spans 25% less horizontal world width. Keeping
the native vertical FOV therefore does not establish matching overall framing.
The controller-height mapping also does not establish equal rendered character
scale; the 41% Aletha mesh/controller ratio is not a 41% screenshot mismatch.
Further calibration should use visible character proportions and an explicit
aspect-ratio policy. No camera settings were changed during this comparison.

The retained comparison, full source stills, centre-crop comparison and measured
metadata are in `build/graybox-reach/camera-comparison/`. Temporary startup and
combat captures were deleted. This check concerns stationary free framing;
locked tracking and motion equivalence are not established by these images.


### Raised orbit and lock comparison — 2026-10-06

The raised-free fixture uses eight downward right-stick polls, followed by
lock and unlock. Decode the counter sine and cosine fields by subtracting
4096 before atan2. Their stored values are biased, not raw Q12; applying atan2
directly initially gave incorrect diagnostic angles which were corrected.
The measured downward view angles are 17.7425 degrees free, 10.9470 locked,
and 17.7425 after unlock. Local camera Y changes 109 -> 91 -> 109 above a
player at Y=0. The original project settings were not changed.

A native raised-free screenshot has downward pitch 18.318 degrees and boom
3.70007. The matched lock screenshot has approximately 16.213 degrees and
boom 3.70469, with target horizontal range 7.97623 and target 0.250107 below
the player pivot. Applying the continuous Cortex framing formula to that
native geometry predicts 15.9457 degrees (about 0.27 degrees difference).
This supports the static angular formula for this sample, not complete solver
or visual equivalence. Graybox's target geometry differs; its higher anchor
explains much of the lower locked pitch.

The screenshots expose a remaining practical composition issue: Aletha
visually overlaps the selected enemy in the locked Graybox scene much more
than the Hunter does in the native shot. Character proportions, combat stance,
anchor selection and native follow geometry need to be separated before
claiming a visual match. Raised free orbit gives both characters more foot
margin, but changing default pitch alone does not change the lock goal.

The native lock comparison uses the original solver and active AI, with player
and enemy placement fixtures in an isolated save. The enemy is moving while
Graybox's reference targets are stationary. Process pauses were used between
inspection steps; these images do not benchmark real-time transition speed.
Native combat/contact frames and obstructed views were rejected as framing
references. Both native runs exited normally via SIGINT. Comparison images,
source stills and exact numerical metadata are retained in
`build/graybox-reach/camera-comparison/`; temporary captures were removed.


### Aletha composition recalibration — 2026-10-06

This supersedes the 158/61/61 project calibration above. The active Camera
node (id 6) in Graybox Reach now uses the following for both free and locked
profiles. These are Aletha/4:3 composition choices, not recovered Bloodborne
numeric values.

| Setting | Previous authored / runtime | Current authored / runtime |
| --- | --- | --- |
| Boom radius | 2528 / 158 | 3328 / 208 |
| Default camera height | 976 / 61 | 2304 / 144 |
| Player focus height | 976 / 61 | 1280 / 80 |
| Vertical FOV | 43 degrees | 43 degrees |
| Default free downward pitch | 0 degrees | approximately 17.9 degrees |

The longer boom reduces Aletha's screen size; the higher focus puts her head
below the selected enemy's body in the stationary reference fixture. The
raised default free orbit reveals more ground. The lens, aspect ratio, option-5
orbit acceleration, lag settings, floor clearance, target anchor selection and
lock pitch rule are unchanged. Lock still computes its pitch from the enemy;
unlock restores the manual free orbit. Animation poses and close-range combat
can still alter overlap and foot margin.

A reference-mode pitch conversion bug was also fixed. Authored height minus
focus height is the vertical component of the spherical boom, so its neutral
pitch must be recovered as asin(offset / radius). The former approximate atan
conversion added an unintended height bias when locking from a raised profile.
The replacement uses the existing bounded integer angle search and square root;
legacy non-reference camera conversion is preserved.

Validation: 66 camera tests pass, including a new raised-profile regression
that failed before the fix (enemy anchor at screen Y=55) and passes afterward
(anchor Y=65..71, player head below it by at least 28 pixels, root-foot point
at or above Y=224, exact free position/pitch restoration on unlock). The rendered
boots extend below the root-foot point, so normal-disc screenshots were also
checked. Flat captures exercise lock/unlock/relock/target switch; a separate
raised-stick replay checks manual orbit, lock and unlock. These are emulator
checks, not console measurements. Scoped engine Clippy passes with only the
pre-existing unrelated type_complexity lint allowed; camera Rust formatting
passes.

The normal project disc was rebuilt with the current project scene and active
enemy settings. Its 1,400-poll smoke replay completed, with approach, circling
and attack frames visually checked; a retained live-combat still supplements
the stationary fixture. Its post-link proof patched 21 load-delay hazards with zero
remaining and passed all 8 scratchpad stack guards. Build and replay logs live
under /tmp/graybox-camera-recalibration. The shipping build emits no angle
telemetry, so the approximately 17.9-degree figure above is authored geometry,
not a newly measured telemetry value.

The retained before/after comparison and full-size final stills are in
build/graybox-reach/camera-comparison/recalibration-before-after.png and
recalibrated-*.png, with settings in recalibration.json. Earlier comparison
artifacts remain as historical evidence. Temporary intermediate screenshots
are removed after review.

## Manual orbit stops at walls

With the player's back to a wall (the replay tape walks her into the Graybox
Terrain sky enclosure, 13 units from her centre), a full-stick orbit swept the
eye through every yaw on the wall side. The arm fell to the wall distance over
about 146 degrees of yaw, under `min_distance` for 35 ticks, so the scene hid
her and the view was the room from inside her head.

A manual yaw step is now refused when it shortens an arm already under twice
`min_distance`. Stepping out of that arc, and any orbit with room for the boom,
is unchanged. Lock-on and recenter steering keep their existing rules. With the
guest built with `emulator-telemetry`, each camera update logs a `camera-pose`
line (tick, player, eye, focus, yaw, pitch, boom, pull-in, stick, lock) for
`launch --guest-debug-log`.
