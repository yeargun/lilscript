#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BUILD="$ROOT/benchmarks/build"
CLOSURE_VERSION=${CLOSURE_VERSION:-v20260803}
CLOSURE_JAR=${CLOSURE_JAR:-"$BUILD/closure-compiler-$CLOSURE_VERSION.jar"}
CARGO=${CARGO:-cargo}

mkdir -p "$BUILD"

if [ ! -f "$CLOSURE_JAR" ]; then
  curl -fL "https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/$CLOSURE_VERSION/closure-compiler-$CLOSURE_VERSION.jar" -o "$CLOSURE_JAR"
fi

"$CARGO" build --release --bin lilscript

run_case() {
  name=$1
  lilscript_source=$2
  js_source=$3
  lilscript_output="$BUILD/$name-lilscript.js"
  closure_output="$BUILD/$name-closure.js"

  "$ROOT/target/release/lilscript" "$lilscript_source" -o "$lilscript_output"
  java -jar "$CLOSURE_JAR" \
    --js "$js_source" \
    --js_output_file "$closure_output" \
    --compilation_level ADVANCED \
    --language_in ECMASCRIPT_2021 \
    --language_out ECMASCRIPT_2021 \
    --warning_level QUIET \
    --emit_use_strict=false

  node "$lilscript_output" > "$BUILD/$name-lilscript.out"
  node "$closure_output" > "$BUILD/$name-closure.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript.out"

  printf '\n%s\n' "$name"
  node "$ROOT/benchmarks/measure.mjs" \
    LilScript "$lilscript_output" \
    "Closure ADVANCED $CLOSURE_VERSION" "$closure_output"
}

run_module_case() {
  name=$1
  lilscript_source=$2
  js_directory=$3
  lilscript_output="$BUILD/$name-lilscript.js"
  closure_output="$BUILD/$name-closure.js"

  "$ROOT/target/release/lilscript" "$lilscript_source" -o "$lilscript_output"
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

  node "$lilscript_output" > "$BUILD/$name-lilscript.out"
  node "$closure_output" > "$BUILD/$name-closure.out"
  diff -u "$BUILD/$name-closure.out" "$BUILD/$name-lilscript.out"

  printf '\n%s\n' "$name"
  node "$ROOT/benchmarks/measure.mjs" \
    LilScript "$lilscript_output" \
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
