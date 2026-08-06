#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}

cd "$ROOT"
"$CARGO" fmt --all -- --check
"$CARGO" clippy --all-targets -- -D warnings
"$CARGO" test --all-targets
"$ROOT/scripts/verify.sh"
"$ROOT/benchmarks/run.sh"
node "$ROOT/benchmarks/finite-values/run.mjs"
node "$ROOT/benchmarks/ir-variants/run.mjs"
node "$ROOT/benchmarks/paired/run.mjs" --check
"$ROOT/comparison/run-all.sh"
npm --prefix "$ROOT/benchmarks/apps" run verify
npm --prefix "$ROOT/benchmarks/libraries" run verify
npm --prefix "$ROOT/benchmarks/browser" run verify
npm --prefix "$ROOT/web" test
npm --prefix "$ROOT/web" run build
npm --prefix "$ROOT/vscode-extension" run package

printf 'LilScript release checks passed.\n'
