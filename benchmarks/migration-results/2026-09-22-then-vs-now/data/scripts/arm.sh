#!/bin/bash
# One comparison arm: copy a port to scratch, compile its main artifact, measure.
#   arm.sh <arm-name> <compiler> <route: legacy|semantic> <port>
set -u
SP=/tmp/claude-1000/-home-azureuser-lilscript/3de072ae-233a-45f7-a7d6-9d07057b517c/scratchpad
ARM=$1; COMPILER=$2; ROUTE=$3; PORT=$4
WS=$SP/cmp/ws/$ARM/$PORT
OUT=$SP/cmp/out/$ARM; mkdir -p $OUT
rm -rf $WS; mkdir -p $WS
rsync -a --exclude node_modules --exclude .git --exclude dist --exclude .tmp ~/$PORT/ $WS/
PATCH=/home/azureuser/lilscript/finer/port-migrations/$PORT.patch
PATCHED=no
if [ "$ROUTE" = semantic ] && [ -f "$PATCH" ]; then
  (cd $WS && patch -p1 -s < $PATCH) || { echo "{\"arm\":\"$ARM\",\"port\":\"$PORT\",\"error\":\"patch failed\"}" > $OUT/$PORT.json; exit 1; }
  PATCHED=yes
fi
case $PORT in
  probelil) SRC=src/probe.lil; TARGET=js ;;
  *) SRC=src/entry.lil; TARGET=js-module ;;
esac
EXTRA=()
[ "$ROUTE" = semantic ] && EXTRA=(--backend semantic)
START=$(date +%s.%N)
( cd $WS && timeout 7200 $COMPILER $SRC --target $TARGET --config lilscript.toml -o $OUT/$PORT.js -j ${JOBS:-4} "${EXTRA[@]}" ) > $OUT/$PORT.log 2>&1
STATUS=$?
END=$(date +%s.%N)
SECONDS_TAKEN=$(echo "$END - $START" | bc)
if [ $STATUS -eq 0 ]; then
  SIZES=$($SP/bin/lilscript-codec --json $OUT/$PORT.js)
else
  SIZES=null
fi
printf '{"arm":"%s","port":"%s","route":"%s","patched":"%s","status":%s,"seconds":%s,"compiler_sha256":"%s","sizes":%s}\n' \
  "$ARM" "$PORT" "$ROUTE" "$PATCHED" "$STATUS" "$SECONDS_TAKEN" "$(sha256sum $COMPILER | cut -d' ' -f1)" "$SIZES" > $OUT/$PORT.json
cat $OUT/$PORT.json
