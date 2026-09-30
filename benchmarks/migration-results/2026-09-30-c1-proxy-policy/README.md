# C1: explicit proxy pruning and audit policy

Source `632614da`. Compiler `~/lilscript-work/bin/c1-proxy-policy-1/lilscript` SHA-256 `475f66d876a10a8335c3c0ea1a528d0aeccc38e0b68ec1584943c6e735ded254`; codec SHA-256 `1605ba6a5d786bea2e5b99f416cb914c8577b6d239fe227d07354e002ecea9c6`.

`[policy.search] proxy_pruning = "on" | "audit" | "off"` now controls terminal proxy rejection. The default preserves current pruning. Audit records exact rejected scores without selecting from them; off lets every reached nonidentical move reach exact admission and judging. Effort and hard limits remain in force. A larger restart can lead to a smaller result after refinement, so even exact-proxy rejection and no pruning can follow different trajectories. The focused fixture demonstrates a one-byte gzip improvement with no pruning; this alone is not a calibration claim.

The resolved mode enters the policy fingerprint and search-schedule version 27. `--proxy-pruning` overrides TOML; the deprecated environment adapter is CLI-only and explicit CLI wins over it. The library ignores that variable. Audit exact codec probes are counted, and their work remains on the hard ledger. Negative audit deltas are potential misses, subject to admission. Existing G2 retirement diagnostics now point to the actual private-field control. The generated schema includes search and resource leaf tables.

All 1,735 active library tests and nine CLI tests pass; four existing library ignores and the previously qualified expensive table exclusion remain. Subprocess tests compare deterministic public-API output, policy, search and resources with the retired environment variable present/absent. Execution oracles exercise all three modes under every objective. Configuration round trips, invalid values, fingerprints and CLI/TOML/adapter precedence pass.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under each objective and preserves every artifact from the isolated S2 initialization pin. All 20 [port artifact records](ports.json) are identical, with 29/29, 1,353/1,353 and 21/21 reference tests passing. No baseline update is needed.

[Three alternating CPU pairs](compile-cost.json) compare the combined S2 initialization proof and C1 controls against qualified S3, using frozen effort 13 and equal effective resource ceilings. The explicit policy-change reason accounts for the new fingerprinted control; contracts, objectives and effort match.

| Workload | Previous median CPU | New median CPU | Ratio | Exact judgments | Brotli bytes |
|---|---:|---:|---:|---:|---:|
| markedlil | 5.766 s | 5.618 s | 0.974× | 68 → 68 | 8,851 → 8,851 |
| zodlil | 18.699 s | 20.142 s | 1.077× | 67 → 67 | 26,573 → 26,573 |
| posthoglil | 0.987 s | 1.050 s | 1.063× | 25 → 25 | 5,224 → 5,224 |

All ranges overlap: Marked 5.613–5.907 versus 5.500–6.591 seconds; Zod 18.620–21.682 versus 19.363–20.919; PostHog 0.968–0.994 versus 0.957–1.070. No speed change is established. Samples and host load remain in the report. Artifact hashes, baseline/optional/codec work and peak accounted retained bytes match exactly on all three workloads. C3 calibration and remaining C1 compatibility/risk coverage stay open.
