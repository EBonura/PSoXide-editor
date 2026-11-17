#!/bin/sh
# cook_check on every project (or the directories given), returning as each
# answer arrives, and reusing the stored result when every input is
# byte-identical (see reuse.sh for exactly what the key covers).
# Build first (see tools/README.md, "Running the full gate").
#
#   tools/gates/cook-check.sh [--no-reuse] [--out-dir DIR] [--scratch DIR] [projects/<name> ...]
#
# With no project directories it runs all of projects/*/. --out-dir gets one
# cc-<project>.json per project whether the result was run or reused.
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
cd "$ROOT"
flags=""; out=""; all="--all"; projs=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --no-reuse) flags="--no-reuse" ;;
        --out-dir) out="$2"; shift ;;
        --scratch) shift ;; # the scratch location never changes the answer; the default is used
        projects/*) all=""; projs="$projs $1" ;;
        *) echo "cook-check: unknown argument $1" >&2; exit 1 ;;
    esac
    shift
done
log="$(mktemp)"
trap 'rm -f "$log"' EXIT HUP INT TERM
rc=0
# Output and scratch locations do not change the result, so they stay out of the command (and the key).
tag="projects:${all:-$projs}"
# shellcheck disable=SC2086
sh tools/gates/reuse.sh $flags cook-check \
    --file "$TARGET/release/psxed-mcp" --file "$TARGET/release/examples/cook-check" \
    --tag "$tag" -- \
    "$TARGET/release/examples/cook-check" --bin "$TARGET/release/psxed-mcp" $all $projs >"$log" 2>&1 || rc=$?
cat "$log"
# A reused run replays the lines but does not rewrite the per-project files.
if [ -n "$out" ]; then
    mkdir -p "$out"
    sed -n 's/^== \([^:]*\): \(.*\)$/\1 \2/p' "$log" | while read -r name line; do
        printf '%s\n' "$line" >"$out/cc-$name.json"
    done
fi
exit "$rc"
