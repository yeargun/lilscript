# Generic corpus ratchet

Compiler `5a1b5da099dfca52`, 642 items, bars `770649437963ff5b`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 110 | 612 | 50251 | 58285 |
| cases | raw | old | 506 | 612 | 50251 | 44195 |
| cases | gzip9 | competitor | 109 | 612 | 48047 | 54669 |
| cases | gzip9 | old | 489 | 610 | 47873 | 45206 |
| cases | brotli11 | competitor | 167 | 612 | 38530 | 43541 |
| cases | brotli11 | old | 509 | 611 | 38481 | 33656 |
| apps | raw | competitor | 0 | 7 | 859 | 1527 |
| apps | raw | closure | 1 | 7 | 859 | 1332 |
| apps | raw | old | 5 | 7 | 859 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 738 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 738 | 982 |
| apps | gzip9 | old | 5 | 7 | 738 | 711 |
| apps | brotli11 | competitor | 0 | 7 | 578 | 870 |
| apps | brotli11 | closure | 2 | 7 | 578 | 834 |
| apps | brotli11 | old | 5 | 7 | 578 | 558 |
| algorithms | raw | competitor | 1 | 11 | 5076 | 6210 |
| algorithms | raw | closure | 10 | 11 | 5076 | 4522 |
| algorithms | raw | old | 11 | 11 | 5076 | 3995 |
| algorithms | gzip9 | competitor | 2 | 11 | 3157 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3157 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3157 | 2662 |
| algorithms | brotli11 | competitor | 4 | 11 | 2880 | 3039 |
| algorithms | brotli11 | closure | 9 | 11 | 2880 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2880 | 2305 |

Failures (compile, crash or oracle): 36. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases raw losses to old: 567 -> 506
- loss count fell: cases gzip9 losses to old: 545 -> 489
- loss count fell: cases brotli11 losses to competitor: 174 -> 167
- loss count fell: cases brotli11 losses to old: 569 -> 509
- loss count fell: apps raw losses to old: 7 -> 5
- loss count fell: apps gzip9 losses to old: 7 -> 5
- loss count fell: apps brotli11 losses to old: 7 -> 5
- loss fell: apps/control-flow-engine raw vs old: +27 -> +0
- loss fell: apps/control-flow-engine gzip9 vs old: +18 -> +0
- loss fell: apps/control-flow-engine brotli11 vs old: +23 -> +0
- loss fell: apps/numerical-kernel raw vs old: +10 -> +0
- loss fell: apps/numerical-kernel gzip9 vs old: +2 -> +0
- loss fell: apps/numerical-kernel brotli11 vs old: +1 -> +0
- loss fell: cases/canonical/control/fibonacci raw vs old: +25 -> +0
- loss fell: cases/canonical/control/fibonacci gzip9 vs old: +16 -> +0
- loss fell: cases/canonical/control/fibonacci brotli11 vs competitor: +2 -> +0
- loss fell: cases/canonical/control/fibonacci brotli11 vs old: +27 -> +0
- loss fell: cases/canonical/control/gcd raw vs old: +17 -> +0
- loss fell: cases/canonical/control/gcd gzip9 vs old: +16 -> +0
- loss fell: cases/canonical/control/gcd brotli11 vs old: +13 -> +0
- loss fell: cases/canonical/control/loop-control raw vs old: +25 -> +0
- loss fell: cases/canonical/control/loop-control gzip9 vs old: +13 -> +0
- loss fell: cases/canonical/control/loop-control brotli11 vs old: +17 -> +0
- loss fell: cases/canonical/control/nested-loop-score raw vs old: +27 -> +0
- loss fell: cases/canonical/control/nested-loop-score gzip9 vs old: +18 -> +0
- loss fell: cases/canonical/control/nested-loop-score brotli11 vs old: +23 -> +0
- loss fell: cases/canonical/scalars/i32-churn raw vs old: +15 -> +0
- loss fell: cases/canonical/scalars/i32-churn gzip9 vs old: +6 -> +0
- loss fell: cases/canonical/scalars/i32-churn brotli11 vs old: +10 -> +0
- loss fell: cases/catalog/closure/closure-capture-2 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-2 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-2 brotli11 vs old: +26 -> +0
- loss fell: cases/catalog/closure/closure-capture-3 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-3 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-3 brotli11 vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-4 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-4 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-4 brotli11 vs old: +25 -> +0
- loss fell: cases/catalog/closure/closure-capture-5 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-5 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-5 brotli11 vs old: +23 -> +0
- loss fell: cases/catalog/closure/closure-capture-6 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-6 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-6 brotli11 vs old: +28 -> +0
- loss fell: cases/catalog/closure/closure-capture-7 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-7 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-7 brotli11 vs old: +23 -> +0
- loss fell: cases/catalog/closure/closure-capture-8 raw vs old: +22 -> +0
- loss fell: cases/catalog/closure/closure-capture-8 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/closure/closure-capture-8 brotli11 vs old: +24 -> +0
- loss fell: cases/catalog/function/fibonacci-10 raw vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-10 gzip9 vs old: +15 -> +0
- loss fell: cases/catalog/function/fibonacci-10 brotli11 vs old: +27 -> +0
- loss fell: cases/catalog/function/fibonacci-12 raw vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-12 gzip9 vs old: +16 -> +0
- loss fell: cases/catalog/function/fibonacci-12 brotli11 vs competitor: +2 -> +0
- loss fell: cases/catalog/function/fibonacci-12 brotli11 vs old: +27 -> +0
- loss fell: cases/catalog/function/fibonacci-14 raw vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-14 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/function/fibonacci-14 brotli11 vs competitor: +3 -> +0
- loss fell: cases/catalog/function/fibonacci-14 brotli11 vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-16 raw vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-16 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/function/fibonacci-16 brotli11 vs competitor: +3 -> +0
- loss fell: cases/catalog/function/fibonacci-16 brotli11 vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-18 raw vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-18 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/function/fibonacci-18 brotli11 vs competitor: +3 -> +0
- loss fell: cases/catalog/function/fibonacci-18 brotli11 vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-8 raw vs old: +25 -> +0
- loss fell: cases/catalog/function/fibonacci-8 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/function/fibonacci-8 brotli11 vs old: +24 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-1073741824n7n13n4n11 raw vs old: +18 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-1073741824n7n13n4n11 gzip9 vs old: +10 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-1073741824n7n13n4n11 brotli11 vs old: +8 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-123456789n1664525n1013904223n3n257 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-123456789n1664525n1013904223n3n257 gzip9 vs old: +6 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-123456789n1664525n1013904223n3n257 brotli11 vs old: +10 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-1nn2147483647nn1n3n19 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-1nn2147483647nn1n3n19 gzip9 vs old: +5 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-1nn2147483647nn1n3n19 brotli11 vs old: +12 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-2147483647n3n1n2n7 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-2147483647n3n1n2n7 gzip9 vs old: +7 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-2147483647n3n1n2n7 brotli11 vs old: +6 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-65535n65537n23n3n97 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-65535n65537n23n3n97 gzip9 vs old: +6 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-65535n65537n23n3n97 brotli11 vs old: +8 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n1073741824nn3n17n2n5 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n1073741824nn3n17n2n5 gzip9 vs old: +7 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n1073741824nn3n17n2n5 brotli11 vs old: +6 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n1n2147483647n1n3nn23 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n1n2147483647n1n3nn23 gzip9 vs old: +6 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n1n2147483647n1n3nn23 brotli11 vs old: +11 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n2147483647n5nn9n3nn3 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n2147483647n5nn9n3nn3 gzip9 vs old: +6 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n2147483647n5nn9n3nn3 brotli11 vs old: +7 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n65536n32769nn31n4nn17 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n65536n32769nn31n4nn17 gzip9 vs old: +7 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n65536n32769nn31n4nn17 brotli11 vs old: +10 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n987654321n1103515245n12345n2n4093 raw vs old: +15 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n987654321n1103515245n12345n2n4093 gzip9 vs old: +4 -> +0
- loss fell: cases/catalog/integer/edge-i32-churn-n987654321n1103515245n12345n2n4093 brotli11 vs old: +12 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-10-4-9 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-10-4-9 gzip9 vs old: +13 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-10-4-9 brotli11 vs old: +17 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-11-5-10 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-11-5-10 gzip9 vs old: +13 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-11-5-10 brotli11 vs old: +15 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-12-2-9 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-12-2-9 gzip9 vs old: +13 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-12-2-9 brotli11 vs old: +17 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-13-3-11 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-13-3-11 gzip9 vs old: +12 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-13-3-11 brotli11 vs old: +15 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-14-4-12 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-14-4-12 gzip9 vs old: +12 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-14-4-12 brotli11 vs old: +15 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-15-5-13 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-15-5-13 gzip9 vs old: +12 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-15-5-13 brotli11 vs old: +15 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-16-6-14 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-16-6-14 gzip9 vs old: +13 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-16-6-14 brotli11 vs old: +15 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-18-7-15 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-18-7-15 gzip9 vs old: +13 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-18-7-15 brotli11 vs old: +14 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-8-2-7 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-8-2-7 gzip9 vs old: +12 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-8-2-7 brotli11 vs old: +19 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-9-3-8 raw vs old: +25 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-9-3-8 gzip9 vs old: +12 -> +0
- loss fell: cases/catalog/loop/edge-loop-control-9-3-8 brotli11 vs old: +19 -> +0
- loss fell: cases/catalog/loop/gcd-100-35 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-100-35 gzip9 vs old: +16 -> +0
- loss fell: cases/catalog/loop/gcd-100-35 brotli11 vs old: +18 -> +0
- loss fell: cases/catalog/loop/gcd-1071-462 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-1071-462 gzip9 vs old: +16 -> +0
- loss fell: cases/catalog/loop/gcd-1071-462 brotli11 vs old: +13 -> +0
- loss fell: cases/catalog/loop/gcd-144-89 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-144-89 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-144-89 brotli11 vs competitor: +1 -> +0
- loss fell: cases/catalog/loop/gcd-144-89 brotli11 vs old: +14 -> +0
- loss fell: cases/catalog/loop/gcd-270-192 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-270-192 gzip9 vs old: +16 -> +0
- loss fell: cases/catalog/loop/gcd-270-192 brotli11 vs old: +13 -> +0
- loss fell: cases/catalog/loop/gcd-48-18 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-48-18 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-48-18 brotli11 vs old: +13 -> +0
- loss fell: cases/catalog/loop/gcd-512-48 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-512-48 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-512-48 brotli11 vs old: +13 -> +0
- loss fell: cases/catalog/loop/gcd-81-27 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-81-27 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-81-27 brotli11 vs old: +13 -> +0
- loss fell: cases/catalog/loop/gcd-99-27 raw vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-99-27 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/loop/gcd-99-27 brotli11 vs competitor: +1 -> +0
- loss fell: cases/catalog/loop/gcd-99-27 brotli11 vs old: +14 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-10 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-10 gzip9 vs old: +18 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-10 brotli11 vs old: +23 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-11 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-11 gzip9 vs old: +18 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-11 brotli11 vs old: +22 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-12 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-12 gzip9 vs old: +18 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-12 brotli11 vs old: +23 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-4 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-4 gzip9 vs old: +17 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-4 brotli11 vs old: +15 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-5 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-5 gzip9 vs old: +18 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-5 brotli11 vs old: +18 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-6 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-6 gzip9 vs old: +20 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-6 brotli11 vs old: +17 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-7 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-7 gzip9 vs old: +19 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-7 brotli11 vs old: +17 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-8 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-8 gzip9 vs old: +20 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-8 brotli11 vs old: +17 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-9 raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-9 gzip9 vs old: +19 -> +0
- loss fell: cases/catalog/loop/nested-loop-score-9 brotli11 vs old: +18 -> +0
- loss fell: cases/catalog/loop/win-control-flow raw vs old: +27 -> +0
- loss fell: cases/catalog/loop/win-control-flow gzip9 vs old: +18 -> +0
- loss fell: cases/catalog/loop/win-control-flow brotli11 vs old: +23 -> +0
- loss fell: cases/catalog/number/edge-number-flow-0p0625n0p03125n16n6 raw vs old: +2 -> +0
- loss fell: cases/catalog/number/edge-number-flow-0p0625n0p03125n16n6 brotli11 vs old: +1 -> +0
- loss fell: cases/catalog/number/edge-number-flow-100p5nn10p25n0p125n4 raw vs old: +2 -> +0
- loss fell: cases/catalog/number/edge-number-flow-100p5nn10p25n0p125n4 brotli11 vs old: +3 -> +0
- loss fell: cases/catalog/number/edge-number-flow-1p125nn0p25n3p5n5 raw vs old: +2 -> +0
- loss fell: cases/catalog/number/edge-number-flow-1p125nn0p25n3p5n5 brotli11 vs old: +2 -> +0
- loss fell: cases/catalog/number/edge-number-flow-3p75n0p125nn2n3 raw vs old: +1 -> +0
- loss fell: cases/catalog/number/edge-number-flow-n8p25n2p5nn0p25n5 raw vs old: +2 -> +0
- loss fell: cases/catalog/number/edge-number-flow-n8p25n2p5nn0p25n5 brotli11 vs old: +1 -> +0

