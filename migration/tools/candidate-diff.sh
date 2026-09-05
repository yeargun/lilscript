#!/usr/bin/env bash
# Compare the *candidate sets* two compiler binaries produce, with the search
# forced and the control-flow variant families enabled, so renderings that
# never win (the state machine, the switch lowering) are still checked byte
# for byte. Usage: candidate-diff.sh <old-binary> <new-binary> [case ...]
set -u
old=$1; new=$2; shift 2
root=$(cd "$(dirname "$0")/../.." && pwd)
cases=${*:-"04_while 05_for_control 08_recursion 33_algorithms"}
work=${CANDIDATE_DIFF_DIR:-/tmp/claude-1000/candidate-diff}
python3 - "$root/lilscript.toml" "$work/always-sm.toml" <<'PY'
import re,sys,os
base=open(sys.argv[1]).read()
def setk(t,k,v):
    return re.sub(r'^%s\s*=.*$'%k, f'{k} = {v}', t, flags=re.M) if re.search(r'^%s\s*='%k,t,re.M) else t.replace('[javascript]', f'[javascript]\n{k} = {v}',1)
t=setk(base,'candidate_search','"always"'); t=setk(t,'candidate_beam_width','32')
t=setk(t,'optimizations','["structural-control-flow-variants", "switch-lowering-variants", "parsed-peephole"]')
os.makedirs(os.path.dirname(sys.argv[2]),exist_ok=True); open(sys.argv[2],'w').write(t)
PY
rm -rf "$work/old" "$work/new"
for c in $cases; do
  LILSCRIPT_DUMP_CANDIDATES="$work/old/$c" LILSCRIPT_DUMP_REJECTED="$work/old/$c-rej" "$old" --config "$work/always-sm.toml" "$root/tests/cases/$c.lil" --target js -o "$work/o.js" >/dev/null 2>&1
  LILSCRIPT_DUMP_CANDIDATES="$work/new/$c" LILSCRIPT_DUMP_REJECTED="$work/new/$c-rej" "$new" --config "$work/always-sm.toml" "$root/tests/cases/$c.lil" --target js -o "$work/n.js" >/dev/null 2>&1
done
o=$(find "$work/old" -type f | wc -l); n=$(find "$work/new" -type f | wc -l)
so=$(grep -rl 'for(;;)switch\|for(;;){if(' "$work/old" | wc -l); sn=$(grep -rl 'for(;;)switch\|for(;;){if(' "$work/new" | wc -l)
echo "candidates old $o new $n; state-machine renders old $so new $sn"
if diff <(find "$work/old" -type f -exec cat {} + | sort -u) <(find "$work/new" -type f -exec cat {} + | sort -u) > "$work/diff.txt"; then
  echo "candidate sets identical"
else
  echo "CANDIDATE SETS DIFFER: $(grep -c '^[<>]' "$work/diff.txt") lines (see $work/diff.txt)"; exit 1
fi
