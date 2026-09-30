# G1: first local naming schedule, superseded before acceptance

This is the first experiment, source `10173cb2`, policy algorithm 5 / walk schedule 4. Compiler `~/lilscript-work/bin/g1-local-1/lilscript` SHA-256 `3421b4f2a2ca6d2000b40ed9d29d0537615ffd79937d2d81b52d962b67697b6f`; codec SHA-256 `9729d4db84b4ded46dd95c52f227c3cc8f6c4e8d696f429e51930eeecba12ae2`.

The trial reordered non-root scopes by local static read frequency and ran after alphabet trials in every existing pass. The focused, full library (1,700 passed, four existing ignores, one separately qualified exclusion) and six CLI tests pass. The [generic gate](ratchet.md), with [complete receipt](ratchet.json), reports:

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Cases | 0 | −3 | −12 |
| Applications | 0 | 0 | 0 |
| Algorithms | 0 | −2 | −6 |
| Total | 0 | −5 | −18 |

The strict ratchet fails because `collection-geometry` grows by two gzip bytes (234 → 236); the program's structure is unchanged and the names differ. The earlier search trajectory needs protection before adding another greedy naming move. No baseline update accepts this schedule. The revised implementation tries local ordering after the earlier walks finish and retains their selected result.

The [port suites](ports.json) pass 29/29, 1,353/1,353 and 21/21. All 20 reported files are unchanged. [Three paired compilation runs](compile-cost.json) likewise produce identical selected output; median CPU ratios are 1.016, 1.010 and 0.961. Terminal exact judgments are 49 → 49, 28 → 28 and 24 → 25. These measurements do not establish a general speed improvement. This receipt remains evidence for the superseded schedule, separate from the revised schedule's qualification.
