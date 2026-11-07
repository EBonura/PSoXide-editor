# Aletha standard Horizon light attack

Approved for Cortex Ignition Tech Demo 0.4b on 30 September 2026: **v5**, the final PS1 exaggeration pass. Approval: “amazing, that's the one!” followed by the request to bake it into the default project.

- `light_attack.blend`: editable approved master, action `aletha_cybernetic_light_attack_v5`.
- `light_attack.glb`: exact approved export; SHA-256 `188e3f651bc015af9c5112510111e9a2b24bbd7054819e9b6f024a91b90467dc`.
- Authored and cooked playback: frames **0–60 at 30 Hz**, **2.0 seconds**, non-looping.
- Clip resource **84**, `gen_light_attack`, `assets/animations/gen/light_attack.psxanim`.
- Source resource **203**, player animation set **61**, `LightAttack` action (Horizon stance / R1).

This is an authored refinement of `source_assets/animations/player/direct_shoulders/r1_horizon_light.glb`, with a deep rear-leg load, 84 cm driving step, wider arm silhouettes, shoulder coil, landing absorption, follow-through and recovery. It is not a raw UniMate sample. Preview-only sword/floor objects are not in the character export.

## Playback and events

Keep clip calibration **`in_place: false, offset: (0, 0, 0)`** and action **`in_place: false`**. The authored body translation provides the approved lunge and weight transfer; cancelling it would change the performance. Keep `looping: false`, `speed_q8: 256`, `frame_start: 0`, `frame_end: 60` and `push_distance: 0`. The root returns to its starting pose; no additional controller displacement is added.

All event values below are authored 30 Hz frames. The current cook preserves that sample rate and full range.

| Event | Frames / settings |
| --- | --- |
| Sword materialization | 6–12; fully visible 12, transition 6 |
| Strike hitbox and sword trail | 22–29 |
| Sword dissolve | 42–48; hidden 48, transition 6 |
| Recovery completes | 60 |

The existing light sword (resource 30), `right_hand_grip` socket, capsule geometry, trail colors, damage 25 and poise damage 25 are retained. Other attacks, including the Zenith light attack, remain unchanged.

## Rebuild

From the repository root:

```sh
target/release/import-locomotion \
  editor/projects/default/project.ron \
  editor/projects/default/source_assets/animations/player/cybernetic_light_attack \
  --pack gen --fps 30 --no-trim
```

The generic importer resets clip metadata: restore source 203, tags `gen`, `cybernetic`, `approved`, and the calibration above after rebaking. Retain the action and event settings listed above. Do not restore the original clip’s 26–83 trim or 3× playback speed.

```sh
target/release/frontend build-project-disc --project editor/projects/default/project.ron
```

The default disc is `baked/cortex_ignition_tech_demo_0_4b.cue` with its adjacent BIN. The approved source bakes to 26 joints and 61 stored frames at 30 Hz; the runtime dictionary encoding retains that timing and disables root cancellation.

## Verification and record

The baked disc passed a native emulator smoke replay with two light attacks. Runtime checks verified the full range, speed, blade events and hit/trail window. The guest executable reports zero scanned load-delay hazards. This is an emulator check, not physical-console validation. Every other animation asset, including the approved walk, is byte-identical to its pre-bake state. The catalogue audit retains its pre-existing 14 unregistered legacy clips.

See [the project animation register](../../../../ANIMATION_REGISTER.md) and its JSON companion for all replacements and remaining originals, baseline commit, and source/asset checksums. Local logs and the gameplay recording are in `review/light-attack-bake/`.
