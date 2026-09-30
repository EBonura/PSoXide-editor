# Verified local performance replays

`tools/performance_suite.py` reuses completed replays by their complete input identity. It does not build games, download assets, mutate a game library, or resume snapshots. Python's standard library is sufficient on macOS/Linux.

Bindings map each `$name` input in `cases.json` to a local file. They are machine-specific and never live in this repository: keep the JSON (`{"name": "/absolute/path"}`) next to your local replay store. No retail assets or personal paths belong in the committed manifest. `cases.json` contains Celeste recorded checkpoints, a bounded Cortex gameplay checkpoint, Half-Life chapter-two timing, and the Quake E1M1-to-E1M2 diagnostic route. These are named historical fixtures, not claims that every current main was rebuilt.

```sh
python3 tools/performance_suite.py run benchmarks/performance-suite/cases.json \
  --bindings /absolute/local/bindings.json --store /absolute/local/replay-store \
  --case celeste.recorded.poll900 --jobs 1 --report /absolute/local/run.json
python3 tools/performance_suite.py get /absolute/local/replay-store/results/KEY
python3 tools/performance_suite.py compare /absolute/local/baseline/KEY /absolute/local/candidate/KEY
python3 -m unittest discover -s tools -p test_performance_suite.py
python3 -m unittest discover -s benchmarks/performance-suite -p test_quake_chain_adapter.py
```

`run` rehashes current inputs, takes an exclusive per-key lock, checks the inputs again, and verifies every cached artifact before returning a hit. Different inputs, commands, script/parser bytes, environment, build provenance, absolute paths or CUE-referenced files produce another key. It makes no assumption that a source commit built in two directories yields equivalent binaries. Results are atomically published only after completion and a post-run input rehash. Failed, timed-out, corrupt and partial entries cannot be hits. Failures remain available for diagnosis; storage retention is manual. A lock prevents duplicate simultaneous execution. Use a small `--jobs` value for bounded concurrency; parallel host wall times are not comparable benchmarks.

`get` verifies a historical result and returns an artifact index without copying the artifacts. It does **not** verify today's external inputs; use `run` for that. Historical logs without the complete identity/completion receipt are not automatically imported or trusted. Lookup still hashes all bytes, including transitive disc inputs for `run`; there is no mtime shortcut.

Each run gets an empty config/HOME and absent memory cards. Inherited `PSOXIDE_*` options are removed; declared overrides are part of the key. Native executable bytes are hashed. Opaque executable wrappers are rejected. Input flags require an exact declared token, output flags stay inside the result, and unsupported options fail closed. Add file-bearing options deliberately rather than bypassing this validation.

## Completion and quality

Generic poll completion supports validated binary `PXITAPE2` only. Its sample extent must continue beyond the maximum accepted poll. Competing frame limits are rejected, and the stdout early-stop marker must precede the instruction cap. CLI faults fail even when the process exits zero. These conditions distinguish the CLI's next-display-flip stop from tape exhaustion or a hard cap. Other clocks and game-specific completion use a hashed Python adapter/interpreter. Quake's adapter validates its v9 RAM probe and the canonical gameplay presentation window; this is diagnostic-game evidence, not an ordinary shipping campaign proof.

`quick` means timing triage: it can never pass acceptance. `acceptance` enables explicitly declared artifact comparisons. At least two visual artifacts and one state/audio artifact are required for those dimensions; missing evidence remains unavailable. The supplied generic cases compare software captures and, where captured, whole PCM, but do not infer game-state equivalence from pixels or cycles. Quake's semantic completion is reported separately from comparative state parity. Celeste's actual frame deadlines and authored freeze classification remain the responsibility of its source-bound timing observer; controller polls and display refresh counts alone do not prove 60 fps.

For A/B, every non-candidate input must match, including the adapter, interpreter, tape and complete transitive input records. Only `candidate_inputs` may differ (normally guest, guest map and its disc). Both cases must use the same emulator, options, completion contract and quality contract. An image may contain different assets; matching route inputs do not by themselves prove visual or audio equivalence. Byte-different checkpoints are reported, never silently aligned or accepted. Supply separately reviewed semantic comparators when changing layout or timing makes raw comparisons unsuitable.

Guest instruction and bus-cycle counters are distinct from host elapsed time. Cycle profile columns are deltas; cumulative `bus_cycles` is never summed, and stack load stalls are a subset of RAM load stalls. No universal noise threshold or speedup verdict is supplied. Record a behavior-neutral control and review matched-poll/state/presentation alignment before accepting a performance change. Intensive PC profiling is opt-in; normal sparse software captures remain the default.
