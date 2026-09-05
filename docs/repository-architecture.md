# Repository architecture

The original `EBonura/PSoXide` repository owns the SDK. This repository owns
the editor, engine and Cortex. PSoXide-emulator owns standalone emulation.

| Path here | Owner | Workspace |
| --- | --- | --- |
| `editor/crates/` | Editor authoring and cookers | Host |
| `engine/` | Editor runtime and gameplay | Engine, with separate guest examples |
| `editor/projects/` | Editor, including Cortex 0.4b | Data |
| `editor/archive/fixtures/` | Editor integration fixtures | Data |
| `emu/crates/frontend/` | Editor's integrated application and Play viewport | Host |
| `emu/crates/{emulator-core,psoxide-settings,psoxide-validation,psx-gpu-render}` | Imported from PSoXide-emulator | Host |
| `sdk/` | Imported from PSoXide SDK | SDK |
| `crates/` | Imported SDK hardware, disc, trace and format contracts | Host |
| `tools/mkisopsx`, `tools/psoxide-link` | Imported SDK build tools | Host |

Run `make bootstrap` before Cargo. The component lock pins full revisions;
imported source is ignored by Git and verified using content hashes. It is
materialized into one consistent source layout so the cookers, guest builds
and emulator all use the same SDK and format crate. Edit source in its owning
repository and bump the lock; do not patch the imported copy.

The shared `psxed-format` crate now lives at `crates/psxed-format` in the SDK.
Its crate name and binary formats are unchanged. The standalone emulator has
no editor or engine dependency. The editor still links the emulator's core
and renderer, while owning its preview and authoring shell.

The demo-disc repository owns release integration, including every game,
hardware tests, complete-disc packing and relocated audio checks. See the
[dependency matrix](demo-disc-dependencies.md) and [migration acceptance](sdk-separation.md).
