# Evidence

Parent: [tree](../README.md). Mission: [how to judge a change](../mission.md).
Current snapshot: [`docs/current-status.md`](../../current-status.md).

Test meaning is defined by [verification](../verification/README.md); work order lives
in the [single migration plan](../../migration/index.md); external ideas live under
[research](../research/README.md).

Claims about compression need a semantic boundary, source revision, compiler,
config, artifact, baseline toolchain, codec, and harness fingerprint. Tracked
generated reports own numbers; prose here explains scope and limitations.
**Most pages here were measured on the compiler route deleted on 2026-09-23 (plan
M1)**; each records its compiler, and a number from the old route is a record of
that route, never a claim about the one compiler. Current standings are in
[current status](../../current-status.md).

How the compiler decides representations (including heuristics that evidence
cannot yet justify as global): [current architecture](../compilation/current-architecture.md),
[future architecture §9](../../future-architecture.md#9-choices-search-and-the-objective). When a port loses, classify
compiler bug vs missing proof vs JS-shaped rewrite:
[compressor surface](../language/compressor-surface.md).

## Pages

### Suites

- [Paired web micro suite](micro-suite.md) — generated catalog plus
  [`canonical/`](../../../comparison/cases/canonical/) folders
- [Structural algorithm suite](algorithm-suite.md) — audited 11-pair/42-vector
  whole-program corpus
- [Corpora and lanes](corpora-and-lanes.md) — what each evidence boundary can claim

### Method

- [Configuration ablations](config-ablations.md)
- [Negative results](negative-results.md)
- [Toolchain provenance](toolchain-provenance.md)

### Ports

- [Library proof matrix](library-proof-matrix.md) — required boundary/evidence for every maintained port
- [jQuery](jquery.md)
- [Marked](marked.md)
- [MobX](mobx.md)
- [Closure and corpus](closure-comparison.md)
- [Motion compatibility](motion-compatibility.md)
- [Large-library evidence contract](../../../comparison/large-libraries/README.md)
- [Tracked immutable seed](../../../comparison/large-libraries/results/seed.json)

### Numbers

- [Benchmark results](benchmark-results.md) — core synthetic corpus sizes (no jQuery row)
- [Post-minify audit](vite-closure-minification-audit.md) — post-minify is not a global win

Related: the old route's [optimization coverage](../history/optimization-coverage.md),
[`docs/differential-testing.md`](../../differential-testing.md).
Related labs: [`benchmarks/popular/RESULTS.md`](../../../benchmarks/popular/RESULTS.md),
[`comparison/README.md`](../../../comparison/README.md).

## How to read a size number

- Name the artifact (raw LilScript JS, esbuild, terser, npm min, Closure ADVANCED).
- Name the codec (raw / gzip-9 / brotli-11).
- Name the **config** (`priority`, `candidate_search`, mangle exports).
- For a cross-tool claim, name the selected objective and compare only that
  artifact's matching metric; raw/gzip/Brotli claims require separate builds.
- Do not compare a mangled closed-world app to a public-API library without saying so.
- Distinguish direct compiler output from a deployment pipeline that adds a
  banner, facade, bundler, or JS minifier. Only direct output versus an
  independently authored eligible JS baseline supports a compiler claim.
- A before/after LilScript comparison proves regression or recovery, not a win
  over JavaScript tooling.
- Do not copy a result into multiple pages. Link the tracked result and describe
  only its boundary and interpretation.

## Numerical Authority

`comparison/cases/summary.json` and other working summaries may be ignored or
regenerated. Publication requires a tracked immutable report. The current
large-library seed does not yet represent every latest Motion, Marked, MobX, and
jQuery artifact; the [baseline/support inventory](../../migration/record-2026-09.md#001-baselines-and-support-inventory)
must requalify the complete maintained set.
