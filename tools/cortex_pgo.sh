#!/bin/sh
# Profile-guided Cortex disc, driven by the emulator's own instruction counts.
#
# PSoXide counts every guest instruction exactly, so the guest needs no
# instrumentation: a build with profiling debug info replays a tape,
# psoxide-pgo turns the PC histogram into an LLVM sample profile through a
# DWARF ELF of the same code, and the disc is rebuilt with that profile. This
# is the Cortex counterpart of hl-psx's `cargo run --release -- pgo --tape`.
#
# The result lands where `frontend build-project-disc` always puts it (the
# project's baked/ directory); the default build is unchanged. All three guest
# builds use one stage root and one EDITOR_PLAYTEST_FEATURES, because profile
# names carry crate hashes derived from the stage path AND the cargo features:
# a profile from a build with other features matches no function. A profile
# is regenerated here, never committed.
#
# Run as: make cortex-pgo CORTEX_PGO_TAPE=<route.pxtape> [CORTEX_PGO_POLLS=A..B]
#
# Measured 2026-09-22 on Tech Demo 0.4b (projects/default, cd-stream-bench
# lockstep-visuals, spliced whole-level tape, gameplay polls 1412..5340, loads
# excluded), against the same source built without a profile:
#
# - Every figure below had the UI SFX bank read from UI.PAK at boot instead of
#   kept in RAM. That change is not in this tree yet (it waits on a console
#   check), and without it the optimised build does not fit: with the default
#   flags below it overflows RAM by 47,572 bytes (measured 2026-09-22).
#
# - CORTEX_PGO_FLAGS below, trained on polls 1412..3900: +1.2% fps, -0.7% work
#   instructions, -14% I-cache refill stalls, and +1.1% fps on polls
#   3900..5340, which the profile never saw. Two other training windows gave
#   +0.3% and +1.3%, so treat about 1% as the expected gain. Trained on the
#   whole window 1412..5340: +1.9% fps, -1.2% work instructions.
# - -pgso=false: with -profile-sample-accurate, LLVM's profile-guided size
#   optimisation compiles every block the profile calls cold for size, and
#   the struct copies there become memcpy/memset calls (346 memcpy call sites
#   against 141 without it and 115 with no profile). It costs about 15 KB.
# - -hot-callsite-threshold=1500: LLVM's default threshold overflows RAM by
#   8.7 KB with -pgso=false (and leaves 3.4 KB of heap with it on, less than
#   the route allocates: 8,620 B). 1500 leaves about 40 KB; 1000 leaves about
#   69 KB and was 0.04 to 0.35 points slower.
# - A profile without -profile-sample-accurate grows .text by about 157 KB and
#   still overflows RAM by 31 to 39 KB, even with the UI SFX bank on the disc.
# - -sample-profile-use-profi was mixed: +0.4 points with one profile, -1.1
#   with another.
#
# Environment:
#   CORTEX_PGO_TAPE            poll-bound input tape to profile (required)
#   CORTEX_PGO_STOP_POLL       stop the profiling replay at this poll
#   CORTEX_PGO_POLLS           FROM..TO: train on these port-1 polls only (the
#                              gameplay window, loads and menus excluded); the
#                              replay stops at TO
#   CORTEX_PGO_PROFILE         reuse this profile (made from the same stage root
#                              and features) and only run the optimised build
#   CORTEX_PGO_CONVERTER       psoxide-pgo executable (default: the locked one)
#   CORTEX_PGO_PROJECT         project.ron (default: editor/projects/default)
#   CORTEX_PGO_WORK            scratch directory, no spaces (default: build/cortex-pgo)
#   CORTEX_PGO_FLAGS           profile-use flags for the optimised build
#                              (default: the measured variant above)
#   CORTEX_PGO_EXTRA_RUSTFLAGS further flags for the optimised build only
#   CORTEX_PGO_REPLAY_FRONTEND frontend for the profiling replay
#   EDITOR_PLAYTEST_FEATURES, PSOXIDE_GUEST_STAGE_ROOT: as for every guest build
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="${CORTEX_PGO_PROJECT:-editor/projects/default/project.ron}"
case "$PROJECT" in /*) ;; *) PROJECT="$ROOT/$PROJECT" ;; esac
TAPE="${CORTEX_PGO_TAPE:?set CORTEX_PGO_TAPE to a poll-bound .pxtape to profile}"
WORK="${CORTEX_PGO_WORK:-$ROOT/build/cortex-pgo}"
FRONTEND="$ROOT/target/release/frontend"
REPLAY_FRONTEND="${CORTEX_PGO_REPLAY_FRONTEND:-$FRONTEND}"
# Line tables and discriminators for sample profiling. The flat image drops
# them at link time, so the profiled build runs like any other.
PROFILE_FLAGS="-Cdebuginfo=1 -Zdebug-info-for-profiling -Cstrip=none"
# How the optimised build uses the profile; see the measurements above.
PGO_FLAGS="${CORTEX_PGO_FLAGS--Cllvm-args=-profile-sample-accurate -Cllvm-args=-pgso=false -Cllvm-args=-hot-callsite-threshold=1500}"

fail() { echo "cortex-pgo: FAIL: $1" >&2; exit 1; }
# RUSTFLAGS are split on whitespace, so the profile path cannot contain any.
case "$WORK" in *" "*) fail "CORTEX_PGO_WORK must not contain spaces: $WORK" ;; esac
[ -f "$TAPE" ] || fail "tape $TAPE not found"
[ -f "$PROJECT" ] || fail "project $PROJECT not found"

rm -rf "$WORK"
mkdir -p "$WORK"

echo "cortex-pgo: frontend"
(cd "$ROOT/emu" && cargo build -p frontend --release --quiet)

build_disc() {
    (cd "$ROOT/emu" && PSOXIDE_GUEST_EXTRA_RUSTFLAGS="$1" PSOXIDE_GUEST_LINK_MAP="$2" \
        "$FRONTEND" build-project-disc --project "$PROJECT") > "$3" 2>&1 \
        || { tail -20 "$3" >&2; fail "disc build failed (see $3)"; }
}

if [ -n "${CORTEX_PGO_PROFILE:-}" ]; then
    cp "$CORTEX_PGO_PROFILE" "$WORK/cortex.prof"
else
    echo "cortex-pgo: profiling build"
    build_disc "$PROFILE_FLAGS" "$WORK/collect.map" "$WORK/collect-build.txt"
    CUE="$(ls "$(dirname "$PROJECT")"/baked/*.cue | head -1)"
    [ -f "$CUE" ] || fail "no baked cue under $(dirname "$PROJECT")/baked"

    echo "cortex-pgo: DWARF ELF of the same code"
    (cd "$ROOT" && PSOXIDE_GUEST_EXTRA_RUSTFLAGS="$PROFILE_FLAGS" \
        PSOXIDE_GUEST_LINK_ELF="$WORK/cortex.elf" make build-editor-playtest) \
        > "$WORK/elf-build.txt" 2>&1 || { tail -20 "$WORK/elf-build.txt" >&2; fail "ELF build failed"; }

    echo "cortex-pgo: profiling replay"
    if [ -n "${CORTEX_PGO_POLLS:-}" ]; then
        # The frontend samples from boot, so it samples in 30-tick windows and a
        # route log of the same run maps ticks to polls: only windows wholly inside
        # the gameplay polls reach the profile (the SDK's `collect --polls` rule).
        FROM="${CORTEX_PGO_POLLS%..*}"; TO="${CORTEX_PGO_POLLS#*..}"
        "$REPLAY_FRONTEND" launch --path "$CUE" --embedded-playtest --steps 6000000000 \
            --input-tape "$TAPE" --stop-at-poll "$TO" --route-log "$WORK/route.csv" \
            --pc-sample-window-log "$WORK/pc-windows.csv" --pc-sample-window-ticks 30 \
            --pc-sample-instructions 61 \
            > "$WORK/replay.txt" 2>&1 || fail "profiling replay failed (see $WORK/replay.txt)"
        python3 - "$WORK/route.csv" "$WORK/pc-windows.csv" "$WORK/pc.csv" "$FROM" "$TO" <<'PY' \
        || fail "no gameplay sample windows"
import csv, sys
from collections import Counter
route, windows, out, lo, hi = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
polls = {int(r["route_tick"]): int(r["port1_polls"]) for r in csv.DictReader(open(route))}
kept, seen, hist = set(), set(), Counter()
for r in csv.DictReader(open(windows)):
    start = int(r["window_start_tick"])
    first, last = start + 1, start + 30
    seen.add(start)
    # Ticks first..last all ran inside the window: none started before poll
    # lo (polls completed by the end of tick first - 1) and none ran past hi.
    if last in polls and polls[first - 1] >= lo and polls[last] <= hi:
        kept.add(start)
        hist[r["pc"]] += int(r["samples"])
with open(out, "w") as f:
    f.write("pc,samples\n")
    f.writelines(f"{pc},{n}\n" for pc, n in hist.items())
print(f"cortex-pgo: polls {lo}..{hi}: kept {len(kept)} of {len(seen)} sample windows, {sum(hist.values())} samples")
sys.exit(0 if kept else 1)
PY
        rm -f "$WORK/pc-windows.csv"
    else
        "$REPLAY_FRONTEND" launch --path "$CUE" --embedded-playtest --steps 6000000000 \
            --input-tape "$TAPE" ${CORTEX_PGO_STOP_POLL:+--stop-at-poll "$CORTEX_PGO_STOP_POLL"} \
            --pc-sample-log "$WORK/pc.csv" --pc-sample-instructions 61 \
            > "$WORK/replay.txt" 2>&1 || fail "profiling replay failed (see $WORK/replay.txt)"
    fi

    echo "cortex-pgo: sample profile"
    if [ -n "${CORTEX_PGO_CONVERTER:-}" ]; then
        "$CORTEX_PGO_CONVERTER" "$WORK/cortex.elf" "$WORK/pc.csv" "$WORK/cortex.prof" || fail "psoxide-pgo failed"
    else
        (cd "$ROOT" && cargo run --release --quiet -p psoxide-pgo -- \
            "$WORK/cortex.elf" "$WORK/pc.csv" "$WORK/cortex.prof") || fail "psoxide-pgo failed"
    fi
    # The histogram is large and fully captured by the profile.
    rm -f "$WORK/pc.csv"
fi

echo "cortex-pgo: optimised build"
build_disc "$PROFILE_FLAGS -Zprofile-sample-use=$WORK/cortex.prof $PGO_FLAGS ${CORTEX_PGO_EXTRA_RUSTFLAGS:-}" \
    "$WORK/pgo.map" "$WORK/pgo-build.txt"
EXE="$ROOT/build/examples/mipsel-sony-psx/release/editor-playtest.exe"
python3 "$ROOT/tools/hazard_scan.py" "$EXE" || fail "load-delay hazards in $EXE"
# RAM is the risk: profile-driven inlining grows .text, and the heap is what
# is left between __bss_end and the reserved stack.
BSS_END="$(sed -n 's/^ *\([0-9a-f]*\) .* __bss_end = \.$/\1/p' "$WORK/pgo.map" | head -1)"
# The map prints __heap_end at the location counter, not its value; the ELF
# symbol table has the value (a linker-script constant, the same in every build).
HEAP_END="$(mipsel-none-elf-nm "$WORK/cortex.elf" 2>/dev/null | sed -n 's/^\([0-9a-f]*\) . __heap_end$/\1/p')"
HEAP_END="${HEAP_END:-801f7f00}"
echo "cortex-pgo: __bss_end 0x$BSS_END, heap $((0x$HEAP_END - 0x$BSS_END)) bytes"
shasum -a 256 "$EXE"
echo "cortex-pgo: disc $(ls "$(dirname "$PROJECT")"/baked/*.cue | head -1)"
