# Build and validation tools

The Rust command-line tools below belong to the root host workspace and share
its lockfile. Run them from the repository root with `cargo run -p <name> --`.

| Tool | Purpose |
| --- | --- |
| [`mkisopsx`](mkisopsx) | Pack a PSX-EXE and optional assets/audio into a bootable BIN/CUE disc. |
| [`psoxide-link`](psoxide-link) | Hydrate a pinned source tree or an explicit local override for downstream games. |
| [`psoxide-dev`](psoxide-dev) | Repository checks, numeric policy, profiling reports and asset-generation helpers. |
| [`psoxide-perf`](psoxide-perf) | PC-line and sample attribution against linker maps and ELFs, static instruction censuses (`pc-line-attribution`, `pc-symbolize`, `instr-census`, `text-census`), the Cortex benchmark and 30 fps reports, and the content-verified replay store (`performance-suite`, with its `quake-chain-adapter`). |

The remaining Python and shell utilities cover MIPS instruction hazards, guest symbol
checks, hardware capture analysis and web delivery. Use their `--help` where
provided, or the calling target in the [Makefile](../Makefile).

- [Downstream project setup](../docs/downstream-projects.md)
- [Contributor checks](../CONTRIBUTING.md)
- [Disc and executable formats](../crates/psx-iso)
