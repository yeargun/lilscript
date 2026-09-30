# Current status

Updated for the 2026-09-29 planning revision, from implementation commit `8b3d5cc5a1429919b6000831085ce807ec9aeed3`.

There is one compiler. The [migration plan](migration/plan.md) owns the target architecture and acceptance rules; the [checklist](migration/index.md) owns live progress. This page reports evidence, not a second task list.

## Implemented foundation

The compiler checks into a typed Program IR with shared facts and edit machinery, applies program rules, forms target code, allocates names and judges JavaScript alternatives with the requested codec. Native has its own formation and remaining coverage gaps. See the [source map](knowledge/compilation/current-architecture.md).

Y5a/b is the last verified implementation batch: application scripts own their private roots and frames, while lexical strictness is preserved separately. Default IIFE wrapping is still planned. The new plan does not imply that its remaining configuration, naming, search or measurement work is already implemented.

## Evidence

The accepted [ratchet baseline](../tests/ratchet/baseline.json) is Y5. Its application totals beat the pinned Closure ADVANCED totals under raw, gzip and Brotli, with individual application losses remaining. Its algorithm totals still lose under all three objectives. The figures are recorded in the [replanning record](migration/history.md#2026-09-29-focused-plan-and-archive).

The [Y5 record](old-history/migration/history.md#2026-09-30-batch-y5-an-application-script-owns-its-roots-and-frames-owner-answer-y5-halves-a-and-b) contains unit/case results, port accounting and compile-cost measurements. Seven primary port suites passed and their 681 compared delivered files were byte-identical to V2. Four known site receipt failures remained. This evidence does not establish full runtime parity or that every delivered port file is already compiler-written without a post-minifier.

The benchmark contract, calibration/held-out program, dedicated runtime runner and remaining delivery migration are open work. Therefore broad competitor and runtime victory claims remain unqualified. Keep the exact binary, source, configuration and artifact scope with every measurement.

## Verification tools

[testing.md](testing.md) documents the current case runner, port runner, generic ratchet, expected-failure ledgers and output admission. Legacy M-task IDs in ledgers map to current owners through [coverage.md](migration/coverage.md). The revised [acceptance rules](migration/plan.md#9-verification-and-acceptance) distinguish correctness gates, per-objective totals, runtime evidence and compilation work.

The previous status page is [archived](old-history/current-status.md). Its measurements and descriptions are historical.
