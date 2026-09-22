# Finite Search Schedule Ablation

006-P4 compares **32 fixed configurations** on the existing structural-valley
fixture: beam widths 1/2, immediate/staged scoring, Brotli-only/all-codec requests,
and optional probe ceilings 2/8/24/48. Proposal limit 24, retained candidates 8,
render batch 8, diversity interval 4, source, semantics and family permissions
stay fixed. This is one constructed fixture, not held-out library qualification.
Only the existing search test file changes; no production heuristic or default
is modified.

The independent oracle enumerates eight helper subsets in three naming styles.
All 24 outputs execute the original public-name/arity, numeric, coercion-order
and thrown-identity observations. Every searched observation must match an
oracle recipe, naming plan, exact byte stream and requested score. Selected
incumbents must be the best explored score for their own objective and no worse
than the admitted direct artifact. Brotli-only requests do not measure gzip.
All searches retain existing work/memory/proposal/probe bounds and finish with
zero retained bytes when the compilation owner is released.

## Results

Finite oracle minima are **243 raw / 167 gzip / 143 Brotli bytes**. The direct
recipe's best naming scores are 327 / 181 / 162. The helper subset containing
`distractor` and `first` reaches 151 Brotli despite its singleton minima being
169 and 175. The original `first` plus `second` pair still reaches 161, with
singleton minima 175 and 187. All three together reach 143.

For beam width 2 and Brotli-only requests:

| Optional Probe Ceiling | Scoring | Brotli | Actual Probes | Renders | Optional Logical Work |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2 | Immediate | 162 | 2 | 3 | 6178 |
| 2 | Staged | 161 | 2 | 9 | 31180 |
| 8 | Immediate | 162 | 8 | 9 | 34188 |
| 8 | Staged | 151 | 8 | 16 | 67053 |

At ceilings 24/48, both schedules reach 143 with 17 optional probes and 18
renders. All-codec requests reach all three finite minima at ceiling 24; a
ceiling of 48 permits 34 actual probes without improving these winners.
Every width-one row remains at 162 Brotli even when its frontier drains with
unused probe allowance. More probes cannot recover its already evicted branches.

Delayed scoring buys quality at tight codec limits here by forming more
structures before scoring. It also spends more rendering/work/memory. This
does not justify delaying everything or increasing default effort, especially
for the separately measured emission-heavy Marked workload. Logical work is a
configured tariff, not CPU/wall time. `*_work` kind totals include baseline and
optional work; `optional_work` is the optional-domain total. Neither totals nor
the debug test duration isolate real phase costs.

## Verification

The first [qualifier receipt](../2026-09-19-artifact-service/run-2026-09-20T01-26-08.310Z/receipt.json)
correctly fails a zero-match filter after building the tests. The corrected
[four-test receipt](../2026-09-19-artifact-service/run-2026-09-20T01-27-16.430Z/receipt.json)
passes using the identical cached binary; Cargo reports `fresh=true`. Source
does not change between attempts. The three existing fairness/valley tests and
the new matrix test all pass. Initial build time is 41.356 seconds; the cached
check is 0.405 seconds. No library or release build runs.

The same preserved binary then runs only the new test with captured diagnostics.
[Receipt](receipt.json), SHA-256
`4a76cebf8ec49149f9c9ae5fffa49cb6a4224783da121ce1ca40680baaae0831`,
records 32 rows, 24 oracle artifacts, 64 independently executed winner
observations and separate canonical codec replay of every unique artifact.
Full rows and byte streams are retained in [rows.json](rows.json) and
[oracle.json](oracle.json); unrequested winners are absent, not scored as zero.

The [root audit](audit.json), SHA-256
`d52e6a81c56c6aa5f4a67305bd858bfcbf9f473e3993809b3868f54872276d2a`,
verifies all 980 current input pins, both qualifier receipts, all 30 diagnostic
outputs, complete matrix coverage and the matched observations/scores. Only
`src/semantic_program/search_fairness_tests.rs` differs from the C8 prerequisite.
Input digest is `77f4458c7b69666bade4bb744498d92efaefbc0f854d043bff94b91f56bad0a8`.
The 416,036,688-byte debug binary is preserved at
`/tmp/lilscript-search-schedule-baseline-20260920/lilscript-c737e884cfca95f9`,
SHA-256 `16e3d817b7af00089ef92992ae7b18493a9d7f254788b00b7f07db8a478bcc98`.
Original test source remains under `/tmp/lilscript-search-schedule-before-20260920/`.
No independent agent review, general optimality, speed or fleet claim follows.

## Architectural Boundary

Current inventory and proof seeds belong to one immutable semantic snapshot.
`combine_javascript` requires that exact snapshot; a checked equivalent rewrite
creates a new one. Dependency-directed rediscovery must carry revision-qualified
inventories/proofs and validate existing choices through the same scheduler and
edit owner. Rescanning source and combining old seeds would be unsound. This
ablation neither implements rediscovery nor adds a second search coordinator.
The [single migration plan](../../../docs/migration/index.md) owns that work.
