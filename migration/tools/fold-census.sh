#!/bin/bash
# Phase 6 gate: which folds still rewrite, per lane. A fold is deletable only
# when its active count (calls - idle, from LILSCRIPT_FOLD_REPORT) is 0 on
# every lane and every port. Prints one line per (lane, fold) with active > 0.
#
#   migration/tools/fold-census.sh <lilscript> [file.lil ...]   (default: the probe + tests/cases)
set -u
L=$(readlink -f "$1"); shift
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
FILES=("$@"); [ ${#FILES[@]} -eq 0 ] && FILES=("$HOME/probelil/src/probe.lil" "$ROOT"/tests/cases/*.lil)
declare -A LANES=( [shipped]="--config $ROOT/lilscript.toml" [none]="--config $ROOT/tests/config/no-optimization.toml" [zodlike]="--config /tmp/claude-1000/zodlike.toml" )
WORK=${TMPDIR:-/tmp/claude-1000}/fold-census-$$; mkdir -p "$WORK"
for lane in shipped none zodlike; do
  : > "$WORK/$lane"
  for f in "${FILES[@]}"; do
    cfg=${LANES[$lane]}; [[ "$f" == *probelil* ]] && [ $lane = shipped ] && cfg="--config $HOME/probelil/lilscript.toml"
    LILSCRIPT_FOLD_REPORT=all LILSCRIPT_TIMING=1 "$L" $cfg "$f" --target js -o "$WORK/out.js" 2>&1 | grep "idle/calls" | awk -v file="$(basename "$f")" '{split($3,a,"/"); if (a[2]-a[1]>0) printf "%s %d %s\n", $NF, a[2]-a[1], file}' >> "$WORK/$lane"
  done
  echo "== $lane: active folds (fold, files firing, total activations)"
  awk '{n[$1]++; t[$1]+=$2} END{for (k in n) printf "%-52s %4d files %6d activations\n", k, n[k], t[k]}' "$WORK/$lane" | sort -k3 -nr
done
echo "(work: $WORK)"
