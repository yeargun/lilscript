# Generic corpus ratchet

Compiler `0e53dce81d3cb28d`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 111 | 613 | 50371 | 58402 |
| cases | raw | old | 507 | 613 | 50371 | 44247 |
| cases | gzip9 | competitor | 109 | 613 | 48147 | 54771 |
| cases | gzip9 | old | 490 | 611 | 47973 | 45258 |
| cases | brotli11 | competitor | 168 | 613 | 38600 | 43620 |
| cases | brotli11 | old | 510 | 612 | 38551 | 33697 |
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
| algorithms | gzip9 | competitor | 2 | 11 | 3153 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3153 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3153 | 2662 |
| algorithms | brotli11 | competitor | 4 | 11 | 2874 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2874 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2874 | 2305 |

Failures (compile, crash or oracle): 33. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: algorithms brotli11 losses to closure: 9 -> 8
- loss fell: cases/canonical/wins/point-batch brotli11 vs competitor: +6 -> +5
- loss fell: cases/canonical/wins/point-batch brotli11 vs old: +21 -> +20
- loss fell: cases/catalog/aggregate/struct-pairs-2-7-1-8-2-8 brotli11 vs competitor: +4 -> +3
- loss fell: cases/catalog/aggregate/struct-pairs-2-7-1-8-2-8 brotli11 vs old: +36 -> +35
- loss fell: cases/catalog/aggregate/struct-pairs-9-8-7-6-5-4-3-2 brotli11 vs competitor: +6 -> +5
- loss fell: cases/catalog/aggregate/struct-pairs-9-8-7-6-5-4-3-2 brotli11 vs old: +37 -> +36
- loss fell: cases/catalog/aggregate/nested-struct-1-2-3-4-5-6-7-8 brotli11 vs old: +24 -> +23
- loss fell: cases/catalog/aggregate/nested-struct-9-8-7-6-5-4-3-2 brotli11 vs old: +25 -> +24
- loss fell: cases/catalog/aggregate/nested-struct-0-1-0-2-0-3-0-4 brotli11 vs old: +28 -> +22
- loss fell: cases/catalog/winner/win-point-batch brotli11 vs competitor: +6 -> +5
- loss fell: cases/catalog/winner/win-point-batch brotli11 vs old: +21 -> +20
- loss fell: cases/catalog/closure/edge-closure-mutation-5n3n6 gzip9 vs old: +4 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-7nn2n7 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-20nn3n4 brotli11 vs competitor: +7 -> +4
- loss fell: cases/catalog/closure/edge-closure-mutation-20nn3n4 brotli11 vs old: +14 -> +11
- loss fell: algorithms/aggregate-ledger gzip9 vs closure: +6 -> +5
- loss fell: algorithms/aggregate-ledger gzip9 vs old: +47 -> +46
- loss fell: algorithms/collection-geometry brotli11 vs competitor: +2 -> +1
- loss fell: algorithms/collection-geometry brotli11 vs closure: +15 -> +14
- loss fell: algorithms/collection-geometry brotli11 vs old: +72 -> +71
- loss fell: algorithms/dictionary-template-router gzip9 vs closure: +2 -> +1
- loss fell: algorithms/dictionary-template-router gzip9 vs old: +38 -> +37
- loss fell: algorithms/state-machine-parser brotli11 vs old: +41 -> +40
- loss fell: algorithms/static-rule-engine gzip9 vs old: +21 -> +19
- loss fell: algorithms/static-rule-engine brotli11 vs closure: +8 -> +4
- loss fell: algorithms/static-rule-engine brotli11 vs old: +30 -> +26
- loss fell: algorithms/state-machine-parser brotli11 vs closure: +1 -> +0

