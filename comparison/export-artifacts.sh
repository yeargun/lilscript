#!/bin/sh
set -eu

COMPARISON=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ARTIFACTS="$COMPARISON/artifacts"

mkdir -p "$ARTIFACTS"
for app in "$COMPARISON"/apps/*; do
  [ -d "$app" ] || continue
  name=$(basename "$app")
  build="$app/build"
  destination="$ARTIFACTS/$name"

  # Snapshot export is an attested rebuild operation, never a copy of whatever
  # complete-looking files happen to be left in build/.
  "$app/build.sh"

  mkdir -p "$destination"
  install -m 0644 "$build/lilscript.js" "$destination/lilscript.js"
  install -m 0644 "$build/lilscript-raw.js" "$destination/lilscript-raw.js"
  install -m 0644 "$build/lilscript-gzip.js" "$destination/lilscript-gzip.js"
  install -m 0644 "$build/lilscript-brotli.js" "$destination/lilscript-brotli.js"
  install -m 0644 "$build/lilscript.c" "$destination/lilscript.c"
  install -m 0644 "$build/closure.js" "$destination/closure-advanced.js"
  install -m 0644 "$build/report.json" "$destination/report.json"
  install -m 0644 "$build/report.md" "$destination/report.md"
  install -m 0644 "$build/toolchain.txt" "$destination/toolchain.txt"
  install -m 0644 "$app/tests/stdout.txt" "$destination/expected.stdout"
  if [ "$(uname -s)-$(uname -m)" = "Darwin-arm64" ]; then
    install -m 0755 "$build/lilscript" "$destination/lilscript-native-macos-arm64"
  fi
done

node "$COMPARISON/lib/check-size-gate.mjs" "$COMPARISON"

printf 'Exported checked-in comparison artifacts to %s\n' "$ARTIFACTS"
