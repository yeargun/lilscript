# Chromium runtime gate

This lane executes the mechanically paired workload artifacts in headless
Chromium. It times warmed functions in alternating order, uses 400 samples per
artifact, and computes deterministic paired-bootstrap 95% upper confidence
bounds for both the median and p95 LilScript/Closure time ratios. A workload
fails when either upper bound exceeds `1.03`.

The result is a scoped steady-state regression gate, not proof that every
LilScript program is faster than every JavaScript program. Transfer and parse
sizes are gated separately by `benchmarks/paired/run.mjs`.
The timed LilScript JavaScript is that runner's explicitly declared
Brotli-objective deploy artifact; raw and gzip diagnostics do not select the
runtime artifact.

Every invocation generates an ephemeral paired report, verifies its compiler
binary and Brotli-config digests against the current repository files, and
embeds that compiler, config, objective contract, scorer, and source-report
digest in the schema-2 Chromium report. `verify` therefore cannot silently use
an older checked-in paired artifact.

```sh
npm --prefix benchmarks/browser install
npm --prefix benchmarks/browser run install-browser
npm --prefix benchmarks/browser run benchmark
npm --prefix benchmarks/browser run verify
```

`benchmark` publishes fresh timing evidence. `verify` runs the same regression
gate without rewriting the checked-in result files.
