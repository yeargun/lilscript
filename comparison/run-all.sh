#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CARGO=${CARGO:-cargo}
for app in "$ROOT"/apps/*; do
  "$app/build.sh"
done
node "$ROOT/lib/summarize.mjs" "$ROOT"
node "$ROOT/lib/check-size-gate.mjs" "$ROOT"
CARGO="$CARGO" node "$ROOT/cases/run.mjs"
CARGO="$CARGO" node "$ROOT/algorithms/run.mjs"
node "$ROOT/lib/check-provenance.mjs" "$ROOT"
