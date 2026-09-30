# Q2 target reach: negative first experiment

Source `00be51b34bc63e2c3c4eb89d24543f12bce850e9`; [binary identities](identity.json). This experiment is not qualified as a performance improvement. The broader generic/port size gates and the expensive table test were not run for this pin.

The implementation separates reachable-node/strictness facts from capture facts, admits backing storage and releases temporary facts after each existing transformation round. Four focused oracle/ownership/refusal tests, 1,758 ordinary active library tests and all 13 CLI tests pass. Four existing library ignores remain and the expensive table oracle was filtered. The schema consistency check passes.

The [complete CPU report](compile-cost.json) retains three alternating pairs after warm-up for each of nine frozen effort-13 workloads. Against the qualified objective-set pin, every workload preserves its complete output hash, complete search receipt and exact-codec work. Policies differ only by algorithm 10 to 11; input identities, contracts and effective resource ceilings match. Both compilers release all accounted storage after handoff. Algorithm 11 accounts for previously uncounted analysis storage and work; increased counts are not evidence of greater process memory or more optimization opportunities.

| Workload | Previous CPU (s) | Experiment CPU (s) | Ratio | Sample ranges overlap |
|---|---:|---:|---:|---|
| markedlil-raw | 1.968 | 2.238 | 1.137 | no |
| markedlil-gzip | 1.824 | 2.100 | 1.151 | no |
| markedlil-brotli | 5.871 | 6.562 | 1.118 | no |
| zodlil-raw | 6.486 | 7.375 | 1.137 | no |
| zodlil-gzip | 8.030 | 8.845 | 1.101 | no |
| zodlil-brotli | 17.024 | 17.589 | 1.033 | yes |
| posthoglil-raw | 0.394 | 0.423 | 1.073 | no |
| posthoglil-gzip | 0.576 | 0.632 | 1.096 | yes |
| posthoglil-brotli | 0.960 | 0.976 | 1.016 | yes |

All nine medians regress, with no output-size gain. In Marked's warm-up elapsed phase profiles, increased formation/rule time accompanies the regression; those observations are supporting profiles, not separately paired phase CPU measurements. Per-node admission and a separately grown reference list are candidates for correction. The next experiment reserves result capacity from node bounds and summarizes reference owners per binding while preserving the independent traversal oracle and resource accounting.

No baseline was changed. These are exposed development/reference workloads, not held-out qualification. Logs: `/tmp/lilscript-q2-reach-{focused2,full,cli,release,cpu}.log`.
