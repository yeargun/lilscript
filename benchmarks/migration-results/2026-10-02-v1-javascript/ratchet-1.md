# Generic corpus ratchet

Compiler `3e13744e26f6048a`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 120 | 624 | 52601 | 60614 |
| cases | raw | old | 516 | 624 | 52601 | 45935 |
| cases | gzip9 | competitor | 119 | 624 | 49789 | 56410 |
| cases | gzip9 | old | 499 | 622 | 49613 | 46478 |
| cases | brotli11 | competitor | 172 | 624 | 39973 | 44919 |
| cases | brotli11 | old | 524 | 623 | 39924 | 34677 |
| apps | raw | competitor | 0 | 7 | 836 | 1527 |
| apps | raw | closure | 1 | 7 | 836 | 1332 |
| apps | raw | old | 5 | 7 | 836 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 726 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 726 | 982 |
| apps | gzip9 | old | 5 | 7 | 726 | 711 |
| apps | brotli11 | competitor | 1 | 7 | 575 | 870 |
| apps | brotli11 | closure | 2 | 7 | 575 | 834 |
| apps | brotli11 | old | 5 | 7 | 575 | 558 |
| algorithms | raw | competitor | 0 | 11 | 4841 | 6210 |
| algorithms | raw | closure | 9 | 11 | 4841 | 4522 |
| algorithms | raw | old | 11 | 11 | 4841 | 3995 |
| algorithms | gzip9 | competitor | 1 | 11 | 3074 | 3440 |
| algorithms | gzip9 | closure | 7 | 11 | 3074 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3074 | 2662 |
| algorithms | brotli11 | competitor | 2 | 11 | 2789 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2789 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2789 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **fail**.

## Blocking

- loss count grew: cases brotli11 losses to old: 521 -> 524
- loss count grew: algorithms gzip9 losses to competitor: 0 -> 1
- loss grew: cases/catalog/control/enum-int-dispatch-4 brotli11 vs old: +0 -> +3 (ours 91, bar 88)
- loss grew: cases/catalog/control/enum-int-dispatch-12 brotli11 vs old: +0 -> +3 (ours 91, bar 88)
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
- loss grew: algorithms/collection-geometry raw vs closure: +42 -> +93 (ours 376, bar 283)
- loss grew: algorithms/collection-geometry raw vs old: +99 -> +150 (ours 376, bar 226)
- loss grew: algorithms/collection-geometry gzip9 vs closure: +24 -> +42 (ours 252, bar 210)
- loss grew: algorithms/collection-geometry gzip9 vs old: +63 -> +81 (ours 252, bar 171)
- loss grew: algorithms/collection-geometry brotli11 vs competitor: +1 -> +14 (ours 225, bar 211)
- loss grew: algorithms/collection-geometry brotli11 vs closure: +14 -> +27 (ours 225, bar 198)
- loss grew: algorithms/collection-geometry brotli11 vs old: +71 -> +84 (ours 225, bar 141)
- loss grew: algorithms/helper-sharing raw vs closure: +2 -> +8 (ours 216, bar 208)
- loss grew: algorithms/helper-sharing raw vs old: +44 -> +50 (ours 216, bar 166)
- loss grew: algorithms/helper-sharing gzip9 vs closure: +3 -> +5 (ours 175, bar 170)
- loss grew: algorithms/helper-sharing gzip9 vs old: +31 -> +33 (ours 175, bar 142)
- loss grew: algorithms/helper-sharing brotli11 vs closure: +6 -> +9 (ours 158, bar 149)
- loss grew: algorithms/helper-sharing brotli11 vs old: +38 -> +41 (ours 158, bar 117)
- loss grew: algorithms/shape-invoice-pipeline raw vs closure: +130 -> +138 (ours 590, bar 452)
- loss grew: algorithms/shape-invoice-pipeline raw vs old: +258 -> +266 (ours 590, bar 324)
- loss grew: algorithms/shape-invoice-pipeline gzip9 vs competitor: +0 -> +2 (ours 320, bar 318)
- loss grew: algorithms/shape-invoice-pipeline gzip9 vs closure: +12 -> +22 (ours 320, bar 298)
- loss grew: algorithms/shape-invoice-pipeline gzip9 vs old: +94 -> +104 (ours 320, bar 216)
- loss grew: algorithms/shape-invoice-pipeline brotli11 vs competitor: +1 -> +4 (ours 297, bar 293)
- loss grew: algorithms/shape-invoice-pipeline brotli11 vs closure: +33 -> +36 (ours 297, bar 261)
- loss grew: algorithms/shape-invoice-pipeline brotli11 vs old: +105 -> +108 (ours 297, bar 189)
- loss grew: algorithms/state-machine-parser brotli11 vs closure: +0 -> +1 (ours 245, bar 244)
- loss grew: algorithms/state-machine-parser brotli11 vs old: +40 -> +41 (ours 245, bar 204)

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases raw losses to competitor: 121 -> 120
- loss count fell: cases raw losses to old: 517 -> 516
- loss count fell: cases gzip9 losses to competitor: 120 -> 119
- loss count fell: cases gzip9 losses to old: 500 -> 499
- loss count fell: cases brotli11 losses to competitor: 174 -> 172
- loss count fell: algorithms raw losses to closure: 10 -> 9
- loss count fell: algorithms gzip9 losses to closure: 9 -> 7
- loss count fell: algorithms brotli11 losses to competitor: 3 -> 2
- loss fell: cases/canonical/collections/array-pipeline brotli11 vs old: +9 -> +8
- loss fell: cases/canonical/control/early-return brotli11 vs old: +6 -> +5
- loss fell: cases/canonical/identity/saved-loop-phi brotli11 vs competitor: +6 -> +3
- loss fell: cases/canonical/identity/saved-loop-phi brotli11 vs old: +6 -> +3
- loss fell: cases/catalog/aggregate/nested-struct-3-1-4-1-5-9 brotli11 vs old: +31 -> +30
- loss fell: cases/catalog/aggregate/class-counter-13-8 brotli11 vs old: +28 -> +22
- loss fell: cases/catalog/string/string-search-compression-press brotli11 vs competitor: +7 -> +6
- loss fell: cases/catalog/string/string-search-compression-press brotli11 vs old: +50 -> +49
- loss fell: cases/catalog/collection/minmax-scan-5-5-5-5-5-5 brotli11 vs old: +14 -> +11
- loss fell: cases/catalog/string/string-chars-brotli brotli11 vs old: +10 -> +9
- loss fell: cases/catalog/aggregate/record-transform-4 brotli11 vs old: +39 -> +30
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
- loss fell: apps/module-graph gzip9 vs closure: +15 -> +14
- loss fell: apps/module-graph gzip9 vs old: +32 -> +31
- loss fell: apps/module-graph brotli11 vs closure: +6 -> +5
- loss fell: apps/module-graph brotli11 vs old: +35 -> +34
- loss fell: algorithms/dictionary-template-router raw vs closure: +64 -> +21
- loss fell: algorithms/dictionary-template-router raw vs old: +89 -> +46
- loss fell: algorithms/dictionary-template-router gzip9 vs old: +37 -> +18
- loss fell: algorithms/dictionary-template-router brotli11 vs old: +37 -> +24
- loss fell: algorithms/large-event-analytics raw vs old: +80 -> +36
- loss fell: algorithms/large-event-analytics gzip9 vs old: +27 -> +8
- loss fell: algorithms/large-event-analytics brotli11 vs closure: +35 -> +17
- loss fell: algorithms/large-event-analytics brotli11 vs old: +45 -> +27
- loss fell: algorithms/policy-specialization raw vs closure: +26 -> +22
- loss fell: algorithms/policy-specialization raw vs old: +42 -> +38
- loss fell: algorithms/policy-specialization gzip9 vs closure: +13 -> +12
- loss fell: algorithms/policy-specialization gzip9 vs old: +24 -> +23
- loss fell: algorithms/policy-specialization brotli11 vs closure: +21 -> +16
- loss fell: algorithms/policy-specialization brotli11 vs old: +27 -> +22
- loss fell: algorithms/state-machine-parser raw vs closure: +9 -> +1
- loss fell: algorithms/state-machine-parser raw vs old: +53 -> +45
- loss fell: algorithms/state-machine-parser gzip9 vs closure: +11 -> +6
- loss fell: algorithms/state-machine-parser gzip9 vs old: +36 -> +31
- loss fell: algorithms/stateful-packet-decoder raw vs closure: +11 -> +7
- loss fell: algorithms/stateful-packet-decoder raw vs old: +62 -> +58
- loss fell: algorithms/stateful-packet-decoder gzip9 vs closure: +24 -> +3
- loss fell: algorithms/stateful-packet-decoder gzip9 vs old: +50 -> +29
- loss fell: algorithms/stateful-packet-decoder brotli11 vs closure: +26 -> +4
- loss fell: algorithms/stateful-packet-decoder brotli11 vs old: +53 -> +31
- loss fell: algorithms/static-rule-engine raw vs old: +27 -> +15
- loss fell: algorithms/static-rule-engine gzip9 vs old: +19 -> +12
- loss fell: algorithms/static-rule-engine brotli11 vs old: +26 -> +18
- loss fell: algorithms/string-dictionary raw vs closure: +58 -> +29
- loss fell: algorithms/string-dictionary raw vs old: +83 -> +54
- loss fell: algorithms/string-dictionary gzip9 vs old: +36 -> +27
- loss fell: algorithms/string-dictionary brotli11 vs old: +44 -> +35
- loss fell: algorithms/dictionary-template-router gzip9 vs closure: +1 -> +0
- loss fell: algorithms/large-event-analytics raw vs closure: +27 -> +0
- loss fell: algorithms/large-event-analytics gzip9 vs closure: +10 -> +0
- loss fell: algorithms/static-rule-engine brotli11 vs closure: +4 -> +0
- loss fell: algorithms/string-dictionary brotli11 vs competitor: +2 -> +0
- loss fell: cases/canonical/functions/closure-capture raw vs competitor: +6 -> +0
- loss fell: cases/canonical/functions/closure-capture raw vs old: +9 -> +0
- loss fell: cases/canonical/functions/closure-capture gzip9 vs competitor: +4 -> +0
- loss fell: cases/canonical/functions/closure-capture gzip9 vs old: +7 -> +0
- loss fell: cases/catalog/control/frontier-inline-for raw vs competitor: +44 -> +0
- loss fell: cases/catalog/control/frontier-inline-for gzip9 vs competitor: +22 -> +0
- loss fell: cases/catalog/control/frontier-inline-for brotli11 vs competitor: +7 -> +0
- loss fell: cases/catalog/effect/edge-exception-finally-0-10-12 brotli11 vs competitor: +6 -> +0
- loss fell: cases/catalog/string/edge-string-utf16-accent brotli11 vs competitor: +3 -> +0
