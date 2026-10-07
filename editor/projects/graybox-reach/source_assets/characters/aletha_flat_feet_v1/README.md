# Aletha flat feet

Approved shared geometry: 458 triangles, 235 welded vertices. Both soles are flat from heel to toe; 24 vertices changed, preserving topology, weights and skeleton. The native models, GLB and editable Blender masters are saved in `../aletha_closed_458`.

The compact arch pose is saved in `../../animations/player/arch_perch_v2/arch-perch-a.blend`. Only its two foot matrices were adjusted for the new soles; all other approved joint samples were preserved. Falling and landing sources live in `../../animations/player/fall_land_v1`.

`vertex-changes.json`, `installation.json` and `perch-validation.json` record the migration. `install.py` and `retarget_perch.py` are one-time migration tools: they expect the pre-change backups in `/tmp/aletha-flat-feet-originals`, which are local working files. Do not run them against the already migrated assets on a fresh checkout. Use the saved Blender sources and the animation authoring scripts for further edits. `study.py` reconstructs the old foot coordinates from the change record for comparison.

Validation and the in-game review image are in `../../../validation/aletha-flat-feet-v1`; the grapple/release replay tape is in `../../../validation/fall-land-v1/release-final.csv`. The console hardware has not been tested.
