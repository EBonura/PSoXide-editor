#!/bin/sh
# Run a gate, or reuse the stored result of an earlier run whose inputs were
# byte-identical. Only for gates whose whole input set is enumerable.
#
#   tools/gates/reuse.sh [--no-reuse] [--dry-key] <name> [--file PATH]... [--tag TEXT]... -- <command> [args]
#
# The key is a SHA-256 over everything that can change the result:
#   - every file in the repository checkout (hydrated SDK and emulator
#     components, projects, assets, Cargo.lock, components.lock.json,
#     rust-toolchain.toml, tools/gates), except .git, .tools, build, target,
#     baked disc output and the result store itself;
#   - the gate's command line, this script, and the toolchain
#     (`rustc -vV`, `cargo -V`);
#   - the environment that Cargo, rustc, the editor or the SDK read
#     (CARGO_*, RUST*, PSOXIDE_*, EDITOR_*, XDG_*, HOME), except the target
#     directory, job count and incremental flag, which do not change results;
#   - each `--file PATH` (a binary the gate runs, hashed by content) and each
#     `--tag TEXT` (any further input the caller can name).
# A tracked file added, removed, renamed or changed by one byte changes the key.
#
# Only a passing run (exit 0) is stored, in $GATES_RESULTS (default
# <repo>/.gate-results). A hit prints "reused result from <time> (inputs
# identical), key <short>, run at <head sha>" and replays the stored output and
# exit status. Any doubt about the inputs means do not use this wrapper, or
# pass --no-reuse, which always runs and refreshes the stored result.
#
# Never wrap a gate that depends on the guest build (the disc duel sweep, route
# and tape gates) unless the guest exe sha and disc sha are passed with
# --file or --tag; the sweep is not wrapped.
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STORE="${GATES_RESULTS:-$ROOT/.gate-results}"
reuse=1; dry=0
while :; do
    case "${1:-}" in
        --no-reuse) reuse=0; shift ;;
        --dry-key) dry=1; shift ;;
        *) break ;;
    esac
done
NAME="${1:?usage: reuse.sh [--no-reuse] <name> [--file PATH] [--tag TEXT] -- command}"
shift
extra=""
while [ "${1:-}" != "--" ]; do
    case "${1:-}" in
        --file) [ -f "${2:?--file needs a path}" ] || { echo "reuse: $2 is not a file" >&2; exit 1; }
                extra="$extra
file $(shasum -a 256 "$2" | cut -d' ' -f1)"; shift 2 ;;
        --tag) extra="$extra
tag ${2:?--tag needs text}"; shift 2 ;;
        *) echo "reuse: expected -- before the command" >&2; exit 1 ;;
    esac
done
shift
[ "$#" -gt 0 ] || { echo "reuse: no command" >&2; exit 1; }

cd "$ROOT"
key=$(
    {
        echo "name $NAME"
        echo "command $*"
        echo "script $(shasum -a 256 "$0" | cut -d' ' -f1)"
        echo "rustc $(rustc -vV 2>&1)"
        echo "cargo $(cargo -V 2>&1)"
        env | grep -E '^(CARGO|RUST|PSOXIDE|EDITOR_|XDG_|HOME=)' \
            | grep -vE '^(CARGO_TARGET_DIR|CARGO_BUILD_JOBS|CARGO_INCREMENTAL|PSOXIDE_GUEST_STAGE_ROOT)=' | LC_ALL=C sort || true
        printf '%s\n' "$extra"
        find . \( -name .git -o -name .tools -o -name build -o -name target -o -name baked \
                  -o -name .gate-results \) -prune -o -type f -print0 \
            | LC_ALL=C sort -z | xargs -0 shasum -a 256
    } | shasum -a 256 | cut -d' ' -f1
)
short=$(printf %s "$key" | cut -c1-12)
dir="$STORE/$NAME/$key"
if [ "$dry" -eq 1 ]; then echo "$key"; exit 0; fi

if [ "$reuse" -eq 1 ] && [ -f "$dir/output" ] && [ -f "$dir/meta" ]; then
    echo "reused result from $(sed -n 's/^when //p' "$dir/meta") (inputs identical), key $short, run at $(sed -n 's/^head //p' "$dir/meta")"
    cat "$dir/output"
    exit 0
fi

mkdir -p "$dir.tmp"
set +e
"$@" >"$dir.tmp/output" 2>&1
rc=$?
set -e
cat "$dir.tmp/output"
if [ "$rc" -eq 0 ]; then
    printf 'when %s\nhead %s\ncommand %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" \
        "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)" "$*" >"$dir.tmp/meta"
    rm -rf "$dir"
    mv "$dir.tmp" "$dir"
    echo "gate $NAME ran; result stored, key $short"
else
    rm -rf "$dir.tmp"
    echo "gate $NAME FAILED (exit $rc); nothing stored" >&2
fi
exit "$rc"
