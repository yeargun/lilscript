#!/bin/sh
set -eu

COMPARISON=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CC=${CC:-clang}
temporary=${TMPDIR:-/tmp}/lilscript-artifact-test-$$
mkdir -p "$temporary"
trap 'rm -rf "$temporary"' EXIT HUP INT TERM

count=0
for artifact in "$COMPARISON"/artifacts/*; do
  [ -d "$artifact" ] || continue
  name=$(basename "$artifact")
  node "$artifact/lilscript.js" > "$temporary/$name-lilscript-js.stdout"
  node "$artifact/closure-advanced.js" > "$temporary/$name-closure.stdout"
  "$CC" -std=c11 -O3 "$artifact/lilscript.c" -o "$temporary/$name-from-c"
  "$temporary/$name-from-c" > "$temporary/$name-c.stdout"

  diff -u "$artifact/expected.stdout" "$temporary/$name-lilscript-js.stdout"
  diff -u "$artifact/expected.stdout" "$temporary/$name-closure.stdout"
  diff -u "$artifact/expected.stdout" "$temporary/$name-c.stdout"

  if [ "$(uname -s)-$(uname -m)" = "Darwin-arm64" ]; then
    "$artifact/lilscript-native-macos-arm64" > "$temporary/$name-native.stdout"
    diff -u "$artifact/expected.stdout" "$temporary/$name-native.stdout"
  fi
  count=$((count + 1))
done

printf '%s checked-in comparison artifact sets passed.\n' "$count"
