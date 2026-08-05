#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
VERSION=${1:-v20260803}
EXPECTED_SHA256=${2:-acffbafea43d48064ea1ad64cb4ec95828eac696be0c51a05874178acc19e21a}
TOOLS="$ROOT/.tools"
JAR="$TOOLS/closure-compiler-$VERSION.jar"
URL="https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/$VERSION/closure-compiler-$VERSION.jar"

mkdir -p "$TOOLS"
if [ ! -f "$JAR" ]; then
  DOWNLOAD="$JAR.download"
  /usr/bin/curl -fL "$URL" -o "$DOWNLOAD"
  mv "$DOWNLOAD" "$JAR"
fi

ACTUAL_SHA256=$(shasum -a 256 "$JAR" | awk '{print $1}')
if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
  printf 'Closure checksum mismatch: expected %s, found %s\n' \
    "$EXPECTED_SHA256" "$ACTUAL_SHA256" >&2
  exit 1
fi

printf '%s\n' "$JAR"
