#!/bin/sh
set -eu

APP=${1:?application directory is required}
APP=$(CDPATH= cd -- "$APP" && pwd)
COMPARISON=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$COMPARISON/.." && pwd)
CARGO=${CARGO:-cargo}
CC=${CC:-clang}

. "$APP/versions.env"

JAR=$(
  "$COMPARISON/install-closure.sh" "$CLOSURE_VERSION" "$CLOSURE_SHA256"
)
LILSCRIPT=${LILSCRIPT:-"$REPO/target/release/lilscript"}
BUILD="$APP/build"
EXPECTED="$APP/tests/stdout.txt"

if [ ! -x "$LILSCRIPT" ]; then
  "$CARGO" build --manifest-path "$REPO/Cargo.toml" --release --bin lilscript
fi
mkdir -p "$BUILD"

"$LILSCRIPT" "$APP/lilscript/main.lil" --target all -o "$BUILD/lilscript"
java -jar "$JAR" \
  --js "$APP/closure/main.js" \
  --js_output_file "$BUILD/closure.js" \
  --compilation_level ADVANCED \
  --language_in ECMASCRIPT_2021 \
  --language_out ECMASCRIPT_2021 \
  --warning_level VERBOSE \
  --emit_use_strict=false

node "$BUILD/lilscript.js" > "$BUILD/lilscript-js.stdout"
node "$BUILD/closure.js" > "$BUILD/closure.stdout"
"$BUILD/lilscript" > "$BUILD/lilscript-native.stdout"
"$CC" -std=c11 -O3 "$BUILD/lilscript.c" -o "$BUILD/lilscript-from-c"
"$BUILD/lilscript-from-c" > "$BUILD/lilscript-c.stdout"

diff -u "$EXPECTED" "$BUILD/lilscript-js.stdout"
diff -u "$EXPECTED" "$BUILD/closure.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-native.stdout"
diff -u "$EXPECTED" "$BUILD/lilscript-c.stdout"

java -jar "$JAR" --version > "$BUILD/closure-version.txt"
printf 'LilScript %s\nClosure %s\nClosure SHA-256 %s\n' \
  "$LILSCRIPT_VERSION" "$CLOSURE_VERSION" "$CLOSURE_SHA256" \
  > "$BUILD/toolchain.txt"

node "$COMPARISON/lib/measure.mjs" \
  "$APP" "$BUILD/lilscript.js" "$BUILD/closure.js" \
  "$LILSCRIPT_VERSION" "$CLOSURE_VERSION"

printf '%s: build, runtime equivalence, and size comparison passed.\n' \
  "$(basename "$APP")"
