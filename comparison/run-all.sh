#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
for app in "$ROOT"/apps/*; do
  "$app/build.sh"
done
node "$ROOT/lib/summarize.mjs" "$ROOT"
