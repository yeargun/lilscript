# Generic corpus ratchet

Compiler `27c8ac7f89370238`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 120 | 624 | 52661 | 60614 |
| cases | raw | old | 520 | 624 | 52661 | 45935 |
| cases | gzip9 | competitor | 119 | 624 | 49907 | 56410 |
| cases | gzip9 | old | 502 | 622 | 49733 | 46478 |
| cases | brotli11 | competitor | 175 | 624 | 40023 | 44919 |
| cases | brotli11 | old | 520 | 623 | 39974 | 34677 |
| apps | raw | competitor | 0 | 7 | 835 | 1527 |
| apps | raw | closure | 1 | 7 | 835 | 1332 |
| apps | raw | old | 5 | 7 | 835 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 727 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 727 | 982 |
| apps | gzip9 | old | 5 | 7 | 727 | 711 |
| apps | brotli11 | competitor | 0 | 7 | 574 | 870 |
| apps | brotli11 | closure | 2 | 7 | 574 | 834 |
| apps | brotli11 | old | 5 | 7 | 574 | 558 |
| algorithms | raw | competitor | 0 | 11 | 4938 | 6210 |
| algorithms | raw | closure | 10 | 11 | 4938 | 4522 |
| algorithms | raw | old | 11 | 11 | 4938 | 3995 |
| algorithms | gzip9 | competitor | 0 | 11 | 3134 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3134 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3134 | 2662 |
| algorithms | brotli11 | competitor | 3 | 11 | 2856 | 3039 |
| algorithms | brotli11 | closure | 9 | 11 | 2856 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2856 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **fail**.

## Blocking

- loss count grew: cases raw losses to old: 518 -> 520
- loss count grew: cases gzip9 losses to old: 500 -> 502
- loss count grew: algorithms brotli11 losses to closure: 8 -> 9
- loss grew: cases/catalog/function/factorial-8 raw vs old: +0 -> +1 (ours 47, bar 46)
- loss grew: cases/catalog/function/factorial-8 gzip9 vs old: +0 -> +1 (ours 67, bar 66)
- loss grew: cases/catalog/function/factorial-8 brotli11 vs old: +1 -> +2 (ours 50, bar 48)
- loss grew: cases/catalog/function/factorial-9 raw vs old: +0 -> +1 (ours 47, bar 46)
- loss grew: cases/catalog/function/factorial-9 gzip9 vs old: +0 -> +1 (ours 67, bar 66)
- loss grew: cases/catalog/function/factorial-9 brotli11 vs old: +1 -> +2 (ours 50, bar 48)
- loss grew: cases/catalog/function/factorial-10 raw vs old: +0 -> +1 (ours 48, bar 47)
- loss grew: cases/catalog/function/factorial-10 gzip9 vs old: +0 -> +1 (ours 68, bar 67)
- loss grew: cases/catalog/function/factorial-10 brotli11 vs old: +2 -> +4 (ours 52, bar 48)
- loss grew: cases/catalog/collection/pipeline-3-1-4-1-5-9 gzip9 vs old: +8 -> +9 (ours 121, bar 112)
- loss grew: cases/catalog/collection/pipeline-2-7-1-8-2-8 brotli11 vs competitor: +0 -> +4 (ours 103, bar 99)
- loss grew: cases/catalog/collection/pipeline-2-7-1-8-2-8 brotli11 vs old: +13 -> +18 (ours 103, bar 85)
- loss grew: cases/catalog/collection/pipeline-9-8-7-6-5-4-3-2 gzip9 vs old: +6 -> +7 (ours 124, bar 117)
- loss grew: cases/catalog/collection/pipeline-5-5-5-5-5-5 gzip9 vs old: +8 -> +10 (ours 116, bar 106)
- loss grew: cases/catalog/host/host-hasown-a-b-c brotli11 vs competitor: +1 -> +8 (ours 96, bar 88)
- loss grew: cases/catalog/host/host-hasown-a-b-c brotli11 vs old: +13 -> +20 (ours 96, bar 76)
- loss grew: cases/catalog/control/edge-short-circuit-0-2-4 brotli11 vs old: +32 -> +33 (ours 67, bar 34)
- loss grew: cases/catalog/effect/edge-async-task-1n2 brotli11 vs competitor: +4 -> +5 (ours 69, bar 64)
- loss grew: cases/catalog/effect/edge-async-task-1n2 brotli11 vs old: +6 -> +7 (ours 69, bar 62)
- loss grew: cases/catalog/host/host-callable-fn brotli11 vs competitor: +7 -> +9 (ours 88, bar 79)
- loss grew: cases/catalog/host/host-callable-fn brotli11 vs old: +6 -> +8 (ours 88, bar 80)
- loss grew: cases/catalog/host/host-callable-as-value brotli11 vs competitor: +0 -> +2 (ours 88, bar 86)
- loss grew: cases/catalog/host/host-callable-as-value brotli11 vs old: +6 -> +8 (ours 88, bar 80)
- loss grew: cases/catalog/host/js-and-member-missing brotli11 vs competitor: +6 -> +7 (ours 70, bar 63)
- loss grew: cases/catalog/host/js-and-member-missing brotli11 vs old: +9 -> +10 (ours 70, bar 60)
- loss grew: cases/catalog/boolean/bool-typeof-predicate raw vs competitor: +11 -> +17 (ours 219, bar 202)
- loss grew: cases/catalog/boolean/bool-typeof-predicate raw vs old: +58 -> +64 (ours 219, bar 155)
- loss grew: cases/catalog/boolean/bool-typeof-predicate gzip9 vs old: +1 -> +4 (ours 121, bar 117)
- loss grew: cases/catalog/boolean/bool-typeof-predicate brotli11 vs competitor: +0 -> +1 (ours 98, bar 97)
- loss grew: cases/catalog/boolean/bool-typeof-predicate brotli11 vs old: +6 -> +14 (ours 98, bar 84)
- loss grew: cases/catalog/host/js-array-or-null raw vs competitor: +16 -> +22 (ours 187, bar 165)
- loss grew: cases/catalog/host/js-array-or-null raw vs old: +58 -> +64 (ours 187, bar 123)
- loss grew: cases/catalog/host/js-array-or-null gzip9 vs old: +2 -> +5 (ours 95, bar 90)
- loss grew: cases/catalog/host/js-array-or-null brotli11 vs old: +6 -> +9 (ours 81, bar 72)
- loss grew: cases/catalog/nullish/semantic-nullish-lazy-assignment gzip9 vs competitor: +0 -> +1 (ours 103, bar 102)
- loss grew: cases/catalog/nullish/semantic-nullish-lazy-assignment gzip9 vs old: +50 -> +51 (ours 103, bar 52)
- loss grew: cases/catalog/collection/semantic-array-nan-membership brotli11 vs old: +5 -> +7 (ours 66, bar 59)
- loss grew: algorithms/aggregate-ledger gzip9 vs closure: +5 -> +6 (ours 188, bar 182)
- loss grew: algorithms/aggregate-ledger gzip9 vs old: +46 -> +47 (ours 188, bar 141)
- loss grew: algorithms/helper-sharing raw vs closure: +2 -> +10 (ours 218, bar 208)
- loss grew: algorithms/helper-sharing raw vs old: +44 -> +52 (ours 218, bar 166)
- loss grew: algorithms/helper-sharing gzip9 vs closure: +3 -> +7 (ours 177, bar 170)
- loss grew: algorithms/helper-sharing gzip9 vs old: +31 -> +35 (ours 177, bar 142)
- loss grew: algorithms/helper-sharing brotli11 vs closure: +6 -> +9 (ours 158, bar 149)
- loss grew: algorithms/helper-sharing brotli11 vs old: +38 -> +41 (ours 158, bar 117)
- loss grew: algorithms/policy-specialization raw vs closure: +26 -> +36 (ours 326, bar 290)
- loss grew: algorithms/policy-specialization raw vs old: +42 -> +52 (ours 326, bar 274)
- loss grew: algorithms/policy-specialization gzip9 vs closure: +13 -> +19 (ours 223, bar 204)
- loss grew: algorithms/policy-specialization gzip9 vs old: +24 -> +30 (ours 223, bar 193)
- loss grew: algorithms/policy-specialization brotli11 vs closure: +21 -> +24 (ours 203, bar 179)
- loss grew: algorithms/policy-specialization brotli11 vs old: +27 -> +30 (ours 203, bar 173)
- loss grew: algorithms/state-machine-parser brotli11 vs closure: +0 -> +7 (ours 251, bar 244)
- loss grew: algorithms/state-machine-parser brotli11 vs old: +40 -> +47 (ours 251, bar 204)

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases raw losses to competitor: 121 -> 120
- loss count fell: cases brotli11 losses to competitor: 178 -> 175
- loss count fell: cases brotli11 losses to old: 521 -> 520
- loss fell: cases/canonical/control/if-chain raw vs old: +13 -> +1
- loss fell: cases/canonical/control/if-chain gzip9 vs old: +13 -> +1
- loss fell: cases/canonical/control/if-chain brotli11 vs old: +13 -> +1
- loss fell: cases/canonical/identity/snapshot-captured-rebind brotli11 vs old: +12 -> +10
- loss fell: cases/canonical/identity/snapshot-rebind brotli11 vs old: +55 -> +53
- loss fell: cases/catalog/collection/pipeline-10-20-30-40 gzip9 vs old: +7 -> +6
- loss fell: cases/catalog/collection/pipeline-5-5-5-5-5-5 brotli11 vs competitor: +3 -> +1
- loss fell: cases/catalog/collection/pipeline-5-5-5-5-5-5 brotli11 vs old: +14 -> +12
- loss fell: cases/catalog/collection/minmax-scan-0-1-0-2-0-3-0-4 brotli11 vs old: +13 -> +12
- loss fell: cases/catalog/collection/minmax-scan-10-20-30-40 brotli11 vs old: +15 -> +13
- loss fell: cases/catalog/control/edge-short-circuit-0-2-4 raw vs old: +56 -> +48
- loss fell: cases/catalog/control/edge-short-circuit-0-2-4 gzip9 vs old: +52 -> +44
- loss fell: cases/catalog/control/edge-short-circuit-5-7-9 raw vs old: +55 -> +47
- loss fell: cases/catalog/control/edge-short-circuit-5-7-9 gzip9 vs old: +52 -> +43
- loss fell: cases/catalog/control/edge-short-circuit-6-8-10 raw vs old: +56 -> +47
- loss fell: cases/catalog/control/edge-short-circuit-6-8-10 gzip9 vs old: +53 -> +43
- loss fell: cases/catalog/collection/edge-array-concat gzip9 vs old: +16 -> +15
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 raw vs competitor: +100 -> +17
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 raw vs old: +117 -> +34
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 gzip9 vs competitor: +52 -> +16
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 gzip9 vs old: +69 -> +33
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 brotli11 vs old: +63 -> +24
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 raw vs competitor: +100 -> +17
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 raw vs old: +117 -> +34
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 gzip9 vs competitor: +52 -> +16
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 gzip9 vs old: +69 -> +33
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 brotli11 vs old: +63 -> +24
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 raw vs competitor: +100 -> +17
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 raw vs old: +117 -> +34
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 gzip9 vs competitor: +52 -> +16
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 gzip9 vs old: +69 -> +33
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 brotli11 vs old: +63 -> +24
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 raw vs competitor: +100 -> +17
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 raw vs old: +117 -> +34
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 gzip9 vs competitor: +52 -> +16
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 gzip9 vs old: +69 -> +33
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 brotli11 vs old: +66 -> +26
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 raw vs competitor: +100 -> +17
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 raw vs old: +117 -> +34
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 gzip9 vs competitor: +52 -> +16
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 gzip9 vs old: +69 -> +33
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 brotli11 vs old: +66 -> +26
- loss fell: cases/catalog/integer/semantic-integer-update-values raw vs old: +65 -> +1
- loss fell: cases/catalog/collection/frontier-array-spread-order raw vs competitor: +15 -> +3
- loss fell: cases/catalog/collection/frontier-array-spread-order raw vs old: +21 -> +9
- loss fell: cases/catalog/collection/frontier-array-spread-order gzip9 vs competitor: +13 -> +3
- loss fell: cases/catalog/collection/frontier-array-spread-order gzip9 vs old: +17 -> +7
- loss fell: cases/catalog/collection/frontier-array-spread-order brotli11 vs competitor: +20 -> +12
- loss fell: cases/catalog/collection/frontier-array-spread-order brotli11 vs old: +27 -> +19
- loss fell: algorithms/aggregate-ledger brotli11 vs closure: +11 -> +10
- loss fell: algorithms/aggregate-ledger brotli11 vs old: +53 -> +52
- loss fell: algorithms/shape-invoice-pipeline gzip9 vs closure: +12 -> +10
- loss fell: algorithms/shape-invoice-pipeline gzip9 vs old: +94 -> +92
- loss fell: cases/catalog/integer/semantic-integer-update-values raw vs competitor: +48 -> +0
- loss fell: cases/catalog/integer/semantic-integer-update-values gzip9 vs competitor: +30 -> +0
- loss fell: cases/catalog/integer/semantic-integer-update-values gzip9 vs old: +50 -> +0
- loss fell: cases/catalog/integer/semantic-integer-update-values brotli11 vs competitor: +23 -> +0
- loss fell: cases/catalog/integer/semantic-integer-update-values brotli11 vs old: +40 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 brotli11 vs competitor: +40 -> +0
- loss fell: cases/catalog/union/semantic-union-string-int-flow raw vs old: +1 -> +0

