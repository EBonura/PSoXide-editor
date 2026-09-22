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
# builds use one stage root, because profile names carry crate hashes derived
# from the stage path: a profile is regenerated here, never committed.
#
# Run as: make cortex-pgo CORTEX_PGO_TAPE=<route.pxtape> [CORTEX_PGO_STOP_POLL=N]
#
# RAM decides whether this helps. Measured 2026-09-22 on Tech Demo 0.4b
# (projects/default, cd-stream-bench lockstep-visuals, whole-level tape): a
# plain profile grows .text by 134,832 bytes and overflows RAM by 121,300;
# without -profile-sample-accurate even -hot-callsite-threshold=0 leaves less
# heap than the route allocates (8,620 bytes). With -profile-sample-accurate
# and a hot-callsite threshold of 250 or less the disc fits and replays
# identically, but gameplay was 1.6% to 3.2% SLOWER per presented frame
# (more memcpy calls, 4.6% to 5.2% more work instructions). Do not ship it
# until RAM is reclaimed; the default build is unaffected.
#
# Environment:
#   CORTEX_PGO_TAPE            poll-bound input tape to profile (required)
#   CORTEX_PGO_STOP_POLL       stop the profiling replay at this poll
#   CORTEX_PGO_PROJECT         project.ron (default: editor/projects/default)
#   CORTEX_PGO_WORK            scratch directory, no spaces (default: build/cortex-pgo)
#   CORTEX_PGO_EXTRA_RUSTFLAGS extra flags for the optimised build only, e.g.
#                              -Cllvm-args=-profile-sample-accurate
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

echo "cortex-pgo: profiling build"
build_disc "$PROFILE_FLAGS" "$WORK/collect.map" "$WORK/collect-build.txt"
CUE="$(ls "$(dirname "$PROJECT")"/baked/*.cue | head -1)"
[ -f "$CUE" ] || fail "no baked cue under $(dirname "$PROJECT")/baked"

echo "cortex-pgo: DWARF ELF of the same code"
(cd "$ROOT" && PSOXIDE_GUEST_EXTRA_RUSTFLAGS="$PROFILE_FLAGS" \
    PSOXIDE_GUEST_LINK_ELF="$WORK/cortex.elf" make build-editor-playtest) \
    > "$WORK/elf-build.txt" 2>&1 || { tail -20 "$WORK/elf-build.txt" >&2; fail "ELF build failed"; }

echo "cortex-pgo: profiling replay"
"$REPLAY_FRONTEND" launch --path "$CUE" --embedded-playtest --steps 6000000000 \
    --input-tape "$TAPE" ${CORTEX_PGO_STOP_POLL:+--stop-at-poll "$CORTEX_PGO_STOP_POLL"} \
    --pc-sample-log "$WORK/pc.csv" --pc-sample-instructions 61 \
    > "$WORK/replay.txt" 2>&1 || fail "profiling replay failed (see $WORK/replay.txt)"

echo "cortex-pgo: sample profile"
(cd "$ROOT" && cargo run --release --quiet -p psoxide-pgo -- \
    "$WORK/cortex.elf" "$WORK/pc.csv" "$WORK/cortex.prof") || fail "psoxide-pgo failed"
# The histogram is large and fully captured by the profile.
rm -f "$WORK/pc.csv"

echo "cortex-pgo: optimised build"
build_disc "$PROFILE_FLAGS -Zprofile-sample-use=$WORK/cortex.prof ${CORTEX_PGO_EXTRA_RUSTFLAGS:-}" \
    "$WORK/pgo.map" "$WORK/pgo-build.txt"
EXE="$ROOT/build/examples/mipsel-sony-psx/release/editor-playtest.exe"
python3 "$ROOT/tools/hazard_scan.py" "$EXE" || fail "load-delay hazards in $EXE"
# RAM is the risk: profile-driven inlining grows .text, and the heap is what
# is left between __bss_end and the reserved stack.
BSS_END="$(sed -n 's/^ *\([0-9a-f]*\) .* __bss_end = \.$/\1/p' "$WORK/pgo.map" | head -1)"
# The map prints __heap_end at the location counter, not its value; the ELF
# symbol table has the value (a linker-script constant, the same in every build).
HEAP_END="$(mipsel-none-elf-nm "$WORK/cortex.elf" | sed -n 's/^\([0-9a-f]*\) . __heap_end$/\1/p')"
echo "cortex-pgo: __bss_end 0x$BSS_END, heap $((0x$HEAP_END - 0x$BSS_END)) bytes"
shasum -a 256 "$EXE"
echo "cortex-pgo: disc $CUE"
