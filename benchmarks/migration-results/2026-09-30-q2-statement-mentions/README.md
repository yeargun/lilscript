# Q2: reuse exact statement mention facts

Source `8bc28c1f`. Compiler `~/lilscript-work/bin/q2-statement-mentions-1/lilscript` SHA-256 `a5da0201f944923ba9162c63942431d833178e5f9a38aa25776c050a4d6fa8cd`; codec SHA-256 `ab77ea668ad0e5c9568343f19fd9914e2f2bb547e4c6ee9fcf19468957b1d52e`.

The target's single-use forwarding rule used to traverse later statements repeatedly for different bindings. It now builds an exact mentioned-binding set lazily for each queried statement, including nested regions and created/declared function bodies. Forwarding invalidates the changed receiver and removes the deleted declaration's cache entry; the region discards all remaining sets before advancing. Candidate order, write checks, initialization/capture proofs and single-evaluation checks are unchanged.

The collector admits its stack and result storage before allocation. Its temporary scope transfers only the completed set; the region releases that set's charge after dropping it. Construction, sorting, lookup and row-shift work are counted. Policy algorithm 8 records the work/storage change. There is no new user flag for this internal analysis reuse.

All 1,746 ordinary active library tests pass, and the separate expensive exact data-table test passes in 261.52 seconds: 1,747 active tests total, with four existing ignores and no exclusion. Test builds compare every indexed lookup against the independent original scanner. Focused execution covers forty chained deletions and a moved closure that adds a capture to an already inspected receiver. Tight memory ceilings cover partial refusal, unchanged source and complete release. The table oracle executes every selected decoder and replays its exact score under all objectives.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes 642 programs under every objective. All 1,926 complete artifact hashes and selected sizes are identical to the preceding level-14/refinement pin. The supported baseline update accepts that preceding batch's one-byte gzip improvement; this cache adds no measured generic size change. All 20 [port artifacts](ports.json) are also identical, with reference suites passing 29/29, 1,353/1,353 and 21/21.

The [paired compilation report](compile-cost.json) uses the same frozen inputs and effort-13 settings for each port/objective, with three alternating pairs after warm-up. The [verifier](summarize.py) retains its checked [comparison](comparison.json), including full compiler/report hashes, sample ranges and phase observations. Every complete search receipt is identical, and the policies differ only by algorithm 7→8. All nine peak accounted-memory values and exact-codec work counts are unchanged.

| Workload | Previous CPU (s) | Cached CPU (s) | Ratio | Ranges overlap | Optional counted work |
|---|---:|---:|---:|---|---:|
| markedlil-raw | 2.185 | 1.982 | 0.907 | no | 158,003,856 → 169,850,214 |
| markedlil-gzip | 1.990 | 1.839 | 0.924 | no | 180,240,729 → 188,641,581 |
| markedlil-brotli | 5.286 | 5.042 | 0.954 | no | 176,731,817 → 189,553,414 |
| zodlil-raw | 7.326 | 6.278 | 0.857 | no | 547,700,077 → 573,146,142 |
| zodlil-gzip | 9.221 | 7.866 | 0.853 | no | 651,556,334 → 686,642,170 |
| zodlil-brotli | 17.613 | 16.401 | 0.931 | no | 623,709,269 → 656,110,685 |
| posthoglil-raw | 0.357 | 0.356 | 0.997 | yes | 27,326,093 → 28,234,148 |
| posthoglil-gzip | 0.526 | 0.527 | 1.000 | yes | 57,726,803 → 59,263,137 |
| posthoglil-brotli | 0.911 | 0.908 | 0.996 | no | 30,246,117 → 31,173,955 |

Marked CPU falls 4.6–9.3% and Zod 6.9–14.7%, with separated sample ranges in all six comparisons. PostHog changes by at most 0.4%; raw/gzip ranges overlap, while Brotli has a narrow separated gap. Treat that as effectively unchanged rather than a material performance result. Warm-up phase profiles locate the meaningful savings in `ForwardSingleUses`; they are supporting elapsed observations, not separately paired phase CPU samples.

Baseline counted work rises slightly and optional counted work rises despite lower CPU: the new cache explicitly charges construction, sorting, lookup and row shifts. Tight logical-work ceilings may therefore stop earlier; this batch does not promise identical search under every hard limit. The qualified workloads preserve all opportunities and artifacts without increasing peak accounted memory. No runtime-performance change is claimed. The configuration guide separately corrects an old overstatement about effort: within-run incumbent protection and fast-tier replay do not yet establish non-growing size between every effort level and structural frontier under hard limits.

Logs: `/tmp/lilscript-q2-mentions-{focused,full,tables,release,ratchet,ports,cpu,summary}.log`. These are development/reference workloads, not held-out qualification.
