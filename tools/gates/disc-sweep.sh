#!/bin/sh
# The 20-seed disc duel sweep, failing fast and never reusing a stale disc.
#
#   tools/gates/disc-sweep.sh <outdir> [--jobs N]
#
# 1. preflight.sh: every pinned component must be fetchable from origin.
# 2. For each project, delete the previous disc (cue and bin) from its baked/
#    directory, build a fresh one, and stop on any failure or missing cue. A
#    build that fails can therefore never leave an older disc for the duel to
#    pick up.
# 3. Run that project's seeds with duel-baseline --skip-build, `--jobs N` at a
#    time (default: performance cores minus one).
#
# Discs are built one after another: a disc build cooks into the checkout's
# shared generated/ directory and the shared guest stage, so two builds in one
# checkout would overwrite each other. The seeds are what costs time, and they
# run concurrently.
#
# Environment:
#   FRONTEND        psoxide-emulator built with --features editor
#                   (default $CARGO_TARGET_DIR/release/psoxide-emulator)
#   DUEL_BASELINE   the duel-baseline example (default .../release/examples/duel-baseline)
#   SWEEP_SPECS     space separated project:seeds (default: the 20-seed sweep)
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
FRONTEND="${FRONTEND:-$TARGET/release/psoxide-emulator}"
DUEL_BASELINE="${DUEL_BASELINE:-$TARGET/release/examples/duel-baseline}"
SPECS="${SWEEP_SPECS:-grid-dungeon:1,2,3,4,5,6,7,8 grid-level008:1,2,3,4 grid-cave:1,2,3,4 grid-cathedral:1,2,3,4}"

OUT="${1:?usage: disc-sweep.sh <outdir> [--jobs N]}"
shift
JOBS_ARGS=""
if [ "${1:-}" = "--jobs" ]; then JOBS_ARGS="--jobs ${2:?--jobs needs a number}"; fi

[ -x "$FRONTEND" ] || { echo "sweep: no frontend at $FRONTEND" >&2; exit 1; }
[ -x "$DUEL_BASELINE" ] || { echo "sweep: no duel-baseline at $DUEL_BASELINE" >&2; exit 1; }

cd "$ROOT"
sh tools/gates/preflight.sh
mkdir -p "$OUT"

for spec in $SPECS; do
    p="${spec%%:*}"
    baked="projects/$p/baked"
    rm -f "$baked"/*.cue "$baked"/*.bin
    EDITOR_PLAYTEST_FEATURES="cd-stream-bench host-tty" \
        "$FRONTEND" build-project-disc --project "projects/$p" >"$OUT/build-$p.log" 2>&1 \
        || { echo "sweep: disc build for $p FAILED, see $OUT/build-$p.log" >&2; tail -20 "$OUT/build-$p.log" >&2; exit 1; }
    ls "$baked"/*.cue >/dev/null 2>&1 \
        || { echo "sweep: build for $p printed success but left no cue" >&2; exit 1; }
    echo "sweep: built $p"
done

status=0
for spec in $SPECS; do
    p="${spec%%:*}"; seeds="${spec#*:}"
    # shellcheck disable=SC2086
    "$DUEL_BASELINE" --frontend "$FRONTEND" --project "projects/$p" --seeds "$seeds" \
        --skip-build --out "$OUT/$p" $JOBS_ARGS >"$OUT/sweep-$p.log" 2>&1 \
        || { echo "sweep: duel-baseline for $p FAILED, see $OUT/sweep-$p.log" >&2; status=1; }
    echo "== $p"; cat "$OUT/sweep-$p.log"
done
[ "$status" -eq 0 ] || exit 1
echo sweepdone
