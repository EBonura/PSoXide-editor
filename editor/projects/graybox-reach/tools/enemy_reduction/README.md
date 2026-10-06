# Graybox Reach enemy reduction

Blender 4.4.3, 2026-10-06. Local reductions of the shared default models; shared assets are unchanged.

| Model | Original triangles | Reduced triangles | Original vertices | Reduced vertices |
| --- | ---: | ---: | ---: | ---: |
| Rust Mantis | 530 | 438 | 451 | 400 |
| Tank | 756 | 631 | 618 | 549 |

The light mesh removes the inner eye recess wall and simplifies selected head, torso, arm and leg components. The tank simplifies selected torso, armor and connector components. This second pass saves a further 60 light triangles and 72 tank triangles over the first 498/703 pass. Reductions exceeding 1.2% of character height in the bidirectional rest-surface check at vertices, triangle centres and edge midpoints are rejected. Claws, feet, joint connections, skeleton tables, material tables, palette assignments and existing animation clips are retained. UVs on changed triangles are interpolated by Blender's decimator.

The clawless light variant keeps the same stream and disables part 9, matching the original variant. Its visible triangle count is 392. The model resources have no automatic import source, since reimporting the original GLB would overwrite the reduction.

From this project directory:

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --python-exit-code 1 --python tools/enemy_reduction/reduce.py
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --python-exit-code 1 --python tools/enemy_reduction/validate_render.py
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --python-exit-code 1 --python tools/enemy_reduction/editable_snapshot.py
python3 tools/enemy_reduction/install.py
```

The component index file refers to the original default meshes. Regeneration must use those originals. Generated files go into `source_assets/characters/enemy_reduced`; installation copies the runtime streams into `assets/models/enemy_reduced`.

The editable Blender snapshot includes rest meshes, UVs, packed palette images and runtime vertex weights. It does not contain an editable armature or animation actions. The project continues to use its existing PSXA animation clips. Direct snapshot edits are not consumed by the regeneration script.

Validation checked finite deformed geometry across 330 light frames and 221 tank frames, with surface comparisons at five poses per clip across idle, movement, turn and two attacks. Maximum sampled vertex-to-surface difference was 1.0396% and 1.1691% of original rest height, respectively. This is not a full Hausdorff or pixel silhouette bound. Reduced meshes have no duplicate or zero-area triangles. Before/after textured comparisons and the measurement reports are in `validation/enemy-reduction`.

The second pass was reviewed in front, side and back clay views, plus textured idle and attack poses. See `validation/enemy-reduction/pass2-*-comparison.png` and `pass2-multiview.png`. First-pass runtime assets are preserved in `source_assets/characters/enemy_reduced/previous-pass`. Historical unprefixed comparison images show the first pass.
