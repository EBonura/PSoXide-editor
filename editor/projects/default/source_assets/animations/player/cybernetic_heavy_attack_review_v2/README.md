# Horizon heavy cross slash - review v2

Approved and installed on 30 September 2026 after “very nice, let's use this one!”. Retains v1's staggered blade casts, crossing arm action, forearm alignment pass and two-second timing. Deeper rear-leg compression, 94 cm forward step (v1 76 cm), increased hip/chest counter-rotation, forward pitch and modest lateral bank, stronger landing absorption, overshoot and recoil. The feet are re-solved to the floor each frame.

Beats at 30 Hz: casts 6–11 and 12–17; load through 21; drive 21–23; crossing cut 23–29; body overshoot 29–31; recoil 31–36; settle 36–42; dissolve 45–51; ready 60. All motion remains authored character animation; preview blade appearance is schematic.

Run `author.py` then `render.py` using Blender 5.2 headless with factory startup and Python exit code 1; run `encode.py` using Python/Pillow and FFmpeg. The original two-weapon stage is inherited from `review/heavy-attack-v1/original-preview.blend`. V1 remains available separately. Outputs: editable master, GLB, measured validation, and `review/heavy-attack-v2/heavy-cross-slash-preview.mp4`.

Validation: maximum leg extension stays below 0.986, avoiding a locked knee; ankle target error stays below one micrometre. Both cameras retain the full blade sweep. The core cut at frames 24–25 retains edge-led motion; blade-plane deviation increases during the exaggerated body braking phase, as recorded in validation.json. Native verification is recorded in `runtime-validation.json`.


## Installed defaults

Clip 85 (`gen_heavy_attack`), source 208, player animation set 61, Horizon R2. Bake 61 samples at 30 Hz; action frames 0-60, speed 256, no looping, `in_place: false`, calibration `in_place: false`, zero push. This retains the approved body translation.

Existing heavy blade 31 in right hand: full at 11, five-frame cast from 6. Existing light blade 30 in left hand: full at 17, cast from 12. Both dissolve 45-50 (hidden 50, transition five); the schematic studio wire holds through frame 50 and vanishes at 51. Both trails and the existing heavy hit capsule use 23-29. Damage remains 38, poise 50. No second damage capsule or input binding was added.

Rebake from the repository root:

```sh
target/release/import-locomotion editor/projects/default/project.ron editor/projects/default/source_assets/animations/player/cybernetic_heavy_attack_review_v2 --pack gen --fps 30 --no-trim
```

Restore source 208, approved tags and `in_place: false` calibration after the generic importer, and retain the action and event settings above. Build with `target/release/frontend build-project-disc --project editor/projects/default/project.ron`.

The animation register records this as the third replacement. The approved walk, light attack, two-hit combo and other registered assets retain their prior checksums.

Native replay verified two R2 activations, the unchanged R1 single and the two-hit R1 combo. Full 61-frame/30 Hz runtime bake; no faults or scanned load-delay hazards; 15,228 bytes boot heap remain. All 77 other animation files are byte-identical to the pre-bake snapshot. This is emulator validation, not a physical-console test.
