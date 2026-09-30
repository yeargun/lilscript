# C1: reconstruction permissions and ownership

Isolated source `8666ae59` contains the policy correction and allocation-owner fix, without S2 record changes. Pinned compiler `~/lilscript-work/bin/c1-risk-2/lilscript` SHA-256 `7b4a4c9eb6be55de46bef8f9ee5a8c129f17748ad02a64aab75eff69f0c839ef`; codec SHA-256 `834959a7b6a77a73f1c6996dc59f47c600c0c090455dd2ab16d22190a639ed66`.

Policy algorithm 2 enforces the existing reconstruction permissions in initial formation, terminal search and retained output. This corrects eligibility; it is not a compression optimization. Explicit `on` permits the existing alternatives to compete. Default level 13 no longer silently grants decoder work.

[Paired CPU samples](compile-cost.json) compare against G1, with three alternating pairs after warm-up. Frozen source/configuration identities and codec settings are retained in the report and [manifest](compile-workloads.json). Every configuration is copied into `configs/`.

| Entry/configuration | Median CPU ratio | Raw before → after | Gzip before → after | Brotli before → after |
|---|---:|---:|---:|---:|
| markedlil-default | 0.976 | 36,243 → 36,243 | 10,209 → 10,209 | 9,158 → 9,158 |
| markedlil-explicit | 1.023 | 36,243 → 36,243 | 10,209 → 10,209 | 9,158 → 9,158 |
| zodlil-default | 1.044 | 89,489 → 89,496 | 29,889 → 29,815 | 26,645 → 26,642 |
| zodlil-explicit | 0.991 | 89,489 → 89,489 | 29,889 → 29,889 | 26,645 → 26,645 |
| posthoglil-default | 0.364 | 16,738 → 16,936 | 5,805 → 5,833 | 5,185 → 5,224 |
| posthoglil-explicit | 0.995 | 16,738 → 16,738 | 5,805 → 5,805 | 5,185 → 5,185 |

Explicit permissions retain byte-identical outputs in all three ports. The posthog default removes an ineligible representation and many ensuing search trials; its lower CPU cost must not be attributed to faster implementation. These measurements do not establish runtime-speed parity.

The corrected [generic ratchet](ratchet.md), with [complete receipt](ratchet.json), passes 642 inputs × three objectives with the same 36 ledgered failures. Every passing lane retains its G1 size. The [maintained port suites](ports.json) pass markedlil 29/29, zodlil 1,353/1,353 and posthoglil 21/21, building all nine declared artifacts from the copied, already-migrated checkouts without applying historical patches.

## Rejected initial measurement

[The original samples](compile-cost-before-owner-fix.json) are retained as rejected evidence. A child allocation scope attempted to grow its parent’s arena during a zod packing trial. `InvalidRelease` was hidden as ordinary exhaustion, shortening search and increasing the explicitly permitted output by 16 Brotli bytes. The corrected compiler keeps arena growth with its owner and propagates accounting failures. The corrected zod explicit lane is again byte-identical; the apparent speedup was a bug.
