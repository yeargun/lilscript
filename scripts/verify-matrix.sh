#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}
CC=${CC:-clang}
LILSCRIPT=${LILSCRIPT:-"$ROOT/target/release/lilscript"}
CASES="$ROOT/tests/cases"
BUILD="$ROOT/target/verification-matrix"

if [ ! -x "$LILSCRIPT" ]; then
  "$CARGO" build --release --bin lilscript
fi
mkdir -p "$BUILD"

count=0
verify_case() {
  source=$1
  expected=$2
  name=$3
  base="$BUILD/$name"

  "$LILSCRIPT" "$source" --target all -o "$base"
  test -s "$base.js"
  test -s "$base.c"
  test -x "$base"

  node "$base.js" > "$base.js.out"
  "$base" > "$base.native.out"
  "$CC" -std=c11 -O3 "$base.c" -o "$base.from-c"
  "$base.from-c" > "$base.c.out"

  diff -u "$expected" "$base.js.out"
  diff -u "$expected" "$base.native.out"
  diff -u "$expected" "$base.c.out"
  count=$((count + 1))
}

for source in "$CASES"/*.lil; do
  name=$(basename "$source" .lil)
  verify_case "$source" "$CASES/$name.out" "$name"
done

verify_case \
  "$ROOT/tests/modules/main.lil" \
  "$ROOT/tests/modules/main.out" \
  module_graph

printf '%s LilScript programs matched across JavaScript, emitted C, and native executables.\n' "$count"
