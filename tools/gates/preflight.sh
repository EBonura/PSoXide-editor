#!/bin/sh
# Refuse to run a gate when a pinned component is not on origin.
#
# Every component in components.lock.json is fetched by revision the way Cargo
# and the component hydrator do it (`git fetch <repo> <sha>`), into a throwaway
# repository, so an unpushed or unreachable pin fails here, in seconds, rather
# than deep inside a disc build.
#
#   tools/gates/preflight.sh [components.lock.json]
#
# GATES_REMOTE_BASE overrides the host (default https://github.com).
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOCK="${1:-$ROOT/components.lock.json}"
BASE="${GATES_REMOTE_BASE:-https://github.com}"
[ -f "$LOCK" ] || { echo "preflight: no lock file at $LOCK" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT HUP INT TERM
git init -q --bare "$WORK/probe"

fail=0
for row in $(jq -r '.components | to_entries[] | "\(.key),\(.value.repository),\(.value.revision)"' "$LOCK"); do
    name="${row%%,*}"; rest="${row#*,}"; repo="${rest%%,*}"; rev="${rest#*,}"
    if GIT_TERMINAL_PROMPT=0 git -C "$WORK/probe" fetch -q --depth=1 --no-tags \
        "$BASE/$repo" "$rev" >"$WORK/$name.log" 2>&1; then
        echo "preflight: $name $rev reachable on $repo"
    else
        echo "preflight: FAIL $name $rev is not reachable on $BASE/$repo (push it first)" >&2
        sed 's/^/    /' "$WORK/$name.log" >&2
        fail=1
    fi
done
[ "$fail" -eq 0 ] || exit 1
