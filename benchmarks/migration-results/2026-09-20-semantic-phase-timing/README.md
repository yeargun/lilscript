# Semantic Phase Timing

012-P1 extends the existing opt-in `LILSCRIPT_TIMING` owner. Nine phase buckets
separate JS demand, target formation, structural verification, edition checks,
naming-basis preparation, name allocation, printing, canonical gzip and
canonical Brotli. They report `<phase>_calls` and `<phase>_ms`, not invented byte
totals. One stack guard at each existing operation records attempts, including
refusal and unwind. No optimizer, cache, policy, budget or default changes;
schedule 22 and resource format 3 remain unchanged.

Counters are process-global. Times accumulate elapsed scopes, not process CPU,
and cannot generally be added: workers and enclosing legacy buckets can overlap.
`target_names` includes lazy scoped-name preparation. Canonical codec buckets
include measurement attempts through the common admitted encoder from semantic,
legacy and inspection routes, including refusal before backend entry; they are
distinct from the enclosing legacy `codec` bucket. Raw scoring
does not call an encoder, though artifact bookkeeping has logical codec work.
With telemetry disabled, guards use the existing cached Boolean and read no
clock. Profiling overhead can affect wall-clock cutoffs; these comparisons use
deterministic work/probe caps instead.

## Verification

There are **186 distinct accepted debug checks and 186 release checks**, limited
to the changed owners, codecs, public service and nearby search/target behavior.
The new subprocess test compares independent timing-off/on executions, including
exact winner bytes, scores, search counters and logical resource accounting.
It verifies one formation for three naming trials, actual encoder counts,
invalid-target and output-byte refusal, unwind, and absence of JS/codec phases
for native success and the existing native-export refusal. Raw-only output
invokes neither encoder. Owners finish with zero retained bytes.

All attempts remain recorded:

- [Initial build refusal](../2026-09-19-artifact-service/run-2026-09-20T01-54-46.431Z/receipt.json):
  wrong test-only `OutputError` import; no tests execute.
- [Initial test attempt](../2026-09-19-artifact-service/run-2026-09-20T01-55-54.497Z/receipt.json):
  181 checks pass; the new native fixture requests the unsupported default
  exported ABI. The final test preserves that refusal explicitly and adds a
  supported non-exported native case, without changing production behavior.
- [Encoder-instrumented attempt](../2026-09-19-artifact-service/run-2026-09-20T01-58-00.402Z/receipt.json):
  185 checks pass; the new raw assertion incorrectly requires zero bookkeeping
  work. It is corrected to assert zero actual probes/encoder attempts.
- [Corrected debug test](../2026-09-19-artifact-service/run-2026-09-20T01-59-23.125Z/receipt.json):
  the single test passes. Only that assertion changes; 185 unaffected passes
  from the preceding binary are reused, rather than rerunning the cohort.
- [Final release qualification](../2026-09-19-artifact-service/run-2026-09-20T02-00-06.766Z/receipt.json):
  all 186 checks pass on unchanged final inputs. Build: 255.195 seconds;
  test execution: 9.665 seconds. No inherited compiler/profile overrides;
  optimization level 3, debug assertions and overflow checks off.

## Repeated Diagnostics

[Receipt](receipt.json), SHA-256
`bc28276869156846eadd081567ae86c864a68e78c06028ce63c9975c6ad66f78`,
records five alternating off/on pairs using the same preserved release test
binary. Each observation starts a fresh process with `RAYON_NUM_THREADS=1` and
warm OS caches, then runs fixed direct/naming/structural cases. All 30 case
observations preserve identical artifacts, scores, search and resource fields.
This is a release-profile test executable with `cfg(test)` instrumentation,
not a shipped-CLI benchmark or held-out library qualification.

The structural case retains **243 raw / 167 gzip / 143 Brotli bytes**, six
structures, 18 renders and 34 optional probes plus two baseline encodes.
Median accumulated phase times across the five enabled observations:

| Phase | Calls | Milliseconds |
| --- | ---: | ---: |
| Demand | 6 | 0.318 |
| Formation | 6 | 0.297 |
| Structural verification | 7 | 0.034 |
| Edition checks | 7 | 0.000410 |
| Naming basis | 7 | 0.020 |
| Name allocation | 18 | 0.127 |
| Printing | 18 | 0.097 |
| Canonical gzip | 18 | 0.415 |
| Canonical Brotli | 18 | 14.636 |

Case wall medians are 16.623 ms disabled and 16.536 ms enabled; this noisy
difference is **not a speed improvement**. Case wall time includes TOML and
public source compilation. GNU time's separately recorded CPU/RSS covers the
whole child test, including native/refusal checks and JSON output; it is not
per-case compiler cost. Full observations, extrema and host/protocol identities
are in [samples.json](samples.json) and [summary.json](summary.json).

The naming-only case forms once and renders three styles, but performs only
two gzip and two Brotli encodes because exact-score reuse remains effective.
These small semantic fixtures are codec-heavy; they do not explain the large
legacy Marked emission cost. Do not add a naming cache or more default effort
on this evidence. Profile larger existing semantic module workloads before
choosing a cost-driven change, keeping source/contract/quality comparisons fixed.

## Exact Replay And Audit

The same final release binary replays the previous 32-row search ablation.
All 24 oracle records and every field of every row match the
[reclamation baseline](../2026-09-20-search-reclamation/README.md), including
bytes, requested scores, 64 winner executions, exploration, work, peak retained
memory and stop reasons. [Rows](search-rows.json) and
[oracle](search-oracle.json) retain the complete comparison. No compiler rebuild
is used for the samples or replay, and no full fleet runs.

The [root audit](audit.json), SHA-256
`7c43d59ae13d90689b3979f48ab02691f04824d042a02ae0c64065e6536ce038`,
verifies 981 final inputs, 151 qualification/diagnostic outputs, four preserved
binaries and all eight changed source files. There is no independent agent
review. Input digest:
`742f203a45dfd5483ff2774291e29563abf9b5adb5b9e39c955bb8eb380d8dc8`.
Final release binary:
`/tmp/lilscript-semantic-phase-timing-baseline-20260920/release-tests`,
39,990,288 bytes, SHA-256
`9c65b05499cc73e51212afc343b01dfa1493ade88a018f8ea6b42a5a49d5078a`.
Pre-edit sources, failed test fixtures and intermediate binaries are preserved
in the corresponding `/tmp/lilscript-semantic-phase-timing-*-20260920/` folders.

The target design is unchanged. This is cost observability and bounded evidence,
not a compiler-speed win, compression improvement or completed migration gate.
The [single migration plan](../../../docs/migration/index.md) retains the full
public/library/native qualification and performance prerequisites.
