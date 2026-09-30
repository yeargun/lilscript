# S3: inline explicit strict bodies into proven strict contexts

Source `111f6f71`. Compiler `~/lilscript-work/bin/s3-strict-inline-1/lilscript` SHA-256 `cc4ec38d61bcbc128accd49ef476070a5096cbb520027fb42e71e3bae966ad0d`; codec SHA-256 `678b31f1d0e6950b6d81491c2fef5bedbb1d596ddc4367204864e5645378399f`.

The three existing target inliners now admit an explicit strict body when the call-site execution proof allows it. A strict body still cannot move into a sloppy destination. All argument, frame, effect, recursion and depth restrictions remain, as does the inlining permission. This exposes a general opportunity in imported struct-bearing helpers without adding a workload-specific heuristic.

All 1,728 active library tests pass, with four existing ignores and the previously qualified expensive table case excluded. Added execution oracles cover all three inliners, explicit/inherited/global strict callers, refused sloppy destinations, ordered argument effects and writes to frozen objects. Public imported-helper builds pass under all three objectives, script/module contracts and TOML inlining on/off.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under every objective with no individual growth. The supported baseline-update run accepted these results. Cases and apps are unchanged. Algorithm totals improve by **156 raw, 28 gzip and 30 Brotli bytes**: large-event-analytics saves 100/19/23, and shape-invoice-pipeline saves 56/9/7. Their raw gaps to the pinned Closure bar fall to 27 and 130 bytes. These inspected development programs are not held-out evidence or a general Closure-parity claim.

All 20 [port artifact records](ports.json) equal the preceding S1 scalar-method qualification. Reference suites pass 29/29, 1,353/1,353 and 21/21.

[Three alternating CPU pairs](compile-cost.json) retain complete samples and matched effective resources at frozen effort 13:

| Workload | Previous median CPU | New median CPU | Ratio | Exact judgments | Brotli bytes |
|---|---:|---:|---:|---:|---:|
| markedlil | 6.606 s | 6.273 s | 0.950× | 68 → 68 | 8,851 → 8,851 |
| zodlil | 21.337 s | 20.207 s | 0.947× | 67 → 67 | 26,573 → 26,573 |
| posthoglil | 0.981 s | 1.035 s | 1.055× | 25 → 25 | 5,224 → 5,224 |

Marked ranges are separated (6.569–6.671 versus 6.232–6.340 seconds). Zod's ranges are narrowly separated (20.313–22.318 versus 19.901–20.293). PostHog ranges overlap (0.953–1.095 versus 0.918–1.071). These small samples support limited measured improvements on the first two workloads, without a universal speed claim; host load is retained. All frozen workload artifact hashes and terminal exact-judgement counts are unchanged. More inlining, devirtualization, specialization and sharing remain S3 work.
