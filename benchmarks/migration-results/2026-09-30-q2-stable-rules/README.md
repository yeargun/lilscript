# Q2: skip stable rule applications

Source checkpoint `f0efa953`; pinned compiler `~/lilscript-work/bin/q2-stable-rules-1/lilscript` SHA-256 `1aecc45131612e0f0c5895fe7778f7b6ddfe91d09d7eaf696ff1a1ae89794536`, codec SHA-256 `9a512a21ebdbaf845ac4f467c1377769bcc0e6d9b8362d6e44edd848f4efbcc5`.

The scheduler skips a stable suffix until an actual edit invalidates it, and carries five context-independent head rules' stability into a fresh tail. Actual edits keep their order and round boundaries. Logical work drops, so policy algorithm 4 identifies the changed tariff. Under a finite work allowance, this can reach more existing candidates. This does not add a target-tree cache or complete incremental formation.

Dense-versus-reused tests compare rounds, edits, complete searched bytes, receipts and behavior under all objectives. The full library suite plus the corrected timing-bucket assertion pass 1,694 tests, with five then-existing ignores and the separately qualified expensive table case excluded. Timing adds `js_rule` at the end of the existing bucket order.

The [generic gate](ratchet.md), with [complete receipt](ratchet.json), passes 642 inputs × three objectives, retaining the same 33 ledgered failing lanes. Every prior passing lane's bytes and hash are unchanged. The [port suites](ports.json) pass markedlil 29/29, zodlil 1,353/1,353 and posthoglil 21/21. All 20 reported port files match D1's previous qualification by hash and size, including the nine compiler objective artifacts.

[Three alternating compilation pairs](compile-cost.json) compare the nullish-assignment checkpoint against this scheduler using the [frozen manifest](../2026-09-30-s1-evaluation/compile-workloads.json). Separate warm-up and phase telemetry are excluded from timed samples. The explicit policy-change reason covers algorithm 4; source contract, objective settings and effort remain matched.

| Workload | Previous CPU seconds, median [range] | Candidate CPU seconds, median [range] | Ratio | Exact terminal judgments |
|---|---:|---:|---:|---:|
| markedlil | 4.103 [4.067–4.113] | 3.841 [3.825–3.847] | 0.936 | 49 → 49 |
| zodlil | 8.007 [7.986–8.060] | 7.203 [7.197–7.215] | 0.900 | 28 → 28 |
| posthoglil | 0.938 [0.932–0.943] | 0.898 [0.879–0.920] | 0.956 | 24 → 24 |

Selected outputs are byte-identical in every pair. The separated sample ranges support lower compilation CPU on these workloads; this is not a universal speed claim or a resolution of all repeated formation/scoring work.
