# Generic corpus ratchet

Compiler `984b99192e63043c`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 117 | 624 | 52124 | 60614 |
| cases | raw | old | 514 | 624 | 52124 | 45935 |
| cases | gzip9 | competitor | 116 | 624 | 49470 | 56410 |
| cases | gzip9 | old | 497 | 622 | 49294 | 46478 |
| cases | brotli11 | competitor | 155 | 624 | 39634 | 44919 |
| cases | brotli11 | old | 524 | 623 | 39585 | 34677 |
| apps | raw | competitor | 0 | 7 | 811 | 1527 |
| apps | raw | closure | 1 | 7 | 811 | 1332 |
| apps | raw | old | 5 | 7 | 811 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 707 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 707 | 982 |
| apps | gzip9 | old | 5 | 7 | 707 | 711 |
| apps | brotli11 | competitor | 1 | 7 | 557 | 870 |
| apps | brotli11 | closure | 2 | 7 | 557 | 834 |
| apps | brotli11 | old | 5 | 7 | 557 | 558 |
| algorithms | raw | competitor | 0 | 11 | 4565 | 6210 |
| algorithms | raw | closure | 7 | 11 | 4565 | 4522 |
| algorithms | raw | old | 11 | 11 | 4565 | 3995 |
| algorithms | gzip9 | competitor | 0 | 11 | 2957 | 3440 |
| algorithms | gzip9 | closure | 3 | 11 | 2957 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 2957 | 2662 |
| algorithms | brotli11 | competitor | 0 | 11 | 2661 | 3039 |
| algorithms | brotli11 | closure | 4 | 11 | 2661 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2661 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **fail**.

Requested baseline growth acceptance: **eligible**. The ordinary verdict above remains visible.

| Corpus | Objective | Before | Now | Delta |
|---|---|---:|---:|---:|
| cases | raw | 52651 | 52124 | -527 |
| cases | gzip9 | 49839 | 49470 | -369 |
| cases | brotli11 | 40034 | 39634 | -400 |
| apps | raw | 835 | 811 | -24 |
| apps | gzip9 | 727 | 707 | -20 |
| apps | brotli11 | 576 | 557 | -19 |
| algorithms | raw | 4920 | 4565 | -355 |
| algorithms | gzip9 | 3125 | 2957 | -168 |
| algorithms | brotli11 | 2844 | 2661 | -183 |

## Blocking

- loss count grew: cases brotli11 losses to old: 521 -> 524
- loss grew: cases/catalog/aggregate/struct-pairs-7-14-21-28-35 brotli11 vs competitor: +2 -> +5 (ours 125, bar 120)
- loss grew: cases/catalog/aggregate/struct-pairs-7-14-21-28-35 brotli11 vs old: +36 -> +39 (ours 125, bar 86)
- loss grew: cases/catalog/aggregate/nested-struct-0-1-0-2-0-3-0-4 brotli11 vs old: +22 -> +28 (ours 151, bar 123)
- loss grew: cases/catalog/control/enum-int-dispatch-4 brotli11 vs old: +0 -> +3 (ours 91, bar 88)
- loss grew: cases/catalog/control/enum-int-dispatch-12 brotli11 vs old: +0 -> +3 (ours 91, bar 88)
- loss grew: cases/catalog/aggregate/matrix2-9-8-7-6-5-4-3-2 brotli11 vs old: +35 -> +36 (ours 129, bar 93)
- loss grew: cases/catalog/host/host-hasown-a-b-c brotli11 vs competitor: +1 -> +8 (ours 96, bar 88)
- loss grew: cases/catalog/host/host-hasown-a-b-c brotli11 vs old: +13 -> +20 (ours 96, bar 76)
- loss grew: cases/catalog/collection/minmax-scan-0-1-0-2-0-3-0-4 brotli11 vs old: +12 -> +13 (ours 105, bar 92)
- loss grew: cases/catalog/closure/edge-closure-mutation-20nn3n4 brotli11 vs competitor: +4 -> +5 (ours 93, bar 88)
- loss grew: cases/catalog/closure/edge-closure-mutation-20nn3n4 brotli11 vs old: +11 -> +12 (ours 93, bar 81)
- loss grew: cases/catalog/effect/edge-exception-finally-1-5-7 brotli11 vs old: +5 -> +6 (ours 72, bar 66)
- loss grew: cases/catalog/effect/edge-generator-range-3n12n3 gzip9 vs competitor: +10 -> +11 (ours 134, bar 123)
- loss grew: cases/catalog/effect/edge-generator-range-3n12n3 gzip9 vs old: +9 -> +10 (ours 134, bar 124)
- loss grew: cases/catalog/effect/edge-generator-range-7n11n1 brotli11 vs competitor: +10 -> +11 (ours 115, bar 104)
- loss grew: cases/catalog/effect/edge-generator-range-7n11n1 brotli11 vs old: +15 -> +16 (ours 115, bar 99)
- loss grew: cases/catalog/host/host-callable-undefined raw vs competitor: +101 -> +118 (ours 133, bar 15)
- loss grew: cases/catalog/host/host-callable-undefined raw vs old: +9 -> +26 (ours 133, bar 107)
- loss grew: cases/catalog/host/host-callable-undefined gzip9 vs competitor: +66 -> +68 (ours 103, bar 35)
- loss grew: cases/catalog/host/host-callable-undefined brotli11 vs competitor: +61 -> +63 (ours 82, bar 19)
- loss grew: cases/catalog/host/host-callable-undefined brotli11 vs old: +4 -> +6 (ours 82, bar 76)
- loss grew: cases/catalog/host/host-is-window-nullish raw vs competitor: +51 -> +62 (ours 77, bar 15)
- loss grew: cases/catalog/host/host-is-window-nullish raw vs old: +12 -> +23 (ours 77, bar 54)
- loss grew: cases/catalog/host/host-is-window-nullish brotli11 vs competitor: +36 -> +43 (ours 62, bar 19)
- loss grew: cases/catalog/host/host-is-window-nullish brotli11 vs old: +0 -> +6 (ours 62, bar 56)
- loss grew: cases/catalog/host/js-and-member-missing brotli11 vs competitor: +6 -> +7 (ours 70, bar 63)
- loss grew: cases/catalog/host/js-and-member-missing brotli11 vs old: +9 -> +10 (ours 70, bar 60)
- loss grew: cases/catalog/function/semantic-default-array-freshness raw vs competitor: +0 -> +13 (ours 90, bar 77)
- loss grew: cases/catalog/function/semantic-default-array-freshness raw vs old: +8 -> +23 (ours 90, bar 67)
- loss grew: cases/catalog/function/semantic-default-array-freshness gzip9 vs competitor: +0 -> +14 (ours 94, bar 80)
- loss grew: cases/catalog/function/semantic-default-array-freshness gzip9 vs old: +15 -> +32 (ours 94, bar 62)
- loss grew: cases/catalog/function/semantic-default-array-freshness brotli11 vs competitor: +4 -> +6 (ours 68, bar 62)
- loss grew: cases/catalog/function/semantic-default-array-freshness brotli11 vs old: +16 -> +18 (ours 68, bar 50)
- loss grew: cases/catalog/aggregate/frontier-default-constructor brotli11 vs competitor: +0 -> +2 (ours 81, bar 79)
- loss grew: cases/catalog/aggregate/frontier-default-constructor brotli11 vs old: +46 -> +58 (ours 81, bar 23)
- loss grew: cases/catalog/collection/frontier-array-spread-order raw vs competitor: +15 -> +20 (ours 78, bar 58)
- loss grew: cases/catalog/collection/frontier-array-spread-order raw vs old: +21 -> +26 (ours 78, bar 52)
- loss grew: cases/catalog/collection/frontier-array-spread-order gzip9 vs competitor: +13 -> +16 (ours 83, bar 67)
- loss grew: cases/catalog/collection/frontier-array-spread-order gzip9 vs old: +17 -> +20 (ours 83, bar 63)
- loss grew: cases/catalog/collection/frontier-array-spread-order brotli11 vs competitor: +24 -> +29 (ours 82, bar 53)
- loss grew: cases/catalog/collection/frontier-array-spread-order brotli11 vs old: +31 -> +36 (ours 82, bar 46)
- loss grew: apps/module-graph raw vs closure: +24 -> +25 (ours 143, bar 118)
- loss grew: apps/module-graph raw vs old: +47 -> +48 (ours 143, bar 95)
- loss grew: algorithms/helper-sharing raw vs closure: +2 -> +8 (ours 216, bar 208)
- loss grew: algorithms/helper-sharing raw vs old: +44 -> +50 (ours 216, bar 166)
- loss grew: algorithms/helper-sharing gzip9 vs closure: +3 -> +5 (ours 175, bar 170)
- loss grew: algorithms/helper-sharing gzip9 vs old: +31 -> +33 (ours 175, bar 142)
- loss grew: algorithms/helper-sharing brotli11 vs closure: +6 -> +9 (ours 158, bar 149)
- loss grew: algorithms/helper-sharing brotli11 vs old: +38 -> +41 (ours 158, bar 117)

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases raw losses to competitor: 121 -> 117
- loss count fell: cases raw losses to old: 517 -> 514
- loss count fell: cases gzip9 losses to competitor: 120 -> 116
- loss count fell: cases gzip9 losses to old: 500 -> 497
- loss count fell: cases brotli11 losses to competitor: 174 -> 155
- loss count fell: algorithms raw losses to closure: 10 -> 7
- loss count fell: algorithms gzip9 losses to closure: 9 -> 3
- loss count fell: algorithms brotli11 losses to competitor: 3 -> 0
- loss count fell: algorithms brotli11 losses to closure: 8 -> 4
- loss fell: cases/canonical/aggregates/class-scale brotli11 vs old: +19 -> +1
- loss fell: cases/canonical/aggregates/nested-struct raw vs old: +28 -> +3
- loss fell: cases/canonical/aggregates/nested-struct gzip9 vs old: +27 -> +3
- loss fell: cases/canonical/aggregates/nested-struct brotli11 vs old: +20 -> +3
- loss fell: cases/canonical/aggregates/struct-point raw vs old: +42 -> +1
- loss fell: cases/canonical/aggregates/struct-point gzip9 vs old: +39 -> +1
- loss fell: cases/canonical/aggregates/struct-point brotli11 vs old: +36 -> +1
- loss fell: cases/canonical/collections/array-pipeline brotli11 vs old: +9 -> +8
- loss fell: cases/canonical/control/early-return brotli11 vs old: +6 -> +5
- loss fell: cases/canonical/identity/saved-loop-phi brotli11 vs competitor: +6 -> +3
- loss fell: cases/canonical/identity/saved-loop-phi brotli11 vs old: +6 -> +3
- loss fell: cases/canonical/wins/aggregate-model raw vs old: +71 -> +46
- loss fell: cases/canonical/wins/aggregate-model gzip9 vs old: +59 -> +40
- loss fell: cases/canonical/wins/aggregate-model brotli11 vs old: +53 -> +35
- loss fell: cases/canonical/wins/point-batch raw vs old: +25 -> +12
- loss fell: cases/canonical/wins/point-batch gzip9 vs old: +15 -> +5
- loss fell: cases/canonical/wins/point-batch brotli11 vs old: +20 -> +13
- loss fell: cases/catalog/aggregate/struct-pairs-3-1-4-1-5-9 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-3-1-4-1-5-9 gzip9 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-3-1-4-1-5-9 brotli11 vs old: +36 -> +26
- loss fell: cases/catalog/aggregate/struct-pairs-2-7-1-8-2-8 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-2-7-1-8-2-8 gzip9 vs old: +27 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-2-7-1-8-2-8 brotli11 vs old: +35 -> +26
- loss fell: cases/catalog/aggregate/struct-pairs-1-2-3-4-5-6-7-8 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-1-2-3-4-5-6-7-8 gzip9 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-1-2-3-4-5-6-7-8 brotli11 vs old: +38 -> +29
- loss fell: cases/catalog/aggregate/struct-pairs-9-8-7-6-5-4-3-2 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-9-8-7-6-5-4-3-2 gzip9 vs old: +27 -> +22
- loss fell: cases/catalog/aggregate/struct-pairs-9-8-7-6-5-4-3-2 brotli11 vs old: +36 -> +29
- loss fell: cases/catalog/aggregate/struct-pairs-0-1-0-2-0-3-0-4 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-0-1-0-2-0-3-0-4 gzip9 vs old: +24 -> +18
- loss fell: cases/catalog/aggregate/struct-pairs-0-1-0-2-0-3-0-4 brotli11 vs old: +35 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-10-20-30-40 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-10-20-30-40 gzip9 vs old: +25 -> +20
- loss fell: cases/catalog/aggregate/struct-pairs-10-20-30-40 brotli11 vs old: +37 -> +26
- loss fell: cases/catalog/aggregate/struct-pairs-5-5-5-5-5-5 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-5-5-5-5-5-5 gzip9 vs old: +25 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-5-5-5-5-5-5 brotli11 vs old: +27 -> +25
- loss fell: cases/catalog/aggregate/struct-pairs-1-1-2-3-5-8-13-21 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-1-1-2-3-5-8-13-21 gzip9 vs old: +23 -> +19
- loss fell: cases/catalog/aggregate/struct-pairs-1-1-2-3-5-8-13-21 brotli11 vs old: +35 -> +30
- loss fell: cases/catalog/aggregate/struct-pairs-12-4-8-16-2-6 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-12-4-8-16-2-6 gzip9 vs old: +26 -> +20
- loss fell: cases/catalog/aggregate/struct-pairs-12-4-8-16-2-6 brotli11 vs old: +39 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-15-3-9-27-1-81 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-15-3-9-27-1-81 gzip9 vs old: +26 -> +19
- loss fell: cases/catalog/aggregate/struct-pairs-15-3-9-27-1-81 brotli11 vs old: +42 -> +26
- loss fell: cases/catalog/aggregate/struct-pairs-4-9-16-25-36-49 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-4-9-16-25-36-49 gzip9 vs old: +27 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-4-9-16-25-36-49 brotli11 vs old: +39 -> +28
- loss fell: cases/catalog/aggregate/struct-pairs-11-22-33-44-55-66 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-11-22-33-44-55-66 gzip9 vs old: +27 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-11-22-33-44-55-66 brotli11 vs old: +37 -> +25
- loss fell: cases/catalog/aggregate/struct-pairs-7-14-21-28-35 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-7-14-21-28-35 gzip9 vs old: +27 -> +21
- loss fell: cases/catalog/aggregate/struct-pairs-2-3-5-7-11-13-17 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-2-3-5-7-11-13-17 gzip9 vs old: +26 -> +19
- loss fell: cases/catalog/aggregate/struct-pairs-2-3-5-7-11-13-17 brotli11 vs old: +39 -> +28
- loss fell: cases/catalog/aggregate/struct-pairs-6-1-8-2-10-3 raw vs old: +33 -> +24
- loss fell: cases/catalog/aggregate/struct-pairs-6-1-8-2-10-3 gzip9 vs old: +27 -> +19
- loss fell: cases/catalog/aggregate/struct-pairs-6-1-8-2-10-3 brotli11 vs old: +35 -> +27
- loss fell: cases/catalog/aggregate/nested-struct-3-1-4-1-5-9 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-3-1-4-1-5-9 gzip9 vs old: +25 -> +21
- loss fell: cases/catalog/aggregate/nested-struct-3-1-4-1-5-9 brotli11 vs old: +31 -> +28
- loss fell: cases/catalog/aggregate/nested-struct-2-7-1-8-2-8 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-2-7-1-8-2-8 gzip9 vs old: +23 -> +19
- loss fell: cases/catalog/aggregate/nested-struct-1-2-3-4-5-6-7-8 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-1-2-3-4-5-6-7-8 gzip9 vs old: +21 -> +20
- loss fell: cases/catalog/aggregate/nested-struct-9-8-7-6-5-4-3-2 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-9-8-7-6-5-4-3-2 gzip9 vs old: +23 -> +20
- loss fell: cases/catalog/aggregate/nested-struct-0-1-0-2-0-3-0-4 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-10-20-30-40 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-10-20-30-40 gzip9 vs old: +22 -> +21
- loss fell: cases/catalog/aggregate/nested-struct-10-20-30-40 brotli11 vs old: +30 -> +29
- loss fell: cases/catalog/aggregate/nested-struct-5-5-5-5-5-5 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-5-5-5-5-5-5 gzip9 vs old: +20 -> +19
- loss fell: cases/catalog/aggregate/nested-struct-1-1-2-3-5-8-13-21 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-1-1-2-3-5-8-13-21 gzip9 vs old: +21 -> +20
- loss fell: cases/catalog/aggregate/nested-struct-12-4-8-16-2-6 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-12-4-8-16-2-6 gzip9 vs old: +21 -> +20
- loss fell: cases/catalog/aggregate/nested-struct-15-3-9-27-1-81 raw vs old: +35 -> +31
- loss fell: cases/catalog/aggregate/nested-struct-15-3-9-27-1-81 gzip9 vs old: +22 -> +20
- loss fell: cases/catalog/aggregate/class-counter-13-8 brotli11 vs old: +28 -> +22
- loss fell: cases/catalog/string/string-search-compression-press brotli11 vs competitor: +7 -> +6
- loss fell: cases/catalog/string/string-search-compression-press brotli11 vs old: +50 -> +49
- loss fell: cases/catalog/aggregate/matrix2-3-1-4-1-5-9 raw vs old: +39 -> +30
- loss fell: cases/catalog/aggregate/matrix2-3-1-4-1-5-9 gzip9 vs old: +29 -> +27
- loss fell: cases/catalog/aggregate/matrix2-2-7-1-8-2-8 raw vs old: +39 -> +30
- loss fell: cases/catalog/aggregate/matrix2-2-7-1-8-2-8 gzip9 vs old: +29 -> +25
- loss fell: cases/catalog/aggregate/matrix2-2-7-1-8-2-8 brotli11 vs old: +33 -> +29
- loss fell: cases/catalog/aggregate/matrix2-1-2-3-4-5-6-7-8 raw vs old: +39 -> +30
- loss fell: cases/catalog/aggregate/matrix2-1-2-3-4-5-6-7-8 gzip9 vs old: +27 -> +25
- loss fell: cases/catalog/aggregate/matrix2-9-8-7-6-5-4-3-2 raw vs old: +39 -> +30
- loss fell: cases/catalog/aggregate/matrix2-9-8-7-6-5-4-3-2 gzip9 vs old: +28 -> +25
- loss fell: cases/catalog/aggregate/matrix2-0-1-0-2-0-3-0-4 raw vs old: +39 -> +30
- loss fell: cases/catalog/aggregate/matrix2-0-1-0-2-0-3-0-4 gzip9 vs old: +28 -> +24
- loss fell: cases/catalog/aggregate/matrix2-0-1-0-2-0-3-0-4 brotli11 vs old: +37 -> +32
- loss fell: cases/catalog/aggregate/matrix2-10-20-30-40 raw vs old: +39 -> +30
- loss fell: cases/catalog/aggregate/matrix2-10-20-30-40 gzip9 vs old: +29 -> +26
- loss fell: cases/catalog/aggregate/matrix2-10-20-30-40 brotli11 vs old: +35 -> +29
- loss fell: cases/catalog/collection/minmax-scan-5-5-5-5-5-5 brotli11 vs old: +14 -> +11
- loss fell: cases/catalog/winner/win-aggregate-model raw vs old: +71 -> +46
- loss fell: cases/catalog/winner/win-aggregate-model gzip9 vs old: +59 -> +40
- loss fell: cases/catalog/winner/win-aggregate-model brotli11 vs old: +53 -> +35
- loss fell: cases/catalog/string/string-chars-brotli brotli11 vs old: +10 -> +9
- loss fell: cases/catalog/aggregate/record-transform-2 raw vs old: +34 -> +25
- loss fell: cases/catalog/aggregate/record-transform-2 gzip9 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/record-transform-2 brotli11 vs old: +30 -> +27
- loss fell: cases/catalog/aggregate/record-transform-3 raw vs old: +34 -> +25
- loss fell: cases/catalog/aggregate/record-transform-3 gzip9 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/record-transform-3 brotli11 vs old: +30 -> +27
- loss fell: cases/catalog/aggregate/record-transform-4 raw vs old: +34 -> +25
- loss fell: cases/catalog/aggregate/record-transform-4 gzip9 vs old: +27 -> +20
- loss fell: cases/catalog/aggregate/record-transform-4 brotli11 vs old: +39 -> +27
- loss fell: cases/catalog/aggregate/record-transform-5 raw vs old: +34 -> +25
- loss fell: cases/catalog/aggregate/record-transform-5 gzip9 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/record-transform-5 brotli11 vs old: +40 -> +27
- loss fell: cases/catalog/aggregate/record-transform-6 raw vs old: +34 -> +25
- loss fell: cases/catalog/aggregate/record-transform-6 gzip9 vs old: +26 -> +18
- loss fell: cases/catalog/aggregate/record-transform-6 brotli11 vs old: +39 -> +22
- loss fell: cases/catalog/aggregate/record-transform-7 raw vs old: +34 -> +25
- loss fell: cases/catalog/aggregate/record-transform-7 gzip9 vs old: +26 -> +19
- loss fell: cases/catalog/aggregate/record-transform-7 brotli11 vs old: +40 -> +23
- loss fell: cases/catalog/winner/win-point-batch raw vs old: +25 -> +12
- loss fell: cases/catalog/winner/win-point-batch gzip9 vs old: +15 -> +5
- loss fell: cases/catalog/winner/win-point-batch brotli11 vs old: +20 -> +13
- loss fell: cases/catalog/winner/win-class-scale brotli11 vs old: +19 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-1n2n4 gzip9 vs old: +4 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-1n2n4 brotli11 vs competitor: +3 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-1n2n4 brotli11 vs old: +10 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-3nn1n5 brotli11 vs competitor: +2 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-3nn1n5 brotli11 vs old: +9 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-5n3n6 brotli11 vs competitor: +4 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-5n3n6 brotli11 vs old: +10 -> +9
- loss fell: cases/catalog/closure/edge-closure-mutation-7nn2n7 brotli11 vs competitor: +2 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-7nn2n7 brotli11 vs old: +9 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-11n4n5 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-11n4n5 brotli11 vs competitor: +4 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-11n4n5 brotli11 vs old: +11 -> +9
- loss fell: cases/catalog/closure/edge-closure-mutation-n4n5n6 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-n4n5n6 brotli11 vs competitor: +5 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-n4n5n6 brotli11 vs old: +11 -> +7
- loss fell: cases/catalog/closure/edge-closure-mutation-20nn3n4 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-2n7n5 brotli11 vs competitor: +5 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-2n7n5 brotli11 vs old: +11 -> +9
- loss fell: cases/catalog/closure/edge-closure-mutation-9n1n8 gzip9 vs old: +4 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-9n1n8 brotli11 vs competitor: +3 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-9n1n8 brotli11 vs old: +10 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-n8nn2n6 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-n8nn2n6 brotli11 vs competitor: +7 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-n8nn2n6 brotli11 vs old: +14 -> +10
- loss fell: cases/catalog/string/edge-string-utf16-accent brotli11 vs old: +52 -> +44
- loss fell: cases/catalog/effect/edge-exception-finally-0-4-6 brotli11 vs old: +6 -> +5
- loss fell: cases/catalog/effect/edge-exception-finally-0-7-9 brotli11 vs old: +5 -> +4
- loss fell: cases/catalog/effect/edge-exception-finally-0-10-12 brotli11 vs old: +17 -> +5
- loss fell: cases/catalog/control/frontier-inline-for raw vs old: +115 -> +53
- loss fell: cases/catalog/control/frontier-inline-for gzip9 vs old: +78 -> +40
- loss fell: cases/catalog/control/frontier-inline-for brotli11 vs old: +71 -> +44
- loss fell: cases/catalog/aggregate/frontier-default-constructor raw vs old: +77 -> +69
- loss fell: cases/catalog/aggregate/frontier-inheritance-super brotli11 vs old: +9 -> +8
- loss fell: apps/aggregate-model raw vs old: +71 -> +46
- loss fell: apps/aggregate-model gzip9 vs old: +59 -> +40
- loss fell: apps/aggregate-model brotli11 vs old: +53 -> +35
- loss fell: apps/module-graph gzip9 vs closure: +15 -> +14
- loss fell: apps/module-graph gzip9 vs old: +32 -> +31
- loss fell: apps/module-graph brotli11 vs closure: +6 -> +5
- loss fell: apps/module-graph brotli11 vs old: +35 -> +34
- loss fell: algorithms/collection-geometry raw vs closure: +42 -> +10
- loss fell: algorithms/collection-geometry raw vs old: +99 -> +67
- loss fell: algorithms/collection-geometry gzip9 vs old: +63 -> +33
- loss fell: algorithms/collection-geometry brotli11 vs old: +71 -> +38
- loss fell: algorithms/dictionary-template-router raw vs closure: +64 -> +21
- loss fell: algorithms/dictionary-template-router raw vs old: +89 -> +46
- loss fell: algorithms/dictionary-template-router gzip9 vs old: +37 -> +18
- loss fell: algorithms/dictionary-template-router brotli11 vs old: +37 -> +24
- loss fell: algorithms/large-event-analytics raw vs old: +80 -> +10
- loss fell: algorithms/large-event-analytics gzip9 vs old: +27 -> +1
- loss fell: algorithms/large-event-analytics brotli11 vs closure: +35 -> +2
- loss fell: algorithms/large-event-analytics brotli11 vs old: +45 -> +12
- loss fell: algorithms/policy-specialization raw vs closure: +26 -> +22
- loss fell: algorithms/policy-specialization raw vs old: +42 -> +38
- loss fell: algorithms/policy-specialization gzip9 vs closure: +13 -> +12
- loss fell: algorithms/policy-specialization gzip9 vs old: +24 -> +23
- loss fell: algorithms/policy-specialization brotli11 vs closure: +21 -> +16
- loss fell: algorithms/policy-specialization brotli11 vs old: +27 -> +22
- loss fell: algorithms/shape-invoice-pipeline raw vs closure: +130 -> +9
- loss fell: algorithms/shape-invoice-pipeline raw vs old: +258 -> +137
- loss fell: algorithms/shape-invoice-pipeline gzip9 vs old: +94 -> +62
- loss fell: algorithms/shape-invoice-pipeline brotli11 vs old: +105 -> +66
- loss fell: algorithms/state-machine-parser raw vs old: +53 -> +19
- loss fell: algorithms/state-machine-parser gzip9 vs old: +36 -> +14
- loss fell: algorithms/state-machine-parser brotli11 vs old: +40 -> +22
- loss fell: algorithms/stateful-packet-decoder raw vs old: +62 -> +46
- loss fell: algorithms/stateful-packet-decoder gzip9 vs old: +50 -> +26
- loss fell: algorithms/stateful-packet-decoder brotli11 vs old: +53 -> +25
- loss fell: algorithms/static-rule-engine raw vs old: +27 -> +15
- loss fell: algorithms/static-rule-engine gzip9 vs old: +19 -> +12
- loss fell: algorithms/static-rule-engine brotli11 vs old: +26 -> +18
- loss fell: algorithms/string-dictionary raw vs closure: +58 -> +29
- loss fell: algorithms/string-dictionary raw vs old: +83 -> +54
- loss fell: algorithms/string-dictionary gzip9 vs old: +36 -> +27
- loss fell: algorithms/string-dictionary brotli11 vs old: +44 -> +35
- loss fell: algorithms/collection-geometry gzip9 vs closure: +24 -> +0
- loss fell: algorithms/collection-geometry brotli11 vs competitor: +1 -> +0
- loss fell: algorithms/collection-geometry brotli11 vs closure: +14 -> +0
- loss fell: algorithms/dictionary-template-router gzip9 vs closure: +1 -> +0
- loss fell: algorithms/large-event-analytics raw vs closure: +27 -> +0
- loss fell: algorithms/large-event-analytics gzip9 vs closure: +10 -> +0
- loss fell: algorithms/shape-invoice-pipeline gzip9 vs closure: +12 -> +0
- loss fell: algorithms/shape-invoice-pipeline brotli11 vs competitor: +1 -> +0
- loss fell: algorithms/shape-invoice-pipeline brotli11 vs closure: +33 -> +0
- loss fell: algorithms/state-machine-parser raw vs closure: +9 -> +0
- loss fell: algorithms/state-machine-parser gzip9 vs closure: +11 -> +0
- loss fell: algorithms/stateful-packet-decoder raw vs closure: +11 -> +0
- loss fell: algorithms/stateful-packet-decoder gzip9 vs closure: +24 -> +0
- loss fell: algorithms/stateful-packet-decoder brotli11 vs closure: +26 -> +0
- loss fell: algorithms/static-rule-engine brotli11 vs closure: +4 -> +0
- loss fell: algorithms/string-dictionary brotli11 vs competitor: +2 -> +0
- loss fell: cases/canonical/aggregates/class-scale raw vs old: +19 -> +0
- loss fell: cases/canonical/aggregates/class-scale gzip9 vs old: +14 -> +0
- loss fell: cases/canonical/aggregates/struct-point raw vs competitor: +12 -> +0
- loss fell: cases/canonical/aggregates/struct-point gzip9 vs competitor: +9 -> +0
- loss fell: cases/canonical/aggregates/struct-point brotli11 vs competitor: +6 -> +0
- loss fell: cases/canonical/functions/closure-capture raw vs competitor: +6 -> +0
- loss fell: cases/canonical/functions/closure-capture raw vs old: +9 -> +0
- loss fell: cases/canonical/functions/closure-capture gzip9 vs competitor: +4 -> +0
- loss fell: cases/canonical/functions/closure-capture gzip9 vs old: +7 -> +0
- loss fell: cases/canonical/wins/point-batch raw vs competitor: +7 -> +0
- loss fell: cases/canonical/wins/point-batch gzip9 vs competitor: +8 -> +0
- loss fell: cases/canonical/wins/point-batch brotli11 vs competitor: +5 -> +0
- loss fell: cases/catalog/aggregate/matrix2-0-1-0-2-0-3-0-4 brotli11 vs competitor: +3 -> +0
- loss fell: cases/catalog/aggregate/matrix2-2-7-1-8-2-8 brotli11 vs competitor: +1 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-0-1-0-2-0-3-0-4 brotli11 vs competitor: +4 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-1-2-3-4-5-6-7-8 brotli11 vs competitor: +6 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-10-20-30-40 brotli11 vs competitor: +2 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-11-22-33-44-55-66 brotli11 vs competitor: +5 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-12-4-8-16-2-6 brotli11 vs competitor: +8 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-15-3-9-27-1-81 brotli11 vs competitor: +9 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-2-3-5-7-11-13-17 brotli11 vs competitor: +6 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-2-7-1-8-2-8 brotli11 vs competitor: +3 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-3-1-4-1-5-9 brotli11 vs competitor: +5 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-4-9-16-25-36-49 brotli11 vs competitor: +7 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-6-1-8-2-10-3 brotli11 vs competitor: +2 -> +0
- loss fell: cases/catalog/aggregate/struct-pairs-9-8-7-6-5-4-3-2 brotli11 vs competitor: +5 -> +0
- loss fell: cases/catalog/control/frontier-inline-for raw vs competitor: +44 -> +0
- loss fell: cases/catalog/control/frontier-inline-for gzip9 vs competitor: +22 -> +0
- loss fell: cases/catalog/control/frontier-inline-for brotli11 vs competitor: +7 -> +0
- loss fell: cases/catalog/effect/edge-exception-finally-0-10-12 brotli11 vs competitor: +6 -> +0
- loss fell: cases/catalog/string/edge-string-utf16-accent brotli11 vs competitor: +3 -> +0
- loss fell: cases/catalog/winner/win-class-scale raw vs old: +19 -> +0
- loss fell: cases/catalog/winner/win-class-scale gzip9 vs old: +14 -> +0
- loss fell: cases/catalog/winner/win-point-batch raw vs competitor: +7 -> +0
- loss fell: cases/catalog/winner/win-point-batch gzip9 vs competitor: +8 -> +0
- loss fell: cases/catalog/winner/win-point-batch brotli11 vs competitor: +5 -> +0
