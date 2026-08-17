# Evidence

Parent: [tree](../README.md). Mission: [how to judge a change](../mission.md). Roadmap: [`docs/roadmap.md`](../../roadmap.md).

Test meaning is defined by [verification](../verification/README.md); work order lives
in the [active migration](../migration/README.md); external ideas live under
[research](../research/README.md).

Claims about compression need a corpus, tool versions, codec, and scope. This folder records **what the current codebase believes it has shown**, and where it has not.

## Pages

- [Paired web micro suite](micro-suite.md) — generated catalog plus
  [`canonical/`](../../../comparison/cases/canonical/) folders
- [Structural algorithm suite](algorithm-suite.md) — audited 11-pair/42-vector
  whole-program corpus; the post-fix canonical full report is 11/11 with strict
  wins in every raw, gzip, and Brotli lane, while the first red run remains a
  migration checkpoint
- [Corpora and lanes](corpora-and-lanes.md) — what each evidence boundary can claim
- [Configuration ablations](config-ablations.md) — one-variable proof and reports
- [Negative results](negative-results.md) — retained losses and non-monotonic lessons
- [Toolchain provenance](toolchain-provenance.md) — versions, hashes, configs, codecs
- [jQuery port](jquery.md) — large-library pressure test; not a win yet
- [Closure and corpus](closure-comparison.md) — synthetic apps vs Closure `ADVANCED`

## Other measurement docs (contracts, not this tree)

| Doc | Contents |
|---|---|
| [`docs/benchmark-results.md`](../../benchmark-results.md) | Core synthetic corpus sizes (no jQuery row) |
| [`docs/optimization-coverage.md`](../../optimization-coverage.md) | Closure responsibility map + pass schedule |
| [`docs/vite-closure-minification-audit.md`](../../vite-closure-minification-audit.md) | Post-minify is not a global win |
| [`docs/differential-testing.md`](../../differential-testing.md) | Independent AST oracle |
| [`benchmarks/popular/RESULTS.md`](../../../benchmarks/popular/RESULTS.md) | Popular-lab npm vs ports |
| [`comparison/summary.md`](../../../comparison/summary.md) | Size-gated LilScript vs Closure apps |

## How to read a size number

- Name the artifact (raw LilScript JS, esbuild, terser, npm min, Closure ADVANCED).
- Name the codec (raw / gzip-9 / brotli-11).
- Name the **config** (`priority`, `candidate_search`, mangle exports).
- For a cross-tool claim, name the selected objective and compare only that
  artifact's matching metric; raw/gzip/Brotli claims require separate builds.
- Do not compare a mangled closed-world app to a public-API library without saying so.
