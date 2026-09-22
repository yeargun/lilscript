# Production Semantic CLI Cost

012-P2 measures the normal optimized production CLI on the unchanged
12-module integrated-architecture source/host fixture. It contains 44 units,
1,219 operations, retained closures, reentry/throws, value products and archived
Marked regex/string code. All 23 fixture files are pinned. This is **not whole
Marked or a held-out library**. Compiler code, defaults, schedule 22, resource
format 3 and the target design are unchanged.

## Production Checkpoint

The [release qualification](../2026-09-19-artifact-service/run-2026-09-20T02-12-30.329Z/receipt.json)
passes 89 focused library checks, one CLI check and the unchanged original
direct/search/edit/native consumers. Its 15 compared JS/C deliveries are
[byte-identical](../2026-09-19-artifact-service/semantic-cli-delivery-20260920/receipt.json)
to the preceding C7 release checkpoint. Combining this cohort with the previous
186 checks on the **identical final library-test binary** gives 242 distinct
accepted library checks, not 275. This does not rerun the older 303-check cohort
or qualify the fleet.

The production profile is optimization level 3, without debug assertions,
overflow checks or inherited compiler/profile overrides. Test build: 139.773
seconds; CLI/codec/example build: 9.112 seconds. Those are Cargo build times,
not LilScript compilation performance. All subsequent measurements reuse these
binaries, preserved under `/tmp/lilscript-semantic-cli-cost-20260920/`.

## Observational Comparison

The [accepted paired receipt](run-2026-09-20T02-22-50.724Z/receipt.json), SHA-256
`c5f3908179701ad31e7c51c7cfeee677d054fe8a039f8b190333d57439c4c4ad`,
records five alternating timing-off/on pairs after one warmup. Each invocation
is a fresh process, with warm OS caches and `RAYON_NUM_THREADS=1`. Source,
configuration, contract and deterministic budgets are unchanged. There is no
compiler wall-clock cutoff; the external command limit remains 60 seconds.

Every sample produces the same **17,476 raw / 3,834 gzip / 3,503 Brotli bytes**,
SHA-256 `8d2ea214d1f0fcc014e6aca5823bf2c820bfd46ca80662635f5620078f675466`.
This is a Brotli-only optimization request: gzip is an independent measurement
of that winner, not an independently optimized gzip winner. All stable service
report fields, search decisions and resource counts match with telemetry off/on.
All final ledgers report zero retained bytes.

Median enabled phase durations:

| Phase | Calls | Milliseconds |
| --- | ---: | ---: |
| Demand | 33 | 18.410 |
| Formation | 33 | 19.909 |
| Verification | 34 | 1.603 |
| Edition | 34 | 0.002 |
| Naming basis | 34 | 1.084 |
| Name allocation | 97 | 6.047 |
| Printing | 97 | 23.017 |
| Canonical gzip | 0 | 0 |
| Canonical Brotli | 97 | 1,877.934 |

Service wall medians are 1,965.194 ms disabled and 1,955.012 ms enabled. This
small shared-host difference is not negative profiling overhead or a speedup.
The first qualified artifact appears internally after about 26 ms; the CLI
does not deliver it early. GNU time separately records whole-process wall,
user/system CPU and peak RSS, including config I/O, explanation and file output.
RSS ranges from 14,096 to 14,304 KiB across measured pairs; charged memory is a
different accounting scope. Accumulated phase elapsed times are not process CPU
or generally additive. Full extrema and observations remain in the receipt and
[summary](run-2026-09-20T02-22-50.724Z/summary.json).

The [first attempt](run-2026-09-20T02-21-47.960Z/receipt.json) is retained as
failed: all 11 compiles matched, but the benchmark read the codec JSON envelope
as an artifact. Only the runner parser was corrected; production and fixture
inputs did not change. Its incomplete external checks earn no qualification.

## Probe-Budget Ablation

The [sweep receipt](run-2026-09-20T02-25-40.668Z/receipt.json), SHA-256
`8a41fdfb0fe79fccb60a50e035d2beac4a953fc59d9b64b0c1fa109df164bb1a`,
records three forward/reverse/forward rounds over ten configurations. Only the
optional probe cap and immediate/staged schedule vary. The proposal cap stays
96, beam width 10, render batch 8 and diversity interval 4. Original and generated
configs are parsed with `tomllib`; every other resolved policy field, source
input, service request and search request is checked equal. All observations
within each configuration have identical output and deterministic accounting.

| Probe Cap | Immediate Brotli | Immediate ms | Staged Brotli | Staged ms |
| ---: | ---: | ---: | ---: | ---: |
| 8 | 3,522 | 188.484 | 3,504 | 202.235 |
| 24 | 3,512 | 523.429 | 3,503 | 521.818 |
| 48 | 3,512 | 1,015.474 | 3,503 | 1,006.308 |
| 96 | 3,503 | 1,942.935 | 3,503 | 1,949.541 |
| 192 | 3,503 | 1,944.514 | 3,503 | 1,944.863 |

Times are three-sample service medians, not a statistical release-speed gate.
At caps 96 and 192, only 96 optional encodes execute; the proposal limit stops
search. At 24 probes, staged scoring delivers the **exact same bytes** as the
full run in about 27% of the time on this fixture. At eight probes, it loses one
Brotli byte but forms 11 structures and renders 33 candidates, versus immediate
scoring's three structures and nine renders. Extra exploration is not free:
staged optional work is 4,234,261 versus 1,079,164 at that cap. Complete costs,
scores, counts, limits and extrema are in the [sweep summary](run-2026-09-20T02-25-40.668Z/summary.json).

This supports staged, diverse use of expensive encodes, not a universal
24-probe default or plateau-based stopping. Earlier finite-oracle evidence
already demonstrates that evicting a locally losing parent can prevent the
useful combination regardless of remaining probes. Evaluate interaction
retention and quality-versus-effort on frozen independent workloads before
adopting new defaults or adaptive ranking. Naming-cache work is not justified
by these timings; the large legacy Marked emission bottleneck remains separate.

## Evidence And Reproduction

All 42 accepted paired/sweep artifacts, including two warmups, execute the
unchanged `setup.js`, `host.js` and `expected.json`, then receive independent
canonical codec replay. These checks run after measured compiles. Original
sources, assertions, prerequisites and qualification timeouts remain intact;
reduced probe caps apply only to the explicitly labeled ablation rows.

The [root audit](audit.json), SHA-256
`ad9f99ff3fae83a9aafb89abcd60289f4e53386f5046880821c1ea3e11ed1a7b`,
reconciles 608 outputs, all five preserved binaries, three runner versions and
981 compiler/build/test inputs, digest
`742f203a45dfd5483ff2774291e29563abf9b5adb5b9e39c955bb8eb380d8dc8`.
There is no independent agent review. CLI SHA-256:
`009ceb63ed654308706c2802f309b12def368a501a9d4cad498e9262b040ef65`.

Run `node benchmarks/migration-results/2026-09-20-semantic-cli-cost/measure.mjs`
for the paired study, or append `--sweep` for the explicit ablation. Each creates
a new receipt directory and checks the pinned production qualification first.
The runner is a measurement client of the existing compiler and bounded-command
owner, not a new optimizer. No Rust rebuild occurs in either study. The
[single migration plan](../../../docs/migration/index.md) retains all open gates.
