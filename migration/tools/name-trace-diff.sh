#!/bin/bash
# The name-request-order gate (005 §"The determinism trap"): compare two
# compilers' name-request sequences, not their bytes. Each emission prints one
# block under LILSCRIPT_NAME_TRACE=1; blocks are reduced to one line each and
# sorted, because candidate emissions run on threads and interleave.
#
#   migration/tools/name-trace-diff.sh <old-lilscript> <new-lilscript> [case.lil ...]
#
# Lanes: the shipped config and candidate_search = "off" (one emission).
set -u
OLD=$1; NEW=$2; shift 2
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CASES=("$@"); [ ${#CASES[@]} -eq 0 ] && CASES=("$ROOT"/tests/cases/*.lil)
WORK=${NAME_TRACE_WORK:-${TMPDIR:-/tmp/claude-1000}/name-trace-$$}; mkdir -p "$WORK"
OFF="$WORK/search-off.toml"; printf '[javascript]\ncandidate_search = "off"\n' > "$OFF"
blocks() { # stderr -> one sorted line per emission
  awk '/^name-trace emission/{if(line!="")print line; line=""; next} /^name-trace /{line=line" "$3":"$4} END{if(line!="")print line}' | sort
}
fail=0; total=0
for case in "${CASES[@]}"; do
  for lane in shipped off; do
    cfg=(); [ $lane = off ] && cfg=(--config "$OFF")
    total=$((total+1)); name=$(basename "$case" .lil)
    LILSCRIPT_NAME_TRACE=1 "$OLD" "${cfg[@]}" "$case" --target js -o "$WORK/o.js" 2>&1 >/dev/null | blocks > "$WORK/$name.$lane.old"
    LILSCRIPT_NAME_TRACE=1 "$NEW" "${cfg[@]}" "$case" --target js -o "$WORK/n.js" 2>&1 >/dev/null | blocks > "$WORK/$name.$lane.new"
    if ! cmp -s "$WORK/$name.$lane.old" "$WORK/$name.$lane.new"; then
      fail=$((fail+1)); echo "TRACE DIFFERS $name [$lane] (bytes $(cmp -s "$WORK/o.js" "$WORK/n.js" && echo same || echo differ))"
      diff "$WORK/$name.$lane.old" "$WORK/$name.$lane.new" | head -4 | cut -c1-160
    fi
  done
done
echo "name-trace: $fail of $total case-lanes differ (work: $WORK)"
[ $fail -eq 0 ]
