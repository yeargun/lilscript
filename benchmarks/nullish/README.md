# Nullish syntax gate

This benchmark compares `??`, `??=`, `?.field`, and `?.[index]` with explicit,
semantically equivalent nullable branches. The workload deliberately alternates
an empty string and a missing map key: using truthiness (`||`) would therefore
fail the output check. Both source variants must agree in JavaScript and native
execution. The compiler fuses `optional?.access ?? fallback` into one lazy CFG
branch, avoiding an intermediate nullable value and a repeated null test.

Each variant is independently compiled with the exact gzip and Brotli cost
models. The gate requires a strict win in the configured codec, plus no more
than a 10% median runtime regression and a bounded retained-heap delta across
eleven alternating isolated Node processes.

```sh
node benchmarks/nullish/run.mjs
```
