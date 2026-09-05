# SDK, emulator and editor separation

The repository split was completed on 5 September 2026. The existing
`EBonura/PSoXide` URL is the SDK. Cortex stays with the editor and engine.
The demo disc remains the integration and release owner for the complete set
of components and games.

| Repository | Contents |
| --- | --- |
| [PSoXide](https://github.com/EBonura/PSoXide) | Bare-metal SDK, linker/runtime, shared hardware and cooked-format contracts, small examples, bootstrap and disc tools |
| [PSoXide-emulator](https://github.com/EBonura/PSoXide-emulator) | Emulator core, standalone desktop/web frontend, renderer, debugging and profiling |
| [PSoXide-editor](https://github.com/EBonura/PSoXide-editor) | Authoring UI, cookers, engine/gameplay runtime, Cortex and assets, New Project template and hardware integration fixtures |
| [PSoXide-demo-disc](https://github.com/EBonura/PSoXide-demo-disc) | Launcher, loader, packer, component/game locks, audio relocation and complete-disc validation |
| Existing game repositories | Their game-specific runtime, assets and tools; consume the components they need |

Existing commits at the original SDK URL remain available for reproducible
historical game pins. Moving the current tree does not remove assets from
Git history. Engine/cooker exports omit authored Cortex projects, so ordinary
downstream games do not need those assets or the editor UI.

## Building the editor

From a fresh editor checkout:

```sh
make bootstrap
make verify-components
make check
make test
make run
```

See the [root README](../README.md) for host dependencies. The root host
workspace and `engine/` device workspace remain separate. The dependency-free
`psxed-format` package moved to the SDK's `crates/psxed-format`; its package
name and binary layouts are unchanged.

`components.lock.json` selects full SDK and emulator Git revisions.
`tools/bootstrap-components.py` exports only the declared paths and writes
an ignored `.components-receipt.json` with content hashes. It refuses to
replace edited imported files. `make verify-components` checks the lock and
receipt offline. Local exports still use the exact locked commit:

```sh
python3 tools/bootstrap-components.py \
  --source sdk=/path/to/PSoXide \
  --source emulator=/path/to/PSoXide-emulator
```

Develop shared code in its owning repository, commit it there, then update
the editor lock and bootstrap. Do not use ignored imported files as a source
checkout. Publish the dependency commit before publishing its consumer lock.

## Release integration

The [disc dependency matrix](demo-disc-dependencies.md) covers all games,
Cortex, hardware tests and the demo itself. The demo's
`release-components.json` and submodule revisions select the tested source
combination; a newer component `main` is not automatically a release input.
Ordinary games receive the bootstrapped editor tree through the existing
`PSOXIDE_FROM` adapter. The top-level disc loader and packer use the SDK
directly. The standalone emulator provides release replay evidence.

Quake retains its independently validated pre-split source/artifact pin.
Other historical standalone game pins remain valid until deliberately
advanced. Their combined-disc builds already receive explicit split inputs.
A `.components.json` sidecar binds the final disc to its effective sources,
toolchain and emulator/image hashes; existing Quake and HL receipts remain
additional gates.

The split acceptance covered SDK examples, editor/runtime tests, downstream
cookers, both disc editions, every game route, guest instruction hazards,
Cortex's forbidden-symbol check and relocated audio sectors. The detailed
[dated validation record](https://github.com/EBonura/PSoXide-demo-disc/blob/main/docs/repository-split-validation-2026-09-05.md)
lives with the demo disc. Original-console timing and burn acceptance remain
separate from those host/emulator results.
