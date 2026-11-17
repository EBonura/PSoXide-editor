# Build and validation tools

The Rust command-line tools below belong to the root host workspace and share
its lockfile. Run them from the repository root with `cargo run -p <name> --`.

| Tool | Purpose |
| --- | --- |
| [`mkisopsx`](mkisopsx) | Pack a PSX-EXE and optional assets/audio into a bootable BIN/CUE disc. |
| [`psoxide-link`](psoxide-link) | Hydrate a pinned source tree or an explicit local override for downstream games. |
| [`psoxide-dev`](psoxide-dev) | Repository checks, numeric policy, profiling reports and asset-generation helpers. |
| [`psoxide-perf`](psoxide-perf) | PC-line and sample attribution against linker maps and ELFs, static instruction censuses (`pc-line-attribution`, `pc-symbolize`, `instr-census`, `text-census`), the Cortex benchmark and 30 fps reports, the content-verified replay store (`performance-suite`, with its `quake-chain-adapter`) and the `psxed-mcp` batch client (`mcp-client`). |

The remaining Python and shell utilities cover MIPS instruction hazards, guest symbol
checks, hardware capture analysis and web delivery. Use their `--help` where
provided, or the calling target in the [Makefile](../Makefile).

- [Downstream project setup](../docs/downstream-projects.md)
- [Contributor checks](../CONTRIBUTING.md)
- [Disc and executable formats](../crates/psx-iso)

## Running the full gate (shared gate tools)

The scripts in [`gates/`](gates) and the two `psxed-mcp` examples are the one way
every lane runs the cook and duel gates. They do the same work as the older
per-lane scripts, only without idle sleeps, with the duel seeds in parallel, and
stopping on the first thing that is wrong.

```sh
# Build once (CARGO_TARGET_DIR as usual).
cargo build --release -p psoxide-emulator --features editor
cargo build --release -p psxed-mcp --bin psxed-mcp --example duel-baseline --example cook-check

# 1. Refuse to start when a pinned component is not on origin (also run by the sweep).
sh tools/gates/preflight.sh

# 2. cook_check on all 15 projects; each project returns as soon as its answer arrives.
tools/gates/cook-check.sh --out-dir "$OUT/cook"      # writes cc-<project>.json per project

# 3. The 20-seed disc duel sweep, seeds concurrent (--jobs defaults to performance cores - 1).
tools/gates/disc-sweep.sh "$OUT/duel" --jobs 8
```

- `cook-check` (`psxed-mcp/examples/cook-check.rs`): speaks the MCP lines to
  `psxed-mcp`, waits for each reply, closes stdin. Prints `== <project>: <reply line>`;
  the reply line is the same one the old shell driver printed. A failed check is
  still a reply (read the JSON, as before); the exit status is 1 only when a
  project gave no reply at all.
- `duel-baseline --jobs N`: runs the seeds N at a time, one emulator process
  each, every seed in its own `seed-N` directory. Rows print in seed order and
  `baseline.json` is built in seed order, so the output matches `--jobs 1` (the
  sequential run). It exits 1 when any seed could not run.
- `disc-sweep.sh`: preflight, then per project deletes the old `baked/*.cue|bin`,
  builds the disc and stops on any build failure or missing cue, then runs that
  project's seeds. Discs are built one at a time because a disc build writes the
  checkout's shared `generated/` cook output and guest stage.
- `preflight.sh`: fetches each revision in `components.lock.json` from its
  repository into a scratch repo, the way the hydrator does, and fails when one
  is not reachable (an unpushed SDK pin).

Where a disc build fails silently: it does not at the source. `psoxide-emulator
build-project-disc` runs `make cook-playtest`, whose `bootstrap` step fetches the
pinned SDK, and a failed fetch makes the frontend print `make cook-playtest failed`
and exit non-zero. What went wrong in the lanes was downstream: the scripts ignored
that exit code and `duel-baseline --skip-build` then took the newest cue already in
`baked/`. The sweep script above removes that cue first and stops on the failure.

### Reusing a gate result when the inputs are identical

`tools/gates/reuse.sh` wraps a gate whose whole input set can be named. It hashes
every file in the checkout (hydrated SDK and emulator components, projects, assets,
`Cargo.lock`, `components.lock.json`, `rust-toolchain.toml`, the gate scripts) except
`.git`, `.tools`, `build`, `target` and `baked`, plus the command line, `rustc -vV`,
`cargo -V`, the Cargo, rustc, editor and SDK environment, and any binary passed with
`--file`. On a match it prints `reused result from <date> (inputs identical), key ...,
run at <sha>` and replays the stored output; otherwise it runs the gate and stores the
result (passing runs only) under `.gate-results/`. `--no-reuse` always runs.

- `tools/gates/editor-suites.sh [--no-reuse]`: the three editor suites.
- `tools/gates/cook-check.sh [--no-reuse]`: keyed also on the `psxed-mcp` and `cook-check` binaries.
- `sh tools/gates/test-reuse.sh`: a one-byte change in each input class (source, hydrated
  component, `Cargo.lock`, the SDK pin, project data, asset, toolchain file, gate binary,
  added, deleted and renamed files, the script, environment, tag, command, toolchain version)
  forces a rerun.

The disc duel sweep and every gate that depends on the guest build are not wrapped, and
must not be unless the guest exe sha and disc sha are passed with `--file` or `--tag`.
Any doubt about the inputs means do not wrap the gate, or pass `--no-reuse`.
