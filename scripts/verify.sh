#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}
CC=${CC:-clang}
BUILD="$ROOT/target/verification"

mkdir -p "$BUILD"
"$CARGO" build --release --bins

verify_program() {
  name=$1
  source="$ROOT/examples/$name.lil"
  js="$BUILD/$name.js"
  native="$BUILD/$name-native"

  "$ROOT/target/release/lilscript" "$source" -o "$js"
  "$ROOT/target/release/lilscript" "$source" --target native -o "$native"
  node "$js" > "$BUILD/$name-js.out"
  "$native" > "$BUILD/$name-native.out"
  diff -u "$BUILD/$name-js.out" "$BUILD/$name-native.out"
}

verify_program conformance
verify_program full_conformance

LILSCRIPT="$ROOT/target/release/lilscript" \
  CARGO="$CARGO" \
  CC="$CC" \
  "$ROOT/scripts/verify-matrix.sh"

"$ROOT/target/release/lilscript" "$ROOT/examples/extern_abi.lil" \
  --target c -o "$BUILD/extern_abi.c"
"$CC" -std=c11 -O3 "$ROOT/tests/extern_abi_host.c" -o "$BUILD/extern_abi-host"
extern_result=$("$BUILD/extern_abi-host")
if [ "$extern_result" != "6" ]; then
  printf 'Native extern ABI returned %s instead of 6.\n' "$extern_result" >&2
  exit 1
fi

node "$ROOT/scripts/test-lsp.mjs" "$ROOT/target/release/lilscript-lsp"
node "$ROOT/scripts/verify-bundles.mjs" "$ROOT/target/release/lilscript"

printf 'JavaScript/native/tooling conformance passed.\n'
