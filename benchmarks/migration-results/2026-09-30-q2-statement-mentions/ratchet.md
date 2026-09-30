# Generic corpus ratchet

Compiler `a5da0201f944923b`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 121 | 624 | 53244 | 60614 |
| cases | raw | old | 518 | 624 | 53244 | 45935 |
| cases | gzip9 | competitor | 119 | 624 | 50179 | 56410 |
| cases | gzip9 | old | 500 | 622 | 50005 | 46478 |
| cases | brotli11 | competitor | 178 | 624 | 40314 | 44919 |
| cases | brotli11 | old | 521 | 623 | 40265 | 34677 |
| apps | raw | competitor | 0 | 7 | 835 | 1527 |
| apps | raw | closure | 1 | 7 | 835 | 1332 |
| apps | raw | old | 5 | 7 | 835 | 802 |
| apps | gzip9 | competitor | 1 | 7 | 727 | 1053 |
| apps | gzip9 | closure | 2 | 7 | 727 | 982 |
| apps | gzip9 | old | 5 | 7 | 727 | 711 |
| apps | brotli11 | competitor | 0 | 7 | 574 | 870 |
| apps | brotli11 | closure | 2 | 7 | 574 | 834 |
| apps | brotli11 | old | 5 | 7 | 574 | 558 |
| algorithms | raw | competitor | 0 | 11 | 4920 | 6210 |
| algorithms | raw | closure | 10 | 11 | 4920 | 4522 |
| algorithms | raw | old | 11 | 11 | 4920 | 3995 |
| algorithms | gzip9 | competitor | 0 | 11 | 3125 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3125 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3125 | 2662 |
| algorithms | brotli11 | competitor | 3 | 11 | 2844 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2844 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2844 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- loss fell: cases/catalog/closure/edge-closure-mutation-3nn1n5 gzip9 vs old: +3 -> +2
