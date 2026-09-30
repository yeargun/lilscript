# C2: isolated paired runtime runner

The [runner protocol](../../../docs/testing.md#paired-generated-program-runtime) separates import/startup, warmed workload and whole-process cost, checks an independent oracle on every invocation, and pins all declared inputs. Optional post-GC retained memory is reported as a delta, not an allocation total. Seven integration tests cover actual fresh workers, alternating order, artifact identity, ambient Node option isolation, wrong results/counters, changed dependencies, timeout and opt-in retained memory.

The [smoke report](runtime-cost.json) retains five alternating pairs of the [reference](reference.mjs) and [compiler-written candidate](candidate.mjs). Each sample makes 65,536 calls; three untimed warm-up runs precede it. The independent expected sum is `32 × 256 × (255 × 256 / 2) = 267,386,880`. Every warm-up and measured invocation passes that oracle and the declared call counter. The workload has no retained object, so retained-memory fields are null.

The candidate was built from [source](candidate.lil) and [configuration](config.toml) with compiler source `5dd3b4bd`, pinned binary `~/lilscript-work/bin/s4-primitive-1/lilscript` SHA-256 `5f29c9cbb89de554b38eb8e2539e311b23217e4fa4793a61c27ceb1fbf0daac9`. The [build receipt](build.json) retains effective policy, input and artifact identities. Build command, from repository root:

```sh
~/lilscript-work/bin/s4-primitive-1/lilscript \
  benchmarks/migration-results/2026-09-30-c2-runtime/candidate.lil \
  --target js-module \
  --config benchmarks/migration-results/2026-09-30-c2-runtime/config.toml \
  -o benchmarks/migration-results/2026-09-30-c2-runtime/candidate.mjs \
  --explain json
```

Run `scripts/runtime-cost.py` on [runtime.json](runtime.json), selecting a new work directory and report path. The retained report pins the engine, runner and workload files. This short synthetic fixture qualifies measurement plumbing and oracle enforcement. It does **not** establish a runtime improvement, representative-port parity, peak memory behavior or the full C2 benchmark contract.
