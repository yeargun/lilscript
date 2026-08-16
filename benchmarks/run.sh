#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BUILD="$ROOT/benchmarks/build"
PINNED_CLOSURE_VERSION=v20260804
PINNED_CLOSURE_SHA256=9cad14d0337b2aaaf9ba8b24446cc45dc737bc34fe591e00835eff700a795d5b
CARGO=${CARGO:-cargo}

if [ "${CLOSURE_VERSION:-$PINNED_CLOSURE_VERSION}" != "$PINNED_CLOSURE_VERSION" ]; then
  printf 'Closure version override must remain pinned at %s; found %s\n' \
    "$PINNED_CLOSURE_VERSION" "$CLOSURE_VERSION" >&2
  exit 1
fi
if [ "${CLOSURE_SHA256:-$PINNED_CLOSURE_SHA256}" != "$PINNED_CLOSURE_SHA256" ]; then
  printf 'Closure digest override must remain pinned at %s; found %s\n' \
    "$PINNED_CLOSURE_SHA256" "$CLOSURE_SHA256" >&2
  exit 1
fi
CLOSURE_VERSION=$PINNED_CLOSURE_VERSION
CLOSURE_SHA256=$PINNED_CLOSURE_SHA256

if [ "${CLOSURE_JAR+x}" = x ]; then
  if [ ! -f "$CLOSURE_JAR" ]; then
    printf 'Closure jar override does not exist: %s\n' "$CLOSURE_JAR" >&2
    exit 1
  fi
  ACTUAL_CLOSURE_SHA256=$(shasum -a 256 "$CLOSURE_JAR" | awk '{print $1}')
  if [ "$ACTUAL_CLOSURE_SHA256" != "$CLOSURE_SHA256" ]; then
    printf 'Closure checksum mismatch for %s: expected %s, found %s\n' \
      "$CLOSURE_JAR" "$CLOSURE_SHA256" "$ACTUAL_CLOSURE_SHA256" >&2
    exit 1
  fi
else
  CLOSURE_JAR=$(
    "$ROOT/comparison/install-closure.sh" "$CLOSURE_VERSION" "$CLOSURE_SHA256"
  )
fi

mkdir -p "$BUILD"

"$CARGO" build --release --bin lilscript --bin lilscript-codec

run_case() {
  name=$1
  lilscript_source=$2
  js_source=$3
  lilscript_raw_output="$BUILD/$name-lilscript-raw.js"
  lilscript_gzip_output="$BUILD/$name-lilscript-gzip.js"
  lilscript_brotli_output="$BUILD/$name-lilscript-brotli.js"
  closure_output="$BUILD/$name-closure.js"

  "$ROOT/target/release/lilscript" "$lilscript_source" --target js --mode production \
    --config "$ROOT/comparison/cases/configs/raw.toml" -o "$lilscript_raw_output"
  "$ROOT/target/release/lilscript" "$lilscript_source" --target js --mode production \
    --config "$ROOT/comparison/cases/configs/gzip.toml" -o "$lilscript_gzip_output"
  "$ROOT/target/release/lilscript" "$lilscript_source" --target js --mode production \
    --config "$ROOT/comparison/cases/configs/brotli.toml" -o "$lilscript_brotli_output"
  java -jar "$CLOSURE_JAR" \
    --js "$js_source" \
    --js_output_file "$closure_output" \
    --compilation_level ADVANCED \
    --language_in ECMASCRIPT_2021 \
    --language_out ECMASCRIPT_2021 \
    --warning_level QUIET \
    --emit_use_strict=false

  node "$lilscript_raw_output" > "$BUILD/$name-lilscript-raw.out"
  node "$lilscript_gzip_output" > "$BUILD/$name-lilscript-gzip.out"
  node "$lilscript_brotli_output" > "$BUILD/$name-lilscript-brotli.out"
  node "$closure_output" > "$BUILD/$name-closure.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript-raw.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript-gzip.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript-brotli.out"

  printf '\n%s\n' "$name"
  node "$ROOT/benchmarks/measure.mjs" \
    --objective LilScript \
    "$lilscript_raw_output" "$lilscript_gzip_output" "$lilscript_brotli_output" \
    "Closure ADVANCED $CLOSURE_VERSION" "$closure_output"
}

run_module_case() {
  name=$1
  lilscript_source=$2
  js_directory=$3
  lilscript_raw_output="$BUILD/$name-lilscript-raw.js"
  lilscript_gzip_output="$BUILD/$name-lilscript-gzip.js"
  lilscript_brotli_output="$BUILD/$name-lilscript-brotli.js"
  closure_output="$BUILD/$name-closure.js"

  "$ROOT/target/release/lilscript" "$lilscript_source" --target js --mode production \
    --config "$ROOT/comparison/cases/configs/raw.toml" -o "$lilscript_raw_output"
  "$ROOT/target/release/lilscript" "$lilscript_source" --target js --mode production \
    --config "$ROOT/comparison/cases/configs/gzip.toml" -o "$lilscript_gzip_output"
  "$ROOT/target/release/lilscript" "$lilscript_source" --target js --mode production \
    --config "$ROOT/comparison/cases/configs/brotli.toml" -o "$lilscript_brotli_output"
  java -jar "$CLOSURE_JAR" \
    --js "$js_directory/math.js" \
    --js "$js_directory/stats.js" \
    --js "$js_directory/main.js" \
    --js_output_file "$closure_output" \
    --compilation_level ADVANCED \
    --language_in ECMASCRIPT_2021 \
    --language_out ECMASCRIPT_2021 \
    --warning_level QUIET \
    --emit_use_strict=false

  node "$lilscript_raw_output" > "$BUILD/$name-lilscript-raw.out"
  node "$lilscript_gzip_output" > "$BUILD/$name-lilscript-gzip.out"
  node "$lilscript_brotli_output" > "$BUILD/$name-lilscript-brotli.out"
  node "$closure_output" > "$BUILD/$name-closure.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript-raw.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript-gzip.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript-brotli.out"

  printf '\n%s\n' "$name"
  node "$ROOT/benchmarks/measure.mjs" \
    --objective LilScript \
    "$lilscript_raw_output" "$lilscript_gzip_output" "$lilscript_brotli_output" \
    "Closure ADVANCED $CLOSURE_VERSION" "$closure_output"
}

run_case v01 "$ROOT/examples/v01.lil" "$ROOT/benchmarks/v01.js"
run_case conformance "$ROOT/examples/conformance.lil" "$ROOT/benchmarks/conformance.js"
run_case full_conformance "$ROOT/examples/full_conformance.lil" "$ROOT/benchmarks/full_conformance.js"
run_case optimizer_stress "$ROOT/examples/optimizer_stress.lil" "$ROOT/benchmarks/optimizer_stress.js"
run_case algorithms "$ROOT/examples/algorithms.lil" "$ROOT/benchmarks/algorithms.js"
run_case data_model "$ROOT/examples/data_model.lil" "$ROOT/benchmarks/data_model.js"
run_case higher_order "$ROOT/examples/higher_order.lil" "$ROOT/benchmarks/higher_order.js"
run_case string_optimization "$ROOT/examples/string_optimization.lil" "$ROOT/benchmarks/string_optimization.js"
run_case alias_optimization "$ROOT/examples/alias_optimization.lil" "$ROOT/benchmarks/alias_optimization.js"
run_module_case modules "$ROOT/benchmarks/modules/lilscript/main.lil" "$ROOT/benchmarks/modules/closure"
