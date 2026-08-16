#!/bin/sh
set -eu

APP=${1:?application directory is required}
APP=$(CDPATH= cd -- "$APP" && pwd)
COMPARISON=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$COMPARISON/.." && pwd)
CARGO=${CARGO:-cargo}
CC=${CC:-clang}

. "$APP/versions.env"

LILSCRIPT_OVERRIDE_SET=${LILSCRIPT+x}
LILSCRIPT_CODEC_OVERRIDE_SET=${LILSCRIPT_CODEC+x}
BUILD="$APP/build"
EXPECTED="$APP/tests/stdout.txt"

if { [ -n "$LILSCRIPT_OVERRIDE_SET" ] && [ -z "$LILSCRIPT_CODEC_OVERRIDE_SET" ]; } \
  || { [ -z "$LILSCRIPT_OVERRIDE_SET" ] && [ -n "$LILSCRIPT_CODEC_OVERRIDE_SET" ]; }; then
  echo "LILSCRIPT and LILSCRIPT_CODEC overrides must be supplied together" >&2
  exit 1
fi

if [ -z "$LILSCRIPT_OVERRIDE_SET" ]; then
  LILSCRIPT="$REPO/target/release/lilscript"
  LILSCRIPT_CODEC="$REPO/target/release/lilscript-codec"
  # A standalone evidence build must attest binaries produced from the current
  # checkout, not silently reuse whatever executable happens to be present in
  # target/release. Explicit paired overrides are the only supported reuse
  # path and are validated below.
  unset CARGO_BUILD_TARGET
  CARGO_TARGET_DIR="$REPO/target"
  export CARGO_TARGET_DIR
  "$CARGO" build --manifest-path "$REPO/Cargo.toml" --release \
    --bin lilscript --bin lilscript-codec
else
  if [ -z "$LILSCRIPT" ] || [ -z "$LILSCRIPT_CODEC" ]; then
    echo "LILSCRIPT and LILSCRIPT_CODEC overrides must both be non-empty" >&2
    exit 1
  fi
  if [ ! -x "$LILSCRIPT" ]; then
    echo "LILSCRIPT override is not executable: $LILSCRIPT" >&2
    exit 1
  fi
  if [ ! -x "$LILSCRIPT_CODEC" ]; then
    echo "LILSCRIPT_CODEC override is not executable: $LILSCRIPT_CODEC" >&2
    exit 1
  fi
fi
JAR=$(
  "$COMPARISON/install-closure.sh" "$CLOSURE_VERSION" "$CLOSURE_SHA256"
)
export LILSCRIPT_CODEC
mkdir -p "$BUILD"

"$LILSCRIPT" "$APP/lilscript/main.lil" --target all -o "$BUILD/lilscript"
for objective in raw gzip brotli; do
  "$LILSCRIPT" "$APP/lilscript/main.lil" \
    --config "$COMPARISON/cases/configs/$objective.toml" \
    --target js \
    --mode production \
    -o "$BUILD/lilscript-$objective.js"
done
set --
for javascript in "$APP"/closure/*.js; do
  set -- "$@" --js "$javascript"
done
java -jar "$JAR" \
  "$@" \
  --js_output_file "$BUILD/closure.js" \
  --compilation_level ADVANCED \
  --language_in ECMASCRIPT_2021 \
  --language_out ECMASCRIPT_2021 \
  --warning_level VERBOSE \
  --emit_use_strict=false

node "$BUILD/lilscript.js" > "$BUILD/lilscript-js.stdout"
node "$BUILD/lilscript-raw.js" > "$BUILD/lilscript-raw.stdout"
node "$BUILD/lilscript-gzip.js" > "$BUILD/lilscript-gzip.stdout"
node "$BUILD/lilscript-brotli.js" > "$BUILD/lilscript-brotli.stdout"
node "$BUILD/closure.js" > "$BUILD/closure.stdout"
"$BUILD/lilscript" > "$BUILD/lilscript-native.stdout"
"$CC" -std=c11 -O3 "$BUILD/lilscript.c" -o "$BUILD/lilscript-from-c"
"$BUILD/lilscript-from-c" > "$BUILD/lilscript-c.stdout"

diff -u "$EXPECTED" "$BUILD/lilscript-js.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-raw.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-gzip.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-brotli.stdout"
diff -u "$EXPECTED" "$BUILD/closure.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-native.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-c.stdout"

java -jar "$JAR" --version > "$BUILD/closure-version.txt"
printf 'LilScript %s\nClosure %s\nClosure SHA-256 %s\n' \
  "$LILSCRIPT_VERSION" "$CLOSURE_VERSION" "$CLOSURE_SHA256" \
  > "$BUILD/toolchain.txt"

node "$COMPARISON/lib/measure.mjs" \
  "$APP" \
  "$BUILD/lilscript-raw.js" \
  "$BUILD/lilscript-gzip.js" \
  "$BUILD/lilscript-brotli.js" \
  "$BUILD/closure.js" \
  "$LILSCRIPT_VERSION" "$CLOSURE_VERSION" \
  "$LILSCRIPT" \
  "$JAR" "$CLOSURE_SHA256" \
  "$COMPARISON/cases/configs/raw.toml" \
  "$COMPARISON/cases/configs/gzip.toml" \
  "$COMPARISON/cases/configs/brotli.toml"

printf '%s: build, runtime equivalence, and size comparison passed.\n' \
  "$(basename "$APP")"
