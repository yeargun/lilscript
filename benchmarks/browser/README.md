# Chromium runtime gate

This lane executes the mechanically paired workload artifacts in headless
Chromium. It times warmed functions in alternating order, uses 50 samples per
artifact, and computes a deterministic 95% bootstrap upper bound for the ratio
of LilScript median time to Closure median time. A workload fails when that
upper bound exceeds `1.03`.

The result is a scoped steady-state regression gate, not proof that every
LilScript program is faster than every JavaScript program. Transfer and parse
sizes are gated separately by `benchmarks/paired/run.mjs`.

```sh
npm --prefix benchmarks/browser install
npm --prefix benchmarks/browser run install-browser
npm --prefix benchmarks/browser run benchmark
npm --prefix benchmarks/browser run verify
```

`benchmark` publishes fresh timing evidence. `verify` runs the same regression
gate without rewriting the checked-in result files.
