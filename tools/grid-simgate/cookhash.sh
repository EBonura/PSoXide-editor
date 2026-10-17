#!/bin/zsh
# cookhash.sh LABEL PROJECT... : cook each project (editor/projects/PROJECT) with the tree's own cook and
# record the SHA-256 (first 16 hex digits) of the cooked grid level core and room blobs and of the level manifest, so a later
# change that must not touch existing levels can prove it with `diff`. Output: build/simgate/cookhash-LABEL.txt
root=${SIMGATE_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}
label=$1; shift
out=$root/build/simgate/cookhash-$label.txt; : >| $out
gen=$root/engine/examples/editor-playtest/generated
fnv() { cat "$@" | shasum -a 256 | cut -c1-16; }
for p in "$@"; do
  (cd $root/editor && $root/target/release/cook-playtest $root/editor/projects/$p/project.ron >/dev/null 2>&1) || { echo "$p COOK FAILED" >> $out; continue; }
  echo "$p core=$(fnv $gen/grid_level.pgl) rooms=$(fnv $gen/grid_rooms.pgr) manifest=$(fnv $gen/level_manifest.cooked.rs)" >> $out
done
cat $out
