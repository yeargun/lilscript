#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
VERSION=${1:-v20260804}
EXPECTED_SHA256=${2:-9cad14d0337b2aaaf9ba8b24446cc45dc737bc34fe591e00835eff700a795d5b}
TOOLS="$ROOT/.tools"
JAR="$TOOLS/closure-compiler-$VERSION.jar"
URL="https://repo.maven.apache.org/maven2/com/google/javascript/closure-compiler/$VERSION/closure-compiler-$VERSION.jar"

mkdir -p "$TOOLS"
if [ ! -f "$JAR" ]; then
  NPM_JAR="$ROOT/../benchmarks/popular/node_modules/google-closure-compiler-java/compiler.jar"
  if [ -f "$NPM_JAR" ]; then
    cp "$NPM_JAR" "$JAR"
  else
    DOWNLOAD="$JAR.download"
    /usr/bin/curl -fL "$URL" -o "$DOWNLOAD"
    mv "$DOWNLOAD" "$JAR"
  fi
fi

ACTUAL_SHA256=$(shasum -a 256 "$JAR" | awk '{print $1}')
if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
  printf 'Closure checksum mismatch: expected %s, found %s\n' \
    "$EXPECTED_SHA256" "$ACTUAL_SHA256" >&2
  exit 1
fi

printf '%s\n' "$JAR"
