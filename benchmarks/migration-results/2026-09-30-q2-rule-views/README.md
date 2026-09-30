# Q2: reuse views after unchanged rule commits

Source checkpoint `af5124fd`, compiler `~/lilscript-work/bin/q2-rule-views-1/lilscript` SHA-256 `e2f45f8ecbfe4a7de2d90716562f4053ca367824043c3a8f756ee542cbd4c544`; codec SHA-256 `9688c28a991d366630acd26a240365776561e54d12cd60ea726b86a888c13765`.

The semantic rule editor preserves effect/range views after a rule that changes no unit or table. Real edits invalidate them through revision stamps. Rule-phase views are dropped at finish before publication. This does not retain target trees or complete rule-phase allocation accounting.

All 44 rule tests pass, including explicit cold-versus-reused byte/receipt and execution comparisons, both sealing modes, each table/unit mutation owner and finish lifetime. The full library run passes 1,683 tests with nine existing ignores and the previously qualified expensive table case excluded. The [generic ratchet](ratchet.md), with [full receipt](ratchet.json), passes 642 inputs × three objectives with the same 36 ledgered failures. Every lane's state, size and output hash is identical to D1's explicitly bare lane.

[Three alternating measured compilation pairs](compile-cost.json), after warm-up on the [frozen manifest](../2026-09-30-s1-evaluation/compile-workloads.json), compare D1 against this isolated change:

| Entry | Median CPU ratio | Selected output |
|---|---:|---|
| markedlil | 0.999 | byte-identical |
| zodlil | 0.996 | byte-identical |
| posthoglil | 0.992 | byte-identical |

There is no demonstrated whole-build speed improvement above sample spread. Avoiding this repeated analysis is useful bookkeeping, but it does not resolve the larger target-formation and scoring costs. Subsequent scheduling changes are excluded from these measurements.
