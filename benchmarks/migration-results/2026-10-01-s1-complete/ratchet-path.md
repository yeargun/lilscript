# Generic corpus ratchet

Compiler `628028fe56197da6`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 121 | 624 | 52651 | 60614 |
| cases | raw | old | 517 | 624 | 52651 | 45935 |
| cases | gzip9 | competitor | 120 | 624 | 49839 | 56410 |
| cases | gzip9 | old | 500 | 622 | 49665 | 46478 |
| cases | brotli11 | competitor | 174 | 624 | 40034 | 44919 |
| cases | brotli11 | old | 521 | 623 | 39985 | 34677 |
| apps | raw | competitor | 0 | 7 | 835 | 1527 |
| apps | raw | closure | 1 | 7 | 835 | 1332 |
| apps | raw | old | 5 | 7 | 835 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 727 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 727 | 982 |
| apps | gzip9 | old | 5 | 7 | 727 | 711 |
| apps | brotli11 | competitor | 1 | 7 | 576 | 870 |
| apps | brotli11 | closure | 2 | 7 | 576 | 834 |
| apps | brotli11 | old | 5 | 7 | 576 | 558 |
| algorithms | raw | competitor | 0 | 11 | 4920 | 6210 |
| algorithms | raw | closure | 10 | 11 | 4920 | 4522 |
| algorithms | raw | old | 11 | 11 | 4920 | 3995 |
| algorithms | gzip9 | competitor | 0 | 11 | 3125 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3125 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3125 | 2662 |
| algorithms | brotli11 | competitor | 3 | 11 | 2844 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2844 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2844 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **fail**.

## Blocking

- loss count grew: cases gzip9 losses to competitor: 119 -> 120
- loss count grew: apps brotli11 losses to competitor: 0 -> 1
- loss grew: cases/catalog/collection/pipeline-5-5-5-5-5-5 gzip9 vs old: +8 -> +10 (ours 116, bar 106)
- loss grew: cases/catalog/control/edge-short-circuit-0-2-4 brotli11 vs old: +32 -> +33 (ours 67, bar 34)
- loss grew: cases/catalog/effect/edge-async-task-1n2 brotli11 vs competitor: +4 -> +5 (ours 69, bar 64)
- loss grew: cases/catalog/effect/edge-async-task-1n2 brotli11 vs old: +6 -> +7 (ours 69, bar 62)
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
- loss grew: cases/catalog/collection/frontier-array-spread-order brotli11 vs competitor: +20 -> +24 (ours 77, bar 53)
- loss grew: cases/catalog/collection/frontier-array-spread-order brotli11 vs old: +27 -> +31 (ours 77, bar 46)
- loss grew: apps/higher-order-pipeline brotli11 vs competitor: +0 -> +2 (ours 112, bar 110)
- loss grew: apps/higher-order-pipeline brotli11 vs old: +6 -> +8 (ours 112, bar 104)

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases raw losses to old: 518 -> 517
- loss count fell: cases brotli11 losses to competitor: 178 -> 174
- loss fell: cases/canonical/identity/snapshot-captured-rebind brotli11 vs old: +12 -> +10
- loss fell: cases/canonical/identity/snapshot-rebind brotli11 vs old: +55 -> +53
- loss fell: cases/catalog/collection/pipeline-2-7-1-8-2-8 gzip9 vs old: +9 -> +8
- loss fell: cases/catalog/collection/pipeline-5-5-5-5-5-5 brotli11 vs competitor: +3 -> +1
- loss fell: cases/catalog/collection/pipeline-5-5-5-5-5-5 brotli11 vs old: +14 -> +12
- loss fell: cases/catalog/collection/pipeline-1-1-2-3-5-8-13-21 gzip9 vs old: +7 -> +6
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
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-0-8 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-1-6 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-2-4 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-3-2 brotli11 vs competitor: +38 -> +0
- loss fell: cases/catalog/nullish/edge-nullish-optional-missing-4-0 brotli11 vs competitor: +40 -> +0
- loss fell: cases/catalog/union/semantic-union-string-int-flow raw vs old: +1 -> +0

