#!/bin/sh
# Cortex Ignition before/after benchmark on the tracked whole-level tape.
#
# Builds the frontend, cooks and builds the project disc with a linker map
# (in a private guest stage root so the editor's own Play stage is untouched),
# runs the 64-bit symbol gate, replays the poll-bound tape twice, checks the
# two replays are byte-identical, and prints one row of numbers. With
# CORTEX_BENCH_BASELINE=<dir of a previous run> it prints before/after.
#
# The project is Tech Demo 0.4b, editor/projects/default (the project the demo
# disc bakes). It is baked from a copy under $OUT, never in place, so the bench
# writes nothing into the live project.
#
# The tape is Manny's whole-level recording (cortex-0.4/whole-level.pxtape,
# 2026-09-05) with the skippable opening added later (2026-09-07) spliced in:
# replayed as recorded, its welcome-panel presses land in the cinematic and the
# player never gets control. whole-level-skip.pxtape inserts 60 polls of held
# Cross (the skip needs 30 held ticks) and then 40 idle polls before poll 560,
# and keeps the rest of the recording unchanged: 5,183 polls, 177..5359.
# PXITAPE2 layout: magic, u32 samples, u32 first poll, then per poll
# <u16 buttons, u8 lx, ly, rx, ry>; Cross is 0x4000, sticks idle at 128.
#
# Run as: make cortex-bench            (or CORTEX_BENCH_OUT=... make cortex-bench)
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_PROJECT="${CORTEX_BENCH_PROJECT:-editor/projects/default/project.ron}"
TAPE="${CORTEX_BENCH_TAPE:-editor/archive/fixtures/cortex-0.4/whole-level-skip.pxtape}"
OUT="${CORTEX_BENCH_OUT:-${TMPDIR:-/tmp}/psoxide-cortex-bench}"
STAGE_ROOT="${CORTEX_BENCH_STAGE_ROOT:-${TMPDIR:-/tmp}/psoxide-psx-guest-cortex-bench}"
# lockstep-visuals renders exactly once per two fixed ticks regardless of
# wall-clock vblanks, so two builds of different speed present the same guest
# states and their display/VRAM hashes are comparable. Without it a code-layout
# change alone moves which vblank a frame lands on and the hashes diverge
# (observed 2026-09-01: an inlining change diverged at route tick 978, in the
# menu, before any gameplay). Shipping cadence measurements use
# CORTEX_BENCH_FEATURES="cd-stream-bench".
FEATURES="${CORTEX_BENCH_FEATURES:-cd-stream-bench lockstep-visuals}"
STEPS="${CORTEX_BENCH_STEPS:-6000000000}"
# Stop both replays in the same guest state: a run that merely exhausts the
# tape can end a poll early or late. 5340 is the last checkpoint where a no-op
# build (+1% work) still matched display and VRAM; 5350 already moved.
STOP_POLL="${CORTEX_BENCH_STOP_POLL:-5340}"
FRONTEND="$ROOT/target/release/frontend"

fail() { echo "cortex-bench: FAIL: $1" >&2; exit 1; }

cd "$ROOT"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
[ -f "$TAPE" ] || fail "tape $TAPE not found"
[ -f "$SOURCE_PROJECT" ] || fail "project $SOURCE_PROJECT not found"

# Bake a copy (a clone on APFS) without the old bake and review captures.
PROJECT_DIR="$OUT/project"
rm -rf "$PROJECT_DIR"
mkdir -p "$PROJECT_DIR"
for entry in "$(dirname "$SOURCE_PROJECT")"/* "$(dirname "$SOURCE_PROJECT")"/.[!.]*; do
    [ -e "$entry" ] || continue
    case "$(basename "$entry")" in baked|review) continue ;; esac
    cp -cR "$entry" "$PROJECT_DIR/" 2>/dev/null || cp -R "$entry" "$PROJECT_DIR/"
done
PROJECT="$PROJECT_DIR/$(basename "$SOURCE_PROJECT")"

echo "cortex-bench: frontend"
(cd emu && cargo build -p frontend --release --quiet)

# A reused stage root can hand the disc a stale guest executable: the staged
# sources are rsynced with their mtimes, so cargo may keep an older rlib or
# skip the final link (observed 2026-09-01: two benches of different commits
# produced the same exe hash). Start from an empty stage unless told to keep it.
if [ "${CORTEX_BENCH_KEEP_STAGE:-0}" != "1" ]; then
    rm -rf "$STAGE_ROOT"
fi
echo "cortex-bench: disc ($FEATURES) with link map"
(cd emu && EDITOR_PLAYTEST_FEATURES="$FEATURES" \
    PSOXIDE_GUEST_STAGE_ROOT="$STAGE_ROOT" \
    PSOXIDE_GUEST_CARGO_HOME="${PSOXIDE_GUEST_CARGO_HOME:-/tmp/psoxide-psx-guest-v1/cargo-home}" \
    PSOXIDE_GUEST_LINK_MAP="$OUT/link.map" \
    "$FRONTEND" build-project-disc --project "$PROJECT" > "$OUT/disc-build.txt" 2>&1) \
    || { tail -20 "$OUT/disc-build.txt" >&2; fail "disc build failed"; }
CUE="$(ls "$(dirname "$PROJECT")"/baked/*.cue | head -1)"
[ -f "$CUE" ] || fail "no baked cue under $(dirname "$PROJECT")/baked"
shasum -a 256 build/examples/mipsel-sony-psx/release/editor-playtest.exe | tee "$OUT/exe.sha256"

echo "cortex-bench: symbol gate"
sh tools/guest_symbol_gate.sh "$OUT/link.map" | tee "$OUT/symbol-gate.txt" || true

for RUN in 1 2; do
    echo "cortex-bench: replay $RUN/2"
    # The tape-end frame is the same guest state on every build (poll-bound
    # tape), so it doubles as the visual A/B pair; dump it at 4x.
    PSOXIDE_HW_DUMP_SCALE=4 "$FRONTEND" launch --path "$CUE" --embedded-playtest \
        --input-tape "$TAPE" --steps "$STEPS" --stop-at-poll "$STOP_POLL" --dump-hash \
        --route-log "$OUT/route-$RUN.csv" \
        --cpu-cycle-profile-log "$OUT/cycles-$RUN.csv" \
        --pc-line-log "$OUT/pcline-$RUN.csv" \
        --dump-hw "$OUT/final-$RUN.ppm" \
        > "$OUT/run-$RUN.txt" 2>&1 || fail "replay $RUN failed (see $OUT/run-$RUN.txt)"
done

python3 tools/cortex_bench_report.py "$OUT" ${CORTEX_BENCH_BASELINE:+--baseline "$CORTEX_BENCH_BASELINE"}
