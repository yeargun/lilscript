#!/bin/bash
# The port twin lane: compile a port's entry with the candidate search off and
# LILSCRIPT_TWIN=1, on a scratch copy, so the identity re-spell and condition
# witnesses run on real code (the 74 cases never had a `this` parameter with a
# binding; remarklil did). Then compile it under the frequency ordering.
#
#   migration/tools/port-twin.sh <lilscript> <port-dir> <entry.lil> [target]
set -u
L=$(readlink -f "$1"); PORT=$2; ENTRY=$3; TARGET=${4:-js-module}
WORK=${TMPDIR:-/tmp/claude-1000}/port-twin-$(basename "$PORT")
rm -rf "$WORK"; mkdir -p "$WORK"
rsync -a --exclude node_modules --exclude dist --exclude .git "$PORT/" "$WORK/src-copy/"
python3 - "$WORK/src-copy/lilscript.toml" "$WORK/off.toml" <<'PY'
import re,sys
p=open(sys.argv[1]).read()
if re.search(r'^candidate_search',p,re.M): p=re.sub(r'^candidate_search\s*=.*$','candidate_search = "off"',p,count=1,flags=re.M)
else: p=re.sub(r'^\[javascript\]$','[javascript]\ncandidate_search = "off"',p,count=1,flags=re.M)
open(sys.argv[2],'w').write(p)
PY
cd "$WORK/src-copy"
printf "%-16s twin (search off): " "$(basename "$PORT")"
if LILSCRIPT_TWIN=1 "$L" --config "$WORK/off.toml" "$ENTRY" --target "$TARGET" -o "$WORK/twin.js" >"$WORK/twin.err" 2>&1; then echo ok; else echo "FAIL: $(grep -m1 'panicked\|error' "$WORK/twin.err" | cut -c1-160)"; fi
printf "%-16s frequency-desc (real config): " "$(basename "$PORT")"
if LILSCRIPT_NAME_ORDERING=frequency-desc "$L" --config lilscript.toml "$ENTRY" --target "$TARGET" -o "$WORK/freq.js" >"$WORK/freq.err" 2>&1; then echo "ok ($(wc -c < "$WORK/freq.js") bytes)"; else echo "FAIL: $(grep -m1 'error' "$WORK/freq.err" | cut -c1-160)"; fi
