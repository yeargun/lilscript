# Library compatibility lab

This project compares version-pinned, installed npm packages with complete LilScript ports
of their documented callable root entrypoints. It is separate from the synthetic
compiler corpus and from context-only ecosystem builds.

```sh
cd benchmarks/libraries
npm install
npm run benchmark
```

The harness performs four behavior gates for each app:

1. installed package built by Vite 8;
2. the same installed package prebundled by esbuild and optimized by Closure ADVANCED;
3. LilScript-generated JavaScript;
4. LilScript-generated C and the native executable compiled from it.

It then runs dense differential API tests from `test/compatibility.test.mjs`, plus
identical API throughput and retained-memory workloads in isolated Node processes.
Size eligibility is measured separately on the reusable selected root API from
LilScript ESM, Vite 8 library mode, and a Closure ADVANCED artifact with the same
named public contract. This prevents fixed demo inputs and whole-program constant
specialization from removing the implementation being compared. Publication
requires LilScript's reusable surface to be no larger than both baselines in raw
bytes and the configured transport codec, and requires median throughput and
retained memory to remain within 1.05x npm.
The retained-memory lane first runs one complete unretained workload before its
baseline GC. This puts both implementations past V8 tier-up thresholds so JIT
code created during warmup is not nondeterministically charged to the measured
retained-result delta.

The workspace additionally pins `nanoid@6.0.1` and `yocto-queue@1.2.2` as
audited exclusions. They do not enter totals: Nano ID v6's pooled Node root and
non-secure entrypoint are distinct from the exact v5 browser port in the
populated-package lab, while yocto-queue still needs private exported-class
state plus `Symbol.iterator`/generator semantics without a wrapper allocation.
`audit-exclusions.mjs` records and tests their installed entrypoint export names.

The current gate covers eight npm packages across seven independently built apps.
Only rows passing every gate enter `results`; every complete port remains in
`diagnostics` with exact blockers so compiler regressions stay visible. Reusable
surface sizes are the publication gate; checked demo-app sizes and load/execution
times remain diagnostics. Native artifacts are correctness gates, not transfer-size
rows.

Reusable LilScript surfaces use `surface-size.toml`, whose explicit public-arrow
mode matches the selected packages' documented callable contract. Constructor use
of a JavaScript function object is outside these typed package domains. The checked
all-target apps continue to use the repository's ordinary constructible-function
ABI, so the size setting is isolated and auditable.

Selection evidence and exclusions live in `compatibility/libraries.json`.
Generated measurements are written to `RESULTS.md`, `build/results.json`, and
`web/src/library-results.json`.
