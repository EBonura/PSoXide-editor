# Demo-disc dependencies after repository separation

Current architecture as of 5 September 2026. Exact release revisions are
owned by the demo disc's `release-components.json`, submodules and artifact
receipts. Use those locks instead of treating the newest component `main` as
a tested combination.

Every PS1 guest uses SDK code. The additional dependencies below are build
inputs, not programs loaded beside it on the console.

| Consumer | Dependencies beyond SDK | Combined-disc form |
| --- | --- | --- |
| VoXide | Separate emulator for smoke/profile checks | Whole image; streams assets |
| NitroXide | Engine; texture, format and glTF cookers | Whole image; streams arena assets |
| PSXcel | Engine; emulator for screenshots | Bare EXE |
| Celeste Classic Collection | Optional host audio-capture tool links emulator-core/settings; game runtime does not | Bare collection EXE with both games |
| GH-PSX | Engine; chart tools; emulator for testing | Whole data image; shares Arcade's audio track |
| PSoXide Arcade | Engine; its own nested launcher, loader and packer | Whole image; Breakout, Invaders and Magikarp Pong, plus CDDA |
| Quake shareware | Engine, BSP/render contracts and audio cooker from independently validated historical source | Separately built and hash/provenance-verified image |
| Half-Life | Engine, render contracts, audio cooker, game-specific cookers and external Half-Life data | Whole image in HL edition; 27 audio tracks |
| Cortex 0.4b | Editor/cookers, engine/runtime, authored project and assets | Whole project image with world/UI packs and CDDA |
| Hardware tests | Engine and SDK fixtures; owned by the editor repository | Whole test image |
| Demo launcher and loader | Local carousel/disc table; SDK directly | Top-level executable and embedded chain-loader |
| Demo packer and validation | SDK psx-iso; Python validators; pinned standalone emulator | Final layout, receipts, route and relocation checks |

See the demo's [repository map](https://github.com/EBonura/PSoXide-demo-disc/blob/main/docs/repositories.md)
for links to all twelve owning repositories and their visibility.

## Effective build inputs

The demo uses these component paths:

- `games/PSoXide-sdk`: SDK for the launcher, loader, carousel and packer.
- `games/PSoXide-editor`: editor, engine, cookers, Cortex and hardware tests;
  bootstrapped with exact SDK and emulator revisions.
- `games/PSoXide-emulator`: standalone frontend for release validation.
- `games/PSoXide`: Quake's historical build reference only.

Ordinary game recipes receive `PSOXIDE_FROM` pointing to the bootstrapped
editor; HL receives the corresponding `--psoxide` override. The adapter keeps
existing Cargo path layouts working while recording the actual split source
combination. It does not compile the editor UI into a game. Cortex is staged
under the demo's build directory so cooking does not dirty its pinned source.

The original `games/PSoXide-runtime` and `games/PSoXide-cortex-current`
submodule roles have been replaced by the explicit editor component.
VoXide's standalone pin now selects the SDK-only repository. NitroXide's
`components.lock.json` selects SDK, engine/cookers and emulator libraries,
excluding authored Cortex projects. Other standalone pins remain reproducible
historical inputs; their advancement is a separate runtime update.

## Source imports and compatibility

The editor and emulator use `tools/bootstrap-components.py` with exact Git
revisions and a per-file content receipt. Generated source is ignored and
verified before release builds; edits belong in the owning repository.
`psxed-format` is a neutral SDK package under `crates/psxed-format`.

The demo owns the compatibility tuple and verifies it with:

```sh
make components
make verify-components
make sdk-on-main
make sdk-coherence
make check-locks
make quake-verify
```

A `.components.json` receipt records component/game revisions, nested locks,
source receipts, toolchain identity, emulator hash and final image hashes.
Quake's shipping provenance and HL's release receipt add their own content
and image requirements. The loader protocol, executable placement, cooked
formats and audio relocation remain compatibility boundaries even when guests
use different validated source tuples.

## Release validation

Build normal releases without diagnostic features. Verify both editions,
all outer entries, both Celeste games and all three Arcade games. Include
Cortex/Quake/hardware/HL repeat replays, guest instruction-hazard checks,
Cortex's symbol gate, exact track/pregap comparison and source receipts.

The [split validation record](https://github.com/EBonura/PSoXide-demo-disc/blob/main/docs/repository-split-validation-2026-09-05.md)
contains the measured 5 September results. That build retained all 27 HL
tracks and standalone offsets; on the combined HL image they became physical
tracks 8 through 34. Those numbers must be rechecked after any packing change.

Standard public release targets reject `HL=1`. Half-Life builds require the
owner's original game data and remain separate from public standard-disc
uploads. Host and emulator checks do not replace an original-console burn,
controller, timing and audio pass.
