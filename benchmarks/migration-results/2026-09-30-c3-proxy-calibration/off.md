# Generic corpus ratchet

Compiler `475f66d876a10a83`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 122 | 624 | 53264 | 60614 |
| cases | raw | old | 518 | 624 | 53264 | 45935 |
| cases | gzip9 | competitor | 120 | 624 | 50194 | 56410 |
| cases | gzip9 | old | 500 | 622 | 50020 | 46478 |
| cases | brotli11 | competitor | 178 | 624 | 40330 | 44919 |
| cases | brotli11 | old | 521 | 623 | 40281 | 34677 |
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
| algorithms | gzip9 | competitor | 0 | 11 | 3127 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3127 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3127 | 2662 |
| algorithms | brotli11 | competitor | 3 | 11 | 2849 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2849 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2849 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases gzip9 losses to old: 501 -> 500
- loss fell: cases/catalog/collection/minmax-scan-2-7-1-8-2-8 gzip9 vs old: +8 -> +7
- loss fell: cases/catalog/collection/edge-array-pipeline gzip9 vs old: +13 -> +10
- loss fell: cases/catalog/effect/edge-generator-range-1n7n2 gzip9 vs competitor: +11 -> +10
- loss fell: cases/catalog/effect/edge-generator-range-1n7n2 gzip9 vs old: +10 -> +9
- loss fell: cases/catalog/effect/edge-generator-range-2n10n2 gzip9 vs competitor: +11 -> +10
- loss fell: cases/catalog/effect/edge-generator-range-2n10n2 gzip9 vs old: +10 -> +9
- loss fell: cases/catalog/control/enum-int-dispatch-10 gzip9 vs old: +2 -> +0

