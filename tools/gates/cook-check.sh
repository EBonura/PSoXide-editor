#!/bin/sh
# cook_check on every project (or the directories given), returning as each
# answer arrives. Build first (see tools/README.md, "Running the full gate").
#
#   tools/gates/cook-check.sh [--out-dir DIR] [--scratch DIR] [projects/<name> ...]
#
# With no project directories it runs all of projects/*/.
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
cd "$ROOT"
case " $* " in
    *" projects/"*) all="" ;;
    *) all="--all" ;;
esac
# shellcheck disable=SC2086
exec "$TARGET/release/examples/cook-check" --bin "$TARGET/release/psxed-mcp" $all "$@"
