#!/bin/zsh
# Cycle-independent sim-state gate for the grid playtest.
#
# Frame comparisons between two builds are phase dependent (the first display
# flip after a controller poll shows whichever render was in flight), so they
# cannot prove two builds simulate the same world. This gate compares the
# simulation itself: the guest built with the `sim-trace` feature prints one
# `sim-state` line per fixed tick (player position, yaw, vertical speed,
# grounded, room, camera eye and angles, an FNV hash over every entity slot's
# position and state, plus the first three entity positions). Both builds replay
# the same poll-bound tape, and the lines are compared tick for tick.
#
#   simgate.sh disc LABEL PROJECT          build a sim-trace disc (editor/projects/PROJECT)
#   simgate.sh run  LABEL TAPE STOP_POLL   replay it headlessly, keep the sim-state lines
#   simgate.sh cmp  LABEL_A LABEL_B        compare two runs tick for tick
#
# Work dir: $SIMGATE_DIR (default build/simgate). The frontend is $FRONTEND
# (default target/release/frontend); the emulator core does not differ between
# builds being compared, so one frontend can replay both builds' discs, but a
# disc must be cooked by the frontend of the tree it describes (run `disc` from
# the tree under test, with that tree's frontend built).
set -e
root=${SIMGATE_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}
work=${SIMGATE_DIR:-$root/build/simgate}
frontend=${FRONTEND:-$root/target/release/frontend}
features=${SIMGATE_FEATURES:-cd-stream-bench sim-trace}
cmd=$1; shift

case $cmd in
disc)
  label=$1; proj=$2
  out=$work/discs/$label; mkdir -p $out
  cd $root/emu
  env EDITOR_PLAYTEST_FEATURES="$features" PSOXIDE_GUEST_STAGE_ROOT=$work/stage \
    $frontend build-project-disc --project ../editor/projects/$proj/project.ron >| $out/build.log 2>&1
  cp $root/editor/projects/$proj/baked/*.cue $root/editor/projects/$proj/baked/*.bin $out/
  echo "$label disc built"
  ;;
run)
  label=$1; tape=$2; stop=$3
  d=$work/runs/$label; mkdir -p $d
  $frontend launch --path $work/discs/$label/*.cue --embedded-playtest --input-tape $tape \
    --stop-at-poll $stop --steps 120000000000 --guest-debug-log >| $d/run.log 2>&1
  sed -n 's/^\[guest [^]]*\] //p' $d/run.log | grep -a '^sim-state,' >| $d/sim.txt
  echo "$label: $(wc -l < $d/sim.txt | tr -d ' ') sim ticks"
  ;;
cmp)
  a=$work/runs/$1/sim.txt; b=$work/runs/$2/sim.txt
  awk -F, -v A="$1" -v B="$2" '
    BEGIN {
      split("tick px py pz yaw vy grounded room eyex eyey eyez cyaw cpitch nent ehash e0x e0y e0z e1x e1y e1z e2x e2y e2z", N, " ")
    }
    { sub(/^sim-state,/, "") }
    FNR == NR { ra[$1] = $0; ka[++na] = $1; next }
    { rb[$1] = $0; kb[++nb] = $1 }
    END {
      common = 0; same = 0; first = ""; nd = 0
      for (i = 1; i <= nb; i++) {
        t = kb[i]
        if (!(t in ra)) continue
        common++
        if (ra[t] == rb[t]) { same++; continue }
        nd++
        if (first == "") first = t
        n = split(ra[t], x, ","); split(rb[t], y, ",")
        for (f = 1; f <= 24; f++) if (x[f] != y[f]) fd[f]++
      }
      printf "%s: %d ticks (tick %s..%s)\n", A, na, ka[1], ka[na]
      printf "%s: %d ticks (tick %s..%s)\n", B, nb, kb[1], kb[nb]
      printf "common ticks %d, identical %d, different %d\n", common, same, nd
      if (nd == 0 && common > 0) { print "RESULT: MATCH (sim state identical on every common tick)"; exit 0 }
      if (common == 0) { print "RESULT: NO COMMON TICKS"; exit 2 }
      printf "RESULT: DIVERGES, first differing tick %s\n", first
      printf "  A %s\n  B %s\n", ra[first], rb[first]
      for (f = 1; f <= 24; f++) if (fd[f]) printf "  field %s differs on %d ticks\n", N[f], fd[f]
      exit 1
    }' $a $b
  ;;
*) echo "usage: simgate.sh disc|run|cmp ..." >&2; exit 64 ;;
esac
