#!/bin/sh
# The editor suites (psxed-project, psxed-ui, psxed-mcp lib), reused when every
# input is byte-identical. `--no-reuse` forces a run.
#
#   tools/gates/editor-suites.sh [--no-reuse]
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
exec sh tools/gates/reuse.sh "$@" editor-suites -- sh -c \
    'cargo test -p psxed-project && cargo test -p psxed-ui && cargo test -p psxed-mcp --lib'
