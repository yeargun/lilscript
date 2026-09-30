# G2: private property naming with explicit preservation

Source `7b97a107`, policy algorithm 7 / walk 6. Compiler `~/lilscript-work/bin/g2-private-fields-2/lilscript` SHA-256 `71b08d86a7d9847610ad9da69a2acb71bbc21e74f97b54a6716847dbd5fafd9d`; codec SHA-256 `39659f971cb40be6180901d8ff1c1a591da28e86c99c83749f0f4c6e70472d1b`.

Private fields have one checked identity-based assignment through construction, reads, writes and inherited aliases. Unrelated private class families reuse names. Reflection includes subclasses, reachable field/method/constructor types, contextual erasure, host views and explicit/inferred generic arguments. Reflected, external, generic and observed class families keep their layouts. Explicit `mangle.preserve_properties` keeps requested keys and excludes those spellings from generated names, while other eligible fields can still rename.

At default/higher effort, a final trial preserves the completed prior winner and lets exact objective judging select field naming plus its subsequent refinement. Property naming is independent of lexical naming. Permission, target-compaction and rule checks apply before direct formation, cached heads and choice surveys. Four bounded heads represent both field-name and integer-hint choices.

The [generic gate](ratchet.md), with [full receipt](ratchet.json), passes all 642 programs under all three objectives with zero failures and no individual growth. Against S4 record spread:

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Cases | −386 | −165 | −119 |
| Applications | −24 | −11 | −4 |
| Algorithms | 0 | 0 | 0 |
| Total | **−410** | **−176** | **−123** |

There are 23 raw improvements, 23 gzip improvements and 19 Brotli improvements. The supported baseline update accepts this result. Every generic cell and all port artifacts match the [initial measurement](../2026-09-30-g2-private-fields-initial/README.md); that earlier compiler was not accepted because it lacked explicit property preservation.

The [reference port suites](ports.json) pass 29/29, 1,353/1,353 and 21/21 tests. At their shipped effort, markedlil's compiler-written raw objective saves 1,892 raw bytes, its gzip objective saves 267 gzip bytes, and its Brotli objective saves 307 Brotli bytes. Its ESM facade saves 366 Brotli bytes. Zod's Brotli core saves 69 Brotli bytes while growing 154 raw and 6 gzip bytes: those latter coordinates are not separately optimized builds. Its CJS facade grows 658 raw bytes and saves 22 Brotli bytes. All posthog artifacts are unchanged. The receipt includes every file; facade changes are not counted as additional independent compiler wins.

[Three alternating CPU pairs](compile-cost.json) on frozen effort-13 workloads retain all samples, exact artifacts, effective resource limits and runner/manifest identities:

| Workload | Previous median CPU | New median CPU | Ratio | Exact terminal judgments | Brotli bytes |
|---|---:|---:|---:|---:|---:|
| markedlil | 3.884 s | 5.438 s | 1.400× | 49 → 68 | 9,158 → 8,851 |
| zodlil | 7.421 s | 19.139 s | 2.579× | 28 → 67 | 26,642 → 26,573 |
| posthoglil | 0.915 s | 0.926 s | 1.012× | 25 → 25 | 5,224 → 5,224 |

The first two costs are substantial and their sample ranges do not overlap. Posthog ranges overlap. Earlier five-pair measurements remain in the initial report. The extra refinement buys bytes at a real compilation cost; it is not a speed improvement. Target-rule work and canonical encoding dominate the new zod cost, so Q2 continues with rule-level attribution and reuse. `property-mangling='off'` vetoes this opportunity; fast effort tiers do not add it.

Verification combines the complete library suite with focused correction runs: all active G2 tests pass, including reserved keys/collisions, all cached head combinations, poisoned prototypes, inherited aliases, public metadata and erased host crossings. Four existing library ignores remain; the previously qualified expensive data-table case was excluded. A flaky independent-dynamic-import ordering assertion was corrected to require both callbacks exactly once. No runtime-speed or runtime-parity claim is made. Broader generic/observed-layout eligibility, ordering and held-out calibration remain owned by G2/C3; this is the first qualified field family.
