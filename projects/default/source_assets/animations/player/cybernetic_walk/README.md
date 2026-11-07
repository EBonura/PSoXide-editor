# Aletha standard cybernetic walk

Approved for Cortex Ignition Tech Demo 0.4b on 30 September 2026.

- `walk_fwd.blend`: editable master, 30 fps, frames 0 through 42.
- `walk_fwd.glb`: exact approved v7 export, with a duplicate endpoint for seamless looping.
- Cooked destination: `assets/animations/gen/walk_fwd.psxanim`, resource 73, player `Walk` action.
- Authored cycle: 42 playback frames at 30 Hz, 1.4 seconds.
- The project’s existing 2-degree error budget cooks this to 33 playback frames at 23 Hz. Walk playback uses Q8 speed 262 to compensate for integer-rate duration rounding, giving approximately 1.403 seconds after fixed-point playback quantization.

This is an authored refinement of the Mixamo Walking reference used during the UniMate evaluation. It retains foot trajectories, reduces hip/torso motion, steadies the head, and opens the arm clearance to 8 degrees. It is not a raw UniMate model sample.

## Bake

From the repository root:

```sh
cargo run --release -p psxed-project --bin import-locomotion -- \
  editor/projects/default/project.ron \
  editor/projects/default/source_assets/animations/player/cybernetic_walk \
  --pack gen --fps 30 --no-trim
```

Keep the full clip: the final frame is the duplicate endpoint expected by the runtime. The existing resource and action binding are reused.

The generic importer resets clip metadata. After rebaking, retain source resource 202, tags `gen` and `cybernetic`, and set clip calibration to `in_place: false, offset: (0, 0, 0)`. The player Walk action must also have `in_place: false` and `speed_q8: 262` for the current 2-degree project resampling budget. Forward travel is already removed in this source; runtime root cancellation would erase the approved lateral weight transfer. These settings are present in the shipped project.

Build the current default-project disc through the frontend:

```sh
target/release/frontend build-project-disc --project editor/projects/default/project.ron
```

## Validation

The source GLB matches the approved v7 export byte for byte. The baked source has 26 joints, 43 stored frames at 30 Hz, and identical first/last pose records. The runtime cook has 34 stored frames at 23 Hz with its duplicate endpoint intact; its Walk binding keeps root cancellation off and uses Q8 speed 262.

The default-project disc was rebuilt successfully and a native emulator replay reached gameplay, dismissed the introduction messages, and exercised forward walking. The guest executable has zero scanned load-delay hazards. This is a native emulator smoke check, not a physical-console or full-level test.

The broader animation catalogue audit reports the same 14 unregistered legacy clip files as the unchanged project; no new catalogue issue was introduced. Local build logs, checksums and the native video are in `review/cybernetic-walk/`.
