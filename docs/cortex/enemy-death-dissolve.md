# Enemy death dissolve

Enemies finish their death clip, then shed their final posed mesh from top to bottom over five seconds. Each triangle lifts and turns around its own centre, retaining its texture as it fades. Quarter-strength additive blending keeps overlapping fragments from becoming a bright shell.

The existing death-state timer drives the effect, including while the corpse is offscreen. Expired corpses are excluded before pose resolution, which also removes their equipment and shadows. Respawning restores the ordinary entity lifecycle. No per-enemy particle arrays or new textures are allocated.

To fit the effect in PS1 RAM, cube-only projects no longer reserve an unused panorama packet cache. Dash debris also stores exact integer coordinates relative to its capture origin in signed 16-bit fields. It retains the same fragment count, positions and timing.

Validation: 232 runtime tests, release build, arithmetic-symbol gate and load-delay hazard scan. A separate capture executable triggers deaths and positions the camera for inspection; those capture controls are not included in the test disc.
