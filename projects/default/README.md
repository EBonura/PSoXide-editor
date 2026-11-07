# Cortex Ignition Tech Demo 0.4b

Working copy of the v0.4 level, retaining its geometry, textures, actors and eight points of interest. Runtime fixes restore the new music and attack audio, correct authored-to-runtime lighting scale, and put POI markers at actor-compatible depths.

Open `project.ron` in the editor and rebuild Play after changes. The verified native disc is `baked/cortex_ignition_tech_demo_0_4b.cue` (keep its BIN alongside it). The obsolete copied v0.4 disc has been removed to avoid launching stale code. See [runtime fix notes](RUNTIME_FIXES.md) for verification and limitations.

- Open `project.ron` in the Editor.
- All cooked 64 x 64, 4bpp Editor textures live in `assets/textures/`.
- Matching editable PNG sources live in `source_assets/textures/`. The final V3
  sources are already 64 x 64; earlier V1/V2 sources retain their larger masters.
- `DP Fog City Cube Sky` is assigned to the World in `Cube` mode. Its editable
  1774 x 887 (2:1) panorama lives in `source_assets/sky/`; its runtime-ready
  1536 x 256, 4bpp, six-face/six-CLUT PSXT lives in `assets/textures/sky/`.
- The World uses `Always` visibility so the cube sky fills the open review stage
  without requiring a sky-aperture brush. The previous 271-brush scene is backed
  up in `logs/project.ron.pre-dp-skybox.bak`.
- `DP_SKYBOX.md` records the conversion contract, source art direction, scene
  simplification, and real in-engine capture paths.
- `DP_CITY_TEXTURE_KIT.md` documents the eight-texture BSP construction kit,
  including its bright signals, dense teal bays, and two transparent overlays.
- `TEXTURE_SET_V3.md` describes the intended architectural role of each material.
- `FUTURE_GOTHIC_TEXTURES.md` documents the optional grittier future-Gothic
  material pass now painted across the current level geometry.
- `concept/reference_set_v2/` contains the original hero-room reference and four
  matching room concepts to build from.
- Aletha’s standard forward walk is the approved cybernetic gait: a 1.4-second loop sampled at 30 Hz, with restrained body motion and slightly wider arms. The editable Blender file and GLB are in `source_assets/animations/player/cybernetic_walk/`. See that directory’s README for rebaking and the required root-motion settings.
- Aletha’s Horizon light attack keeps the approved v5 performance with the approved v12 head motion: a two-second sword cast, deep load, driving step, broad strike and weighted recovery. Current opener and combo sources are in `source_assets/animations/player/cybernetic_light_combo_review_v12/`; historical v5 sources remain in `cybernetic_light_attack/`.
- Aletha’s Horizon heavy attack (R2) is the approved v2 cross slash: staggered dual-blade casts, deep load, a 94 cm driving step and weighted recoil. Editable sources and bake settings are in `source_assets/animations/player/cybernetic_heavy_attack_review_v2/`.
- Aletha's backward walk is the approved dynamic v3 with planted sole pivots and arm follow-through. Editable sources are in `source_assets/animations/player/cybernetic_back_walk_review_v3/`.
- Aletha's locked-on left/right side steps use the approved faster v2: one-second cycles with stronger weight transfer and non-crossing footwork. Sources and bake notes are in `source_assets/animations/player/cybernetic_strafe_review_v2/`.
- [The animation register](ANIMATION_REGISTER.md) tracks all six approved replacements and every remaining original player/enemy animation, with versions and checksums in `animation-register.json`. Update it whenever an animation is replaced.
- `Aletha (Player)` is a complete player entity with model renderer, animation,
  character controller, third-person camera, and light sword equipment.
- `Intake Custodian` and `Gallery Custodian` are two separate enemy entities
  using the canonical `Rust Mantis Enemy` profile, combat AI settings,
  animations, and heavy sword equipment.
- Their cooked models, textures, and animation clips are copied into
  `assets/models/` and `assets/animations/`; the project does not rely on the
  old tech-demo directory at runtime.

The twelve un-suffixed Editor surface-material names are the recommended V3 set. Eight
older alternatives are clearly labelled `V1 Draft` or `V2 Draft` in the resource
browser, but remain available for remixing.

Six additional materials prefixed `Future Gothic /` provide brighter,
lighting-neutral 64 x 64, 4bpp alternatives for the primary beam, bulkhead,
deck, rib, service-panel, and wall-plinth surfaces. Their masters and cooked
textures live beside the original set; no original material or texture was
removed.

`Future Gothic / Exterior Rock` extends the same set for outdoor brushwork. A
six-face `Future Gothic / Computer Terminal` kit provides front, left, right,
back, top, and bottom materials for brush-built environment props. The front
cycles between two independently selectable CRT readout materials while
preserving the same casing and face UVs. The cooker creates the shared 4bpp
runtime atlas automatically. These resources are registered in the material
browser and intentionally start unassigned.

Sixteen additional materials prefixed `DP BSP /` translate selected 32 x 32
tiles from DP Complete into 64 x 64, 4bpp BSP-ready textures. They are registered
in the material browser and intentionally start unassigned. See
`DP_BSP_TEXTURES.md` for source-cell provenance, rebuild instructions, and
suggested architectural roles.

Eight focused materials prefixed `DP City /` are the recommended kit for the
new fog-city mockups. They are 64 x 64, seamless and 4bpp. The cable and hanging
lattice reserve transparent palette index zero, use PS1 `Average` blending and
are double-sided. The remaining review-stage platform now uses the new deck,
edge and megastructure-wall materials so ordinary project cooking exercises the
kit immediately. See `DP_CITY_TEXTURE_KIT.md` for the contact sheet and roles.

This 0.4 snapshot is now an independent authored project. Save further level
changes directly into this directory; the original 0.2 generator does not own
or regenerate it.

### Horizon light combo

R1 once plays one light attack. Release and press again from 0.40–1.13 seconds after attack start (opener frames 12–34) to queue the approved v12 opposite diagonal swipe. The handoff occurs at frame 34 and keeps the sword materialized. Holding R1 does not chain. There is one pending continuation and no third strike; presses outside the window do not leak into another attack. See [timing, research and source notes](source_assets/animations/player/cybernetic_light_combo_review_v12/README.md) and the [animation register](ANIMATION_REGISTER.md).
