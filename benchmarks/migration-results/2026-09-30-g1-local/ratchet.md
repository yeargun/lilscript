# Generic corpus ratchet

Compiler `3421b4f2a2ca6d20`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 111 | 613 | 50371 | 58402 |
| cases | raw | old | 507 | 613 | 50371 | 44247 |
| cases | gzip9 | competitor | 109 | 613 | 48146 | 54771 |
| cases | gzip9 | old | 490 | 611 | 47972 | 45258 |
| cases | brotli11 | competitor | 168 | 613 | 38603 | 43620 |
| cases | brotli11 | old | 510 | 612 | 38554 | 33697 |
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
| algorithms | gzip9 | competitor | 2 | 11 | 3155 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3155 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3155 | 2662 |
| algorithms | brotli11 | competitor | 3 | 11 | 2874 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2874 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2874 | 2305 |

Failures (compile, crash or oracle): 33. Verdict: **fail**.

## Blocking

- loss grew: algorithms/collection-geometry gzip9 vs closure: +24 -> +26 (ours 236, bar 210)
- loss grew: algorithms/collection-geometry gzip9 vs old: +63 -> +65 (ours 236, bar 171)

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: algorithms brotli11 losses to competitor: 4 -> 3
- loss count fell: algorithms brotli11 losses to closure: 9 -> 8
- loss fell: cases/catalog/aggregate/nested-struct-1-2-3-4-5-6-7-8 brotli11 vs old: +24 -> +23
- loss fell: cases/catalog/aggregate/nested-struct-9-8-7-6-5-4-3-2 brotli11 vs old: +25 -> +24
- loss fell: cases/catalog/closure/edge-closure-mutation-1n2n4 brotli11 vs competitor: +3 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-1n2n4 brotli11 vs old: +10 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-3nn1n5 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-5n3n6 gzip9 vs old: +4 -> +3
- loss fell: cases/catalog/closure/edge-closure-mutation-7nn2n7 gzip9 vs old: +3 -> +2
- loss fell: cases/catalog/closure/edge-closure-mutation-11n4n5 brotli11 vs competitor: +4 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-11n4n5 brotli11 vs old: +11 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-20nn3n4 brotli11 vs competitor: +7 -> +4
- loss fell: cases/catalog/closure/edge-closure-mutation-20nn3n4 brotli11 vs old: +14 -> +11
- loss fell: cases/catalog/closure/edge-closure-mutation-9n1n8 brotli11 vs competitor: +3 -> +1
- loss fell: cases/catalog/closure/edge-closure-mutation-9n1n8 brotli11 vs old: +10 -> +8
- loss fell: cases/catalog/closure/edge-closure-mutation-n8nn2n6 brotli11 vs competitor: +7 -> +6
- loss fell: cases/catalog/closure/edge-closure-mutation-n8nn2n6 brotli11 vs old: +14 -> +13
- loss fell: algorithms/aggregate-ledger gzip9 vs closure: +6 -> +5
- loss fell: algorithms/aggregate-ledger gzip9 vs old: +47 -> +46
- loss fell: algorithms/aggregate-ledger brotli11 vs closure: +11 -> +10
- loss fell: algorithms/aggregate-ledger brotli11 vs old: +53 -> +52
- loss fell: algorithms/collection-geometry brotli11 vs competitor: +2 -> +1
- loss fell: algorithms/collection-geometry brotli11 vs closure: +15 -> +14
- loss fell: algorithms/collection-geometry brotli11 vs old: +72 -> +71
- loss fell: algorithms/dictionary-template-router gzip9 vs closure: +2 -> +1
- loss fell: algorithms/dictionary-template-router gzip9 vs old: +38 -> +37
- loss fell: algorithms/state-machine-parser brotli11 vs old: +41 -> +40
- loss fell: algorithms/static-rule-engine gzip9 vs old: +21 -> +19
- loss fell: algorithms/string-dictionary brotli11 vs old: +44 -> +41
- loss fell: algorithms/state-machine-parser brotli11 vs closure: +1 -> +0
- loss fell: algorithms/string-dictionary brotli11 vs competitor: +2 -> +0

