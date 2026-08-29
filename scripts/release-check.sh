#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}
LILSCRIPT="$ROOT/target/release/lilscript"
LILSCRIPT_CODEC="$ROOT/target/release/lilscript-codec"
export CARGO LILSCRIPT LILSCRIPT_CODEC

cd "$ROOT"
node -e '
const [major, minor] = process.versions.node.split(".").map(Number);
const supported = (major === 20 && minor >= 19)
  || (major === 22 && minor >= 12)
  || major > 22;
if (!supported) {
  console.error(`LilScript release checks require Node 20.19+ or 22.12+; found ${process.versions.node}. Run nvm use.`);
  process.exit(1);
}
'
LILSCRIPT_STATISTICAL_SAMPLES=401
export LILSCRIPT_STATISTICAL_SAMPLES
"$CARGO" fmt --all -- --check
node "$ROOT/scripts/check-doc-links.mjs"
"$CARGO" clippy --all-targets -- -D warnings
"$CARGO" test --all-targets
"$ROOT/scripts/verify.sh"
"$ROOT/benchmarks/run.sh"
node --test "$ROOT/benchmarks/codec-contract.test.mjs"
node --test "$ROOT/comparison/lib/build-app.test.mjs"
node --test "$ROOT/benchmarks/pass-ablation.test.mjs"
node "$ROOT/benchmarks/finite-values/run.mjs"
node "$ROOT/benchmarks/ir-variants/run.mjs"
node "$ROOT/benchmarks/closure-factory-variants/run.mjs"
node "$ROOT/benchmarks/loop-spelling/run.mjs"
node "$ROOT/benchmarks/mutation-spelling/run.mjs"
node "$ROOT/benchmarks/profile-guided/run.mjs"
node "$ROOT/benchmarks/paired/run.mjs" --check
node --test "$ROOT/benchmarks/statistics.test.mjs"
node "$ROOT/benchmarks/compact-intrinsics/run.mjs"
node "$ROOT/benchmarks/nullish/run.mjs"
node "$ROOT/benchmarks/web-number/run.mjs"
node "$ROOT/benchmarks/enum-match/run.mjs"
node "$ROOT/benchmarks/record-object-json/run.mjs"
node "$ROOT/benchmarks/collection-syntax/run.mjs"
node "$ROOT/benchmarks/regex-literals/run.mjs"
node "$ROOT/benchmarks/async-exceptions/run.mjs"
node "$ROOT/benchmarks/inheritance-generators/run.mjs"
CARGO="$CARGO" "$ROOT/comparison/run-all.sh"
# Publish every normative input from the same release compiler/scorer before
# the fail-closed web catalog consumes it. Solid runs before the popular lane
# because that publisher incorporates the current integrated Solid report.
node "$ROOT/benchmarks/paired/run.mjs"
npm --prefix "$ROOT/benchmarks/apps" run benchmark
npm --prefix "$ROOT/benchmarks/libraries" run benchmark
npm --prefix "$ROOT/benchmarks/scenarios" run benchmark
npm --prefix "$ROOT/labs/solid-client" run setup
npm --prefix "$ROOT/labs/solid-client" run check
node "$ROOT/benchmarks/popular/measure-jquery.mjs"
npm --prefix "$ROOT/benchmarks/popular" run benchmark
# Lilastro's fine-grained report owns behavioral/performance evidence. Its
# separately attested browser builds are then copied into the Motion web lane.
npm --prefix "$ROOT/lilastro" run report
npm --prefix "$ROOT/lilastro" run build:browser-fixtures
npm --prefix "$ROOT/benchmarks/popular" run publish:motion-lab
# This non-check invocation publishes the fresh paired Chromium runtime report.
npm --prefix "$ROOT/benchmarks/browser" run benchmark
# Materialize the catalog from those fresh reports before its tests inspect it.
npm --prefix "$ROOT/web" run catalog
npm --prefix "$ROOT/web" test
npm --prefix "$ROOT/web" run build
npm --prefix "$ROOT/vscode-extension" run package

printf 'LilScript release checks passed.\n'
