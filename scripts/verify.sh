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

# A pinned seed makes this a regression corpus rather than a generator: the same
# 64 programs on every commit, so a shape it never draws is a shape nobody
# checks. Draw a fresh one each run; the binary prints the seed before it starts
# and repeats it on every divergence. Replay a failure by exporting
# LILSCRIPT_DIFFERENTIAL_SEED with the value it printed.
if [ -n "${LILSCRIPT_DIFFERENTIAL_SEED:-}" ]; then
  differential_seed_args="--seed ${LILSCRIPT_DIFFERENTIAL_SEED}"
else
  differential_seed_args="--random-seed"
fi
# shellcheck disable=SC2086 # deliberate word splitting: one flag or two tokens.
CC="$CC" "$ROOT/target/release/lilscript-differential" --cases 64 $differential_seed_args

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
node "$ROOT/scripts/verify-lilpack.mjs" \
  "$ROOT/target/release/lilpack" "$ROOT/target/release/lilscript"
"$ROOT/target/release/lilscript-fmt" "$ROOT/tests/tooling/canonical.lil" --check
"$ROOT/target/release/lilscript-lint" "$ROOT/tests/tooling/canonical.lil" --deny-warnings

printf 'JavaScript/native/reference/tooling conformance passed.\n'
