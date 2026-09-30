# Current status

Updated 2026-09-30. The latest qualified source here is `aa1a3783`; implementation in progress is tracked in the checklist below.

There is one compiler. The [migration plan](migration/plan.md) owns the target architecture and acceptance rules; the [checklist](migration/index.md) owns live progress. This page reports evidence, not a second task list.

## Implemented foundation

The compiler checks into a typed Program IR with shared facts and edit machinery, applies program rules, forms target code, allocates names and judges JavaScript alternatives with the requested codec. Native has its own formation and remaining coverage gaps. See the [source map](knowledge/compilation/current-architecture.md).

Application output now defaults to a private IIFE, with classic-script strictness preserved. Qualified batches also cover bounded primitive evaluation, typed record spread, private-field naming, lexical naming refinements and several forms of analysis/measurement reuse. Their exact scopes and remaining work are in the [live checklist](migration/index.md), rather than a second milestone list here.

## Evidence

The [Q2 mention-reuse record](../benchmarks/migration-results/2026-09-30-q2-statement-mentions/README.md) passes all 1,747 active library tests, including the large table oracle, with four existing ignores. All 642 generic programs pass under each objective. It preserves all 1,926 generic outputs and all 20 measured reference-port artifacts from the preceding Q3 release; Marked/Zod/PostHog suites pass 29/1,353/21. Paired measurements show lower Marked/Zod compilation CPU with unchanged peak accounted memory; this is not a runtime-performance claim.

The [D3 independent-objective record](../benchmarks/migration-results/2026-09-30-d3-independent-objectives/README.md) qualifies the Rust build API's separate policies, portfolios and winners over one checked program. All 1,753 active library tests and nine CLI tests pass; all generic and reference-port outputs remain identical. TOML/CLI objective-set delivery is in progress.

The [Q3 higher-effort record](../benchmarks/migration-results/2026-09-30-q3-level14/README.md) documents why expensive deferred naming starts default to effort 14+, with an explicit level-13 opt-in and veto. Core gains and external-wrapper regressions are both reported. [Configuration](configuration.md) describes each control's situation, default, effort gate and tradeoffs.

The accepted [ratchet baseline](../tests/ratchet/baseline.json) is current through that qualification. [History](migration/history.md) owns earlier Y5 and subsequent measurements. The broader maintained/held-out competitor contract, runtime qualification, compiler-written final formats, native coverage and release work remain open. None of these bounded results establishes blanket Closure ADVANCED superiority or complete migration.

## Verification tools

[testing.md](testing.md) documents the current case runner, port runner, generic ratchet, expected-failure ledgers and output admission. Legacy M-task IDs in ledgers map to current owners through [coverage.md](migration/coverage.md). The revised [acceptance rules](migration/plan.md#9-verification-and-acceptance) distinguish correctness gates, per-objective totals, runtime evidence and compilation work.

The previous status page is [archived](old-history/current-status.md). Its measurements and descriptions are historical.
