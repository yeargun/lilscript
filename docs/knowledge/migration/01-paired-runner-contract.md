# Phase 01 — paired runner and gate contract

Parent: [migration](README.md). Primary contract:
[paired cases](../verification/paired-case-contract.md). Baselines:
[toolchains](../verification/baseline-toolchains.md).

## Objective

Turn the micro suite into the smallest authoritative compression regression loop.
Every case must answer two independent questions: “are the programs observably
equivalent?” and “which complete served artifact is smaller?”

## Shipped foundation

- Raw, gzip-9, and Brotli-11 use separate size-first LilScript gold configs with
  `candidate_search = "always"` and
  separate minimum valid Terser/Oxc/esbuild baselines.
- Every baseline and LilScript artifact must reproduce the stdout oracle; a bad
  baseline fails the case and cannot win a size column.
- Tiny files have no codec exemption.
- A checked-in whole-catalog oracle digest makes reference JS/stdout/name/strictness
  changes explicit. Schema 5 records aligned ES2022 options, the canonical scorer,
  resolved tools/runtime, source and artifact hashes, durations, and
  compiler/config/corpus provenance.
- The runner builds a fresh release compiler unless `LILSCRIPT` explicitly selects
  one, and `comparison/run-all.sh` makes the lane release-blocking.

## Remaining work

1. Make folder-per-case sources and metadata canonical. A generator may create cases,
   but a materialized case must be reviewable and runnable alone.
2. Replace or supplement reference-JS-derived stdout with reviewed independent
   expectations/specifications where the reference itself may be wrong.
3. Add Vite for matching graph cases and Closure ADVANCED only where its
   closed-world/extern boundary is eligible; retain every tool's reason for
   ineligibility in the report.
4. Run both a fast canonical config (`candidate_search = off`) and the intended
   release config. The fast lane diagnoses basic codegen; only the release lane proves
   search behavior.
5. Pin the shipped region-outlining compression/hard-off gates with a config-contract
   case so `ir-compress-pass-variants` cannot regress the allowlist.
6. Extend beyond stdout to API, browser, and complete artifact/module contracts while
   keeping one-case, family, changed-case, and full-suite commands.

## Required report fields

Case ID; contract kind; boundary; LilScript config fingerprint; source hashes;
baseline versions/options; emitted artifact hashes; stdout/DOM/API oracle result;
raw/gzip/Brotli sizes for every candidate; metric-specific winners; gate status;
duration; quarantine metadata if any.

## Exit criteria

- No compressed metric is silently skipped.
- A green row means each independently compiled objective artifact is `<=` the
  minimum eligible baseline in its matching required metric; strict rows use `<`.
  Cross-metrics of that artifact are diagnostics.
- Semantic failure prevents a size result from becoming eligible.
- The runner exposes one-case, family, changed-case, and full-suite commands.
- At least one fixture proves that raw, gzip, and Brotli can select different
  baseline tools without corrupting the comparison.
