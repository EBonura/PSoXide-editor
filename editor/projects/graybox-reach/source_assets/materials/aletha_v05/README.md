# Historical 506-triangle material adaptation

Superseded by the exact 458-triangle Cortex Ignition v0.5 mesh. Reach now uses
`assets/models/aletha_closed_458/aletha_mirror_458.psxmdl` and its matching source
masters. The adaptation script below is retained only to reproduce the earlier
comparison; do not use it to replace the selected 458-triangle model.
See `validation/model-458-transfer.json`.

# Cortex Ignition v0.5 character material transfer

The 128×128, 4-bit mirror texture and Aletha Crystal material settings were
copied unchanged from Cortex Ignition 0.5 on 2026-10-06.

Reach retains its existing 506-triangle Aletha geometry, skinning, skeleton and
animations. `adapt_reflection.py` adapts the v0.5 normal/gradient encoding to
that mesh, changing only face UV metadata and its format flag. It derives from
PSoXide/output/aletha-crystal/cook_reflection_model.py and requires NumPy.

From the editor repository root:

```sh
python3 editor/projects/graybox-reach/source_assets/materials/aletha_v05/adapt_reflection.py \
  editor/projects/default/assets/models/aletha_delivered/aletha_delivered.psxmdl \
  editor/projects/graybox-reach/assets/models/aletha_delivered/aletha_mirror.psxmdl
```

The material's facet-reflection mode suppresses the stance recolouring, matching
v0.5's silver crystal appearance. Validation confirms all 506 faces retain their
metadata after cooking, the editor MCP audit passes and normal gameplay
completes the 2,569-poll animation tape. See `validation/texture-transfer.json`.
The pre-transfer backup is `logs/project.ron.before-v05-texture`.
