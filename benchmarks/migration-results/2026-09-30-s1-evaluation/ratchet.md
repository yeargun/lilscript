# Generic corpus ratchet

Compiler `4bd8b39b22560cd2` (s1-1), 642 items, bars `770649437963ff5b`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 110 | 612 | 55007 | 58285 |
| cases | raw | old | 567 | 612 | 55007 | 44195 |
| cases | gzip9 | competitor | 110 | 612 | 52332 | 54669 |
| cases | gzip9 | old | 546 | 610 | 52158 | 45206 |
| cases | brotli11 | competitor | 195 | 612 | 42473 | 43541 |
| cases | brotli11 | old | 578 | 611 | 42392 | 33656 |
| apps | raw | competitor | 0 | 7 | 1090 | 1527 |
| apps | raw | closure | 1 | 7 | 1090 | 1332 |
| apps | raw | old | 7 | 7 | 1090 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 926 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 926 | 982 |
| apps | gzip9 | old | 7 | 7 | 926 | 711 |
| apps | brotli11 | competitor | 1 | 7 | 744 | 870 |
| apps | brotli11 | closure | 3 | 7 | 744 | 834 |
| apps | brotli11 | old | 7 | 7 | 744 | 558 |
| algorithms | raw | competitor | 1 | 11 | 5076 | 6210 |
| algorithms | raw | closure | 10 | 11 | 5076 | 4522 |
| algorithms | raw | old | 11 | 11 | 5076 | 3995 |
| algorithms | gzip9 | competitor | 2 | 11 | 3195 | 3440 |
| algorithms | gzip9 | closure | 10 | 11 | 3195 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3195 | 2662 |
| algorithms | brotli11 | competitor | 4 | 11 | 2900 | 3039 |
| algorithms | brotli11 | closure | 9 | 11 | 2900 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2900 | 2305 |

Failures (compile, crash or oracle): 36. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases raw losses to old: 570 -> 567
- loss count fell: cases gzip9 losses to old: 547 -> 546
- loss count fell: cases brotli11 losses to old: 584 -> 578
- loss count fell: apps raw losses to closure: 2 -> 1
- loss count fell: apps gzip9 losses to competitor: 2 -> 1
- loss count fell: apps brotli11 losses to competitor: 2 -> 1
- loss count fell: apps brotli11 losses to closure: 4 -> 3
- loss fell: cases/catalog/generic/frontier-generic-equality raw vs old: +11 -> +9
- loss fell: cases/catalog/generic/frontier-generic-equality gzip9 vs old: +12 -> +9
- loss fell: cases/catalog/generic/frontier-generic-equality brotli11 vs old: +11 -> +7
- loss fell: apps/numerical-kernel raw vs old: +37 -> +10
- loss fell: apps/numerical-kernel gzip9 vs old: +21 -> +6
- loss fell: apps/numerical-kernel brotli11 vs old: +21 -> +1
- loss fell: apps/numerical-kernel raw vs closure: +9 -> +0
- loss fell: apps/numerical-kernel gzip9 vs competitor: +5 -> +0
- loss fell: apps/numerical-kernel brotli11 vs competitor: +9 -> +0
- loss fell: apps/numerical-kernel brotli11 vs closure: +3 -> +0
- loss fell: cases/canonical/aggregates/enum-dispatch raw vs old: +1 -> +0
- loss fell: cases/canonical/aggregates/enum-dispatch gzip9 vs old: +1 -> +0
- loss fell: cases/canonical/aggregates/enum-dispatch brotli11 vs old: +13 -> +0
- loss fell: cases/canonical/control/expression-if raw vs old: +1 -> +0
- loss fell: cases/canonical/control/expression-if brotli11 vs old: +2 -> +0
- loss fell: cases/canonical/scalars/clamp raw vs old: +1 -> +0
- loss fell: cases/canonical/scalars/clamp brotli11 vs old: +2 -> +0
- loss fell: cases/catalog/function/factorial-5 brotli11 vs old: +1 -> +0
- loss fell: cases/catalog/function/factorial-6 brotli11 vs old: +1 -> +0
- loss fell: cases/catalog/function/factorial-7 brotli11 vs old: +1 -> +0

