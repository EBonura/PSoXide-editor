#!/bin/sh
# Prove that a one-byte change in each input class forces a rerun, and that an
# identical rerun is reused. Works on a throwaway copy of the gate scripts with
# one stand-in file per input class, so it never touches the real checkout.
#
#   sh tools/gates/test-reuse.sh
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
T="$(mktemp -d)"; C="$(mktemp -d)"
trap 'rm -rf "$T" "$C"' EXIT HUP INT TERM
mkdir -p "$T/tools/gates" "$T/editor/src" "$T/sdk/crates" "$T/projects/p" "$T/assets" "$T/bin" "$T/shim"
cp "$HERE/reuse.sh" "$T/tools/gates/"
printf 'fn main() {}\n' >"$T/editor/src/lib.rs"            # tracked source
printf 'sdk component\n' >"$T/sdk/crates/lib.rs"          # hydrated (ignored) component file
printf 'lock\n' >"$T/Cargo.lock"
printf '{"sdk":"aaaa"}\n' >"$T/components.lock.json"      # pinned SDK/emulator shas
printf 'project\n' >"$T/projects/p/project.ron"
printf 'asset\n' >"$T/assets/a.bin"
printf 'binary\n' >"$T/bin/gate-bin"                        # a binary the gate runs
printf 'stable\n' >"$T/rust-toolchain.toml"

cd "$T"
run() { sh tools/gates/reuse.sh --dry-key g --file bin/gate-bin --tag "t1" -- echo gate; }
base=$(run)
fail=0
expect_changed() { # name, command that perturbs one input
    ( eval "$2" )
    k=$(run)
    if [ "$k" = "$base" ]; then echo "FAIL  $1: key unchanged"; fail=1; else echo "ok    $1: key changed, rerun forced"; fi
    ( eval "$3" )
    [ "$(run)" = "$base" ] || { echo "FAIL  restoring $1 did not restore the key"; fail=1; }
}
expect_changed "source file, one byte"         "printf 'fn main() {};\n' >editor/src/lib.rs" "printf 'fn main() {}\n' >editor/src/lib.rs"
expect_changed "hydrated SDK file, one byte"   "printf 'sdk componenT\n' >sdk/crates/lib.rs"    "printf 'sdk component\n' >sdk/crates/lib.rs"
expect_changed "Cargo.lock, one byte"          "printf 'locK\n' >Cargo.lock"                    "printf 'lock\n' >Cargo.lock"
expect_changed "components.lock.json (pin)"    "printf '{\"sdk\":\"aaab\"}\n' >components.lock.json" "printf '{\"sdk\":\"aaaa\"}\n' >components.lock.json"
expect_changed "project data, one byte"        "printf 'projecT\n' >projects/p/project.ron"     "printf 'project\n' >projects/p/project.ron"
expect_changed "asset, one byte"               "printf 'asseT\n' >assets/a.bin"                 "printf 'asset\n' >assets/a.bin"
expect_changed "rust-toolchain.toml"           "printf 'stablE\n' >rust-toolchain.toml"         "printf 'stable\n' >rust-toolchain.toml"
expect_changed "gate binary (--file), one byte" "printf 'binarY\n' >bin/gate-bin"               "printf 'binary\n' >bin/gate-bin"
expect_changed "file added"                    "printf x >editor/src/new.rs"                    "rm editor/src/new.rs"
expect_changed "file deleted"                  "mv assets/a.bin assets/a.bak"                   "mv assets/a.bak assets/a.bin"
expect_changed "file renamed"                  "mv assets/a.bin assets/b.bin"                   "mv assets/b.bin assets/a.bin"
expect_changed "gate script itself"            "printf '# x\n' >>tools/gates/reuse.sh"          "cp '$HERE/reuse.sh' tools/gates/reuse.sh"
if [ "$(RUSTFLAGS=-Cx run)" = "$base" ]; then echo "FAIL  environment: key unchanged"; fail=1; else echo "ok    environment (RUSTFLAGS): key changed"; fi
if [ "$(sh tools/gates/reuse.sh --dry-key g --file bin/gate-bin --tag t2 -- echo gate)" = "$base" ]; then echo "FAIL  tag"; fail=1; else echo "ok    tag: key changed"; fi
if [ "$(sh tools/gates/reuse.sh --dry-key g --file bin/gate-bin --tag t1 -- echo gate2)" = "$base" ]; then echo "FAIL  command"; fail=1; else echo "ok    command line: key changed"; fi
printf '#!/bin/sh\necho "rustc 0.0.0-other"\n' >shim/rustc; chmod +x shim/rustc
if [ "$(PATH="$T/shim:$PATH" run)" = "$base" ]; then echo "FAIL  toolchain"; fail=1; else echo "ok    toolchain version: key changed"; fi

# Reuse, --no-reuse, and failures are never stored.
GATES_RESULTS="$C/store"; export GATES_RESULTS
g() { sh tools/gates/reuse.sh "$@" cnt --file bin/gate-bin -- sh -c 'echo run >>"$0"; echo output-line' "$C/count"; }
g >/dev/null; g >/dev/null; out=$(g)
[ "$(wc -l <"$C/count" | tr -d ' ')" = 1 ] && echo "ok    identical inputs: ran once, then reused" || { echo "FAIL  not reused"; fail=1; }
echo "$out" | grep -q '^reused result from .* (inputs identical)' && echo "ok    reuse message printed" || { echo "FAIL  no message"; fail=1; }
echo "$out" | grep -q 'output-line' && echo "ok    stored output replayed" || { echo "FAIL  output not replayed"; fail=1; }
g --no-reuse >/dev/null
[ "$(wc -l <"$C/count" | tr -d ' ')" = 2 ] && echo "ok    --no-reuse ran again" || { echo "FAIL  --no-reuse"; fail=1; }
printf 'fn main() {};\n' >editor/src/lib.rs; g >/dev/null
[ "$(wc -l <"$C/count" | tr -d ' ')" = 3 ] && echo "ok    changed input ran again" || { echo "FAIL  changed input"; fail=1; }
sh tools/gates/reuse.sh failing --file bin/gate-bin -- false >/dev/null 2>&1 || true
sh tools/gates/reuse.sh failing --file bin/gate-bin -- false >/dev/null 2>&1 && { echo "FAIL  failure reused"; fail=1; } || echo "ok    a failing gate is never stored or reused"
exit "$fail"
