# S3 call optimization completion

Compiler: `s3-complete-2`, SHA-256
`71561803d17e70ac89f5b3b50fd6499fd82edbc3364626d916c1fbead3ba4e50`.
[identity.json](identity.json) pins all Rust/Cargo sources, compiler and codec;
[measure.mjs](measure.mjs) reproduces the comparison against S2. Each objective
compiles its own artifact at effort 13; application IIFE and the exact config,
source, independent Node oracle and output are retained under `artifacts/`.

| Scope | Programs | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|---:|
| Ordinary runtime permissions | 14 | −39 | −30 | −8 |
| Explicit recurring helper-sharing permission | 1 | −15 | −4 | −1 |

All 90 old/new behavior observations pass. The sole local loss is two Brotli
bytes on captured activations, traced to identifier choices and retained as G1
work. There is no cross-objective blended verdict or fleet/timing claim. The
accepted ratchet baseline and port outputs are unchanged.

## What changed

Terminal-return normalization and cross-module/nested-closure inlining preserve
lazy evaluation, TDZ, activation storage and capture identity. Immutable product
roots can be rematerialized through copy transports; field-written reference
parameters no longer need an extra bank when complete-use evidence permits it.
Caller-path frequency is bounded and counted. Native module globals are restricted
to entry-region storage, and declared inline results receive ownership cleanup.

Constant signature specialization, alpha-equivalent primitive body sharing,
parameterized helper sharing and bounded repeated-expression inlining are exact
objective alternatives. The original call remains available. Recurring helper
sharing requires an explicit permission. Operation count no longer licenses
unconditional duplication of nonempty shared bodies or large literal payloads.
All new producers honor their independent tactic permissions.

The first measurement exposed a lost range fact after return normalization and
unfolded literals after late expression duplication. Select now consumes branch
facts, and permitted late literal folding follows these families. The final
comparison fixes both regressions. `candidate-1-identity.json` records that earlier
compiler; only `identity.json` describes the final result.

## Focused verification and work

Across the implementation batches, **100 distinct focused checks pass**. Logs
retain shared call/range/storage tests, public raw/gzip/Brotli oracles, permission
vetoes, module cycles, mutation/reentry/TDZ negatives and native execution under
GCC/Clang O0/O2, UBSan and Clang ASan with leak checks. `shared-and-choices.log`
contains five obsolete expectations after the new inlining policy/coverage;
`expectations.log` records all five corrected checks passing. The range fixture's
invalid integer-token spelling in `ranges.log` is corrected and passes in
`range-mutation.log`. No known test failure remains. The final release build and
`git diff --check` pass. The full library/CLI and full fleet were not rerun.

Aggregate logical work over these 45 objective cells:

| Counter | S2 | S3 |
|---|---:|---:|
| Structural proposals | 99 | 108 |
| Structures | 48 | 51 |
| Renders | 144 | 153 |
| Structural codec probes | 34 | 40 |
| Terminal exact judgments | 368 | 304 |
| Terminal positions examined | 4,116 | 4,968 |

Counts describe changed exploration, not CPU speed. No matched timing claim is
made. Broad qualification remains V1; the owner-deferred S1 reruns remain deferred.
