# G1: local frequency naming after the existing search

Source `893e95bf`, policy algorithm 6 / walk schedule 5. Compiler `~/lilscript-work/bin/g1-local-polish-1/lilscript` SHA-256 `0e53dce81d3cb28d3e00d698747b9777fa689d87f1817a94e19844b598d75acd`; codec SHA-256 `1c1b7cc876d9427f5ad5c09f7ce47af5e26104a9da5d2fae4bffb53c650f77fd`.

Default and higher effort first complete the prior search, then try local read-frequency order from its selected winner. Accepted local naming can be refined by the existing moves; the earlier result remains available. Fast effort tiers retain their existing schedule. Identifier-mangling and naming-search vetoes apply before formation. This replaces the [rejected interleaved schedule](../2026-09-30-g1-local/README.md).

The [generic gate](ratchet.md), with [complete receipt](ratchet.json), passes all previously supported cells and retains the same 33 refused lanes. Relative to the accepted S4 nullish baseline:

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Cases | 0 | −2 | −15 |
| Applications | 0 | 0 | 0 |
| Algorithms | 0 | −4 | −6 |
| Total | 0 | −6 | −21 |

No individual size grows. The supported `--update-baseline` run accepts this result. The [port suites](ports.json) pass 29/29, 1,353/1,353 and 21/21; all 20 reported files match the previous qualified compiler byte for byte.

[Three paired compilation runs](compile-cost.json) report median CPU ratios of 1.014 on markedlil, 1.030 on zodlil and 1.322 on posthoglil. The posthog samples have unusually wide spread: candidate 0.949–1.286 seconds, previous 0.887–1.014. A retained [seven-pair repeat](compile-cost-posthog-repeat.json), using the same frozen posthog workload, reports median 0.910 → 0.957 seconds, ratio **1.051**, with ranges 0.894–0.945 and 0.931–1.010. Both reports remain available. All three selected programs are byte-identical; terminal exact judgments are 49 → 49, 28 → 28 and 24 → 25. This is a small generic compression gain with additional compilation work, not a speed improvement or a claim that the new naming wins on these ports. Repeated formation remains Q2 work.

Verification: 1,702 active library tests passed across the full run and focused correction runs, including the large exact data-table case; four existing ignores remain. Tests cover capture hygiene, unchanged sibling scopes, public names and arity, permission vetoes, deterministic replay, retained-budget release, and preservation of the completed pre-polish winner under each objective.
