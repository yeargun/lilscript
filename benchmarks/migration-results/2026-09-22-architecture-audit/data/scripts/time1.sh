#!/bin/bash
# time1.sh <bin> <port> <route> <tag> [extra compiler args...]: one timed build
# prints: port route tag wall user sys maxrssKB raw gzip brotli
SP=/tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad
BIN=$1; PORT=$2; ROUTE=$3; TAG=$4; shift 4
WS=$SP/ws/$PORT-$ROUTE
case $PORT in probelil) SRC=src/probe.lil; TARGET=js ;; *) SRC=src/entry.lil; TARGET=js-module ;; esac
CFG=${CFG:-lilscript.toml}
mkdir -p $SP/out
OUT=$SP/out/$PORT-$ROUTE-$TAG.js
EXTRA=(--backend $ROUTE)
T=$( { cd $WS && /usr/bin/time -f "%e %U %S %M" $BIN $SRC --target $TARGET --config $CFG -o $OUT -j 4 "${EXTRA[@]}" "$@" > /dev/null 2> $OUT.log; } 2>&1 )
echo "$PORT $ROUTE $TAG $(tail -1 $OUT.log) $($SP/brotli.sh $OUT 2>/dev/null)"
