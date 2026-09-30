# Q3: higher-effort deferred naming and combined refinements

Source `79c1d607`. Compiler `~/lilscript-work/bin/q3-level14-coupled-1/lilscript` SHA-256 `6a80e9323aa2c75a090149ed650c886d362ea70c0bb2b1e57d452f120ee775e9`; codec SHA-256 `61587b7e0773fc167cecc5ece0b9119a797d14c5a76e71f0edb4a6a89eec711b`.

`policy.search.deferred_naming_starts` now defaults to `"auto"`: off at default effort 13, enabled from 14. Explicit `"on"` permits it at 13, `"off"` vetoes it, and old booleans remain canonical on/off aliases. Fast tiers, prerequisite vetoes and no-prune mode keep their existing behavior. The receipt records the requested mode and effective permission; search/walk versions advance to 29/8.

When the tail runs, `deferred_naming_polish = true` combines its ordinary walks with the existing local naming and private-field refinements. False retains the earlier deferred walks. The completed ordinary winner is protected; every kept move is judged exactly and resource limits remain in force. The configuration guide now documents situations, defaults/gates, dependencies, overrides and size/compilation/runtime tradeoffs for each search control and tactic.

All 1,743 active library tests pass, with four existing ignores and the previously qualified expensive table case excluded for this source pin. Public execution tests check mutable inherited/private state, every objective, boolean/string aliases, actual default-13 output/work equality against off, higher-level equality against on, vetoes and exhausted tails. Receipt tests retain complete score/count replay while recognizing the new explicit refinement phase.

## Generic development corpus

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes 642 programs under each objective at the ordinary effort 15. Only `cases/catalog/closure/edge-closure-mutation-3nn1n5` changes, saving one gzip byte (110 → 109); no individual cell grows. All other full artifact hashes are identical to S1. The following Q2 pin preserves these outputs and accepts that cell through the supported baseline update.

| Selected objective | Previous total | New total | Delta |
|---|---:|---:|---:|
| Raw | 58,999 | 58,999 | +0 |
| Gzip | 54,032 | 54,031 | -1 |
| Brotli | 43,732 | 43,732 | +0 |

## Reference ports and packaging limits

All [reference suites](ports.json) pass: 29/29, 1,353/1,353 and 21/21. Independently selected core outputs at shipped effort 15 are:

| Port | Selected objective | Previous | New | Delta |
|---|---|---:|---:|---:|
| markedlil | raw | 32,700 | 32,700 | +0 |
| markedlil | gzip | 9,859 | 9,858 | -1 |
| markedlil | Brotli | 8,851 | 8,829 | -22 |
| zodlil | Brotli | 26,573 | 26,562 | -11 |
| posthoglil | raw | 14,667 | 14,667 | +0 |
| posthoglil | gzip | 5,518 | 5,518 | +0 |
| posthoglil | Brotli | 4,940 | 4,940 | +0 |

The historical `.raw.js` filename denotes a Brotli-targeted core in these ports; `.bytes.js` is the independent raw objective. Secondary raw/gzip coordinates of a Brotli winner are not separate objective wins.

Packaging can reverse the core ranking. Marked's ESM facade grows eight Brotli bytes (8,903 → 8,911), while its CJS and UMD facades each save six. Zod's CJS facade grows 49 Brotli bytes (32,802 → 32,851), 105 gzip bytes and 505 raw bytes despite the smaller core. PostHog's artifacts are unchanged. These are recorded limitations of the external facade builds, not hidden wins; final-format judging and consumer integration remain D1/D3 work. Facade variants are not summed as independent fleet savings.

## Default-effort cost

[Three alternating CPU pairs](compile-cost-13.json) per frozen port/objective compare the pre-deferred C1 pin (`632614da`) with this source at effort 13. Every output hash, full search record, baseline/optional/codec work count and peak accounted byte count is exactly identical. This isolates restoration of the established default search; the intervening primitive assumption change permits the same folds for these pristine-builtin inputs.

| Frozen workload | Previous median CPU | New median CPU | Ratio |
|---|---:|---:|---:|
| markedlil-raw | 2.796 s | 2.784 s | 0.996× |
| markedlil-gzip | 2.355 s | 2.441 s | 1.036× |
| markedlil-brotli | 7.571 s | 6.264 s | 0.827× |
| zodlil-raw | 8.599 s | 8.668 s | 1.008× |
| zodlil-gzip | 9.428 s | 10.828 s | 1.148× |
| zodlil-brotli | 18.501 s | 18.077 s | 0.977× |
| posthoglil-raw | 0.397 s | 0.424 s | 1.068× |
| posthoglil-gzip | 0.662 s | 0.659 s | 0.996× |
| posthoglil-brotli | 0.937 s | 0.913 s | 0.974× |

Every sample range overlaps; host load and all samples remain in the report. No speed improvement is claimed. The higher-effort combined tail is measured separately against the preceding deferred-tail pin with matching level-14 configurations.

Logs: `/tmp/lilscript-q3-level14-{focused,focused2,full,full2,release,ratchet,ports,cpu13}.log`. These are development/reference workloads, not protected held-out qualification or a global-optimum claim.

## Higher-effort combined search cost

[Three alternating pairs at effort 14](compile-cost-14.json) compare S1's already enabled deferred tail with the combined refinement under the same frozen Brotli configurations. This separates the cost of combining choices from the larger earlier cost of adding deferred starts. These frozen inputs differ from the shipped port builds.

| Frozen workload | Selected Brotli bytes | Median CPU | Ratio | Exact probes |
|---|---:|---:|---:|---:|
| markedlil | 8,851 → 8,829 | 11.281 s → 13.724 s | 1.217× | 152 → 194 |
| zodlil | 26,573 → 26,562 | 37.542 s → 48.193 s | 1.284× | 144 → 188 |
| posthoglil | 5,200 → 5,200 | 3.211 s → 3.250 s | 1.012× | 97 → 99 |

Marked and Zod have clearly separated timing ranges; PostHog's ranges only narrowly separate. The additional optional work is 79,955,765 / 366,835,702 / 1,565,152 units. Peak accounted storage increases by 1,658,910 bytes for Marked and 3,145,417 for Zod, and is unchanged for PostHog. This is a measured size/compilation tradeoff, with no new runtime-risk permission; it does not establish runtime-performance parity. Raw/gzip have focused effort-14 execution coverage and generic/port size gates at effort 15; there is no separate paired effort-14 CPU claim for those objectives.

The [comparison receipt](comparison.json), generated by [the verifier](summarize.py), checks full generic hashes, port coordinates and the default-13 search/resource equality. The retained CPU runner checks matching contract, effort, codec settings, source graph and effective resource ceilings. Logs: `/tmp/lilscript-q3-level14-cpu14.log`, `/tmp/lilscript-q3-level14-summary.log`.
