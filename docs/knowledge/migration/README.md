# Active compression-verification migration

Parent: [knowledge tree](../README.md). Verification contract:
[verification](../verification/README.md). Research:
[research](../research/README.md). Previous plan:
[migration archive](../old-migration/README.md).

This is the active route from today’s useful but narrow comparison lanes to a
compression claim that is broad, reproducible, and hard to game. The work is ordered
by proof dependency: freeze measurements first, then grow semantic coverage, then
exercise delivery and global search, and only then use large libraries as release
evidence.

The target invariant for every eligible paired case is:

> The LilScript source and independently authored JavaScript source satisfy the same
> observable contract, and each objective-specific LilScript artifact is no larger
> than the best eligible JavaScript baseline in that artifact's gated metric.

“Best” is metric-specific: raw, gzip-9, and Brotli-11 may have different LilScript
artifacts and baseline winners. A strict-win case must beat those minima, not a
convenient single tool. Cross-metrics from an objective artifact are diagnostic.
Eligibility, boundary, target syntax, config, and tool versions are part of the case.
See [paired-case contract](../verification/paired-case-contract.md).

## Current baseline, not the destination

The hardened `comparison/cases/run.mjs` now compiles separate size-first gold
artifacts for raw, gzip, and Brotli; compares each against its own minimum valid
Terser, Oxc-via-Rolldown, or esbuild artifact; enforces stdout equivalence; and has no
small-file codec exemption. It validates a checked-in whole-catalog oracle digest,
uses `candidate_search = "always"` so its configured 1536 ceiling is effective,
records compiler/tool/config/corpus provenance, and runs through
`comparison/run-all.sh` in `scripts/release-check.sh`. Generated `summary.json` owns
the current case and win counts; focused `--only` reports must not be mistaken for a
full-corpus result.

The remaining gap is breadth and independence, not the three byte comparisons. The
durable sources are still one catalog whose ignored per-case folders are generated,
the reference JavaScript defines the stdout oracle, and the lane does not yet cover
public APIs, browser effects, module artifact sets, Vite, or eligible Closure
ADVANCED rows. Those are the reasons phases 01–09 remain active. See the exact current
runner contract in
[`comparison/cases/README.md`](../../../comparison/cases/README.md).

## Phases

| Phase | Purpose | Exit signal |
|---|---|---|
| [00](00-freeze-current-evidence.md) | Freeze current behavior and provenance | Existing lanes are reproducible without changing their meaning |
| [01](01-paired-runner-contract.md) | Enforce the paired-case and codec contract | Metric-specific baseline minima; raw/gzip/Brotli always reported and gated |
| [02](02-scalar-language-core.md) | Exhaust scalar syntax and numeric semantics | Scalar coverage matrix has no unowned cells |
| [03](03-control-flow-functions-effects.md) | Grow through functions, closures, effects, async/error flow | Semantic oracle and size gate cover higher-order control flow |
| [04](04-aggregates-collections-objects.md) | Prove LilScript-native representation wins | Aggregates/collections cover internal and public ABI cases |
| [05](05-modules-delivery-progressive.md) | Verify bundling, manual splits, lazy loading, PE | Deploy artifacts and behavior are compared at matching boundaries |
| [06](06-browser-host-boundaries.md) | Add real browser and host behavior | Chromium cases cover DOM/events/network/storage with explicit externs |
| [07](07-global-codec-search.md) | Test non-local and config-dependent decisions | Exhaustive small fixtures prove exact minima; production reports the best objective found within budget |
| [08](08-scale-corpus-release.md) | Scale past 500 and promote gates | Micro, app, browser, Closure, and large-library lanes block releases appropriately |
| [09](09-jquery-library-convergence.md) | Converge the jQuery public-library row | Exact surface + metric-specific size + behavior/performance/memory gates all pass |

## Working rules

1. Add several related cases, run that slice, then run the full lane. Do not collect
   hundreds of unexecuted fixtures.
2. A size failure is a compiler/port investigation, not a reason to weaken the gate.
   Quarantine only with an owner, exact artifact, and smallest reproducer.
3. No baseline is post-selected away. A tool can be marked ineligible only for a
   recorded semantic, module, or ABI mismatch.
4. Never compare public-library LilScript output with closed-world application JS.
5. Keep losing experiments as ablations or archive notes when they teach the search;
   do not publish them as wins.
6. 500 cases is a waypoint. Completion is coverage plus release enforcement, not a
   round number.

## Ownership of truth

- This folder owns order and exit criteria.
- [Verification](../verification/README.md) owns test and measurement contracts.
- [Evidence](../evidence/README.md) owns measured claims.
- [Config](../config/README.md) and [compilation](../compilation/README.md) own current
  compiler behavior.
- Generated summaries own numbers; prose must point to them rather than becoming a
  second result database.
