# PSoXide Editor

Start with the [PSoXide Demo Disc](https://bonnie-studios.itch.io/psoxide-demo-disc): it includes the Cortex Ignition Tech Demo
and the other Bonnie Studios PlayStation demos. Standalone downloads are available
for testing just this project.

The PlayStation editor, runtime engine and Cortex Ignition live together here.
Author scenes, BSP geometry, materials, animation, audio and UI, then cook a
project into a PS1 disc and playtest it with the integrated emulator.

[The SDK](https://github.com/EBonura/PSoXide) retains the original repository
URL. [The emulator](https://github.com/EBonura/PSoXide-emulator) owns CPU and
peripheral emulation. This repository pins both components and keeps the
editor's preview and playtest UI with the engine and game content.

## Build

Install Rust through rustup, Python 3 and host C/C++ build tools. On Ubuntu,
install `pkg-config libasound2-dev libudev-dev libxkbcommon-dev`.

```sh
git clone https://github.com/EBonura/PSoXide-editor.git
cd PSoXide-editor
make bootstrap
make check
make run
```

`make run-release` builds the release editor. Cortex's current project is
`editor/projects/cortex-ignition-tech-demo-0.4b`; earlier versions and assets
remain in the source history. PS1 release builds require the MIPS binutils
used by the instruction-hazard scanner. Existing project cooking, guest build
and disc targets remain available through `make help`.

## Components and downstream games

`components.lock.json` records full Git revisions for the SDK and emulator.
The bootstrap exports only the required source directories into ignored
paths and checks them against `.components-receipt.json`. It refuses to
replace modified imported files. `make verify-components` verifies the lock
and receipt offline. Commit component changes in their owning repositories,
then update this lock. Local source exports use exact locked commits:

```sh
python3 tools/bootstrap-components.py --source sdk=/path/to/PSoXide --source emulator=/path/to/PSoXide-emulator
```

The root host workspace contains the editor/cookers and integrated frontend;
`engine/` and the imported `sdk/` retain separate device workspaces. Shared
binary formats come from the SDK's `crates/psxed-format` package. Games which
need engine crates or cookers consume a bootstrapped editor checkout; SDK-only
games can consume the SDK directly. The demo-disc repository owns the tested
component and game combination for a release.

See [the dependency matrix](docs/demo-disc-dependencies.md) for the complete
demo-disc closure, and [the repository split](docs/sdk-separation.md) for
the implemented component workflow. Dated historical documents may use
pre-split repository paths.

## License and validation

[GPL-2.0-or-later](LICENSE), with existing asset attribution and provenance
preserved. Hardware-sensitive changes still require original-console evidence;
emulator checks alone do not establish hardware correctness.

## Recent changes

The current source adds a skippable opening to Cortex Ignition, with orbiting camera shots and Aletha's wake-up animation. See [opening sequence notes](docs/cortex/opening.md) for timing and camera controls.

Source snapshot **2026.09.05**: The editor, engine and Cortex Ignition now live in one repository with pinned SDK and emulator dependencies.
See the [changelog](CHANGELOG.md) for the remaining changes and published download versions.
