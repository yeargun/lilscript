#!/usr/bin/env bash
# Level-sweep harness.
#
# Wall clock on this host is unusable: unrelated processes hold 1-2 cores at all
# times, and back-to-back runs of identical work have varied by 3x. CPU time
# (user+sys, summed across workers) is the work the compiler actually did and is
# far less sensitive to that contention, so it is the reported cost. Byte
# counts come from `lilscript-codec`, the canonical pinned encoders.
#
# usage: bench.sh <port-dir> <entry.lil> <base-config> <level>...
set -euo pipefail
root=/home/azureuser/lilscript
out=${BENCH_OUT:-/tmp/claude-1000/-home-azureuser-lilscript/7bb5c8d5-a852-4426-863b-016079ae3ac7/scratchpad/bench}
mkdir -p "$out"
port=$1; entry=$2; config=$3; shift 3
name=$(basename "$port")
printf '%-8s %-6s %8s %8s %9s %9s %9s %6s %6s %8s\n' \
  port level cpu_s rss_mb raw gzip9 brotli11 codecs emits foldMB
for level in "$@"; do
  probe="$port/_afl-$level.toml"
  sed "s/^optimization_level = .*/optimization_level = $level/" "$config" > "$probe"
  artifact="$out/$name-L$level.js"
  stats=$( { /usr/bin/time -f "%U %S %M" env LILSCRIPT_TIMING=1 \
      "$root/target/release/lilscript" "$entry" --config "$probe" \
      --target js-module -o "$artifact" ; } 2>&1 )
  timing=$(printf '%s\n' "$stats" | grep '^lilscript-timing' | sed 's/^lilscript-timing //')
  usage=$(printf '%s\n' "$stats" | grep -E '^[0-9.]+ [0-9.]+ [0-9]+$' | tail -1)
  printf '%s\n' "$timing" > "$out/$name-L$level.timing.json"
  sizes=$("$root/target/release/lilscript-codec" --json "$artifact")
  python3 - "$name" "$level" "$usage" <<PY
import json,sys
name,level,user,sysd,rss = sys.argv[1],sys.argv[2],*sys.argv[3].split()
sizes=json.loads('''$sizes''')["artifacts"][0]
timing=json.loads(open("$out/$name-L$level.timing.json").read())
print(f'{name:<8} {level:<6} {float(user)+float(sysd):8.1f} {int(rss)/1024:8.1f} '
      f'{sizes["raw"]:9} {sizes["gzip9"]:9} {sizes["brotli11"]:9} '
      f'{timing["codec_calls"]:6} {timing["emit_calls"]:6} '
      f'{timing["idle_fold_mb"]+timing["active_fold_mb"]:8.1f}')
PY
  rm -f "$probe"
done
