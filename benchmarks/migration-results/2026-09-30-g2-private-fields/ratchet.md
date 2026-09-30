# Generic corpus ratchet

Compiler `71b08d86a7d98476`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 122 | 624 | 53264 | 60614 |
| cases | raw | old | 518 | 624 | 53264 | 45935 |
| cases | gzip9 | competitor | 120 | 624 | 50204 | 56410 |
| cases | gzip9 | old | 501 | 622 | 50030 | 46478 |
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
| algorithms | raw | competitor | 1 | 11 | 5076 | 6210 |
| algorithms | raw | closure | 10 | 11 | 5076 | 4522 |
| algorithms | raw | old | 11 | 11 | 5076 | 3995 |
| algorithms | gzip9 | competitor | 2 | 11 | 3153 | 3440 |
| algorithms | gzip9 | closure | 9 | 11 | 3153 | 3031 |
| algorithms | gzip9 | old | 11 | 11 | 3153 | 2662 |
| algorithms | brotli11 | competitor | 4 | 11 | 2874 | 3039 |
| algorithms | brotli11 | closure | 8 | 11 | 2874 | 2703 |
| algorithms | brotli11 | old | 11 | 11 | 2874 | 2305 |

Failures (compile, crash or oracle): 0. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- loss count fell: cases brotli11 losses to competitor: 179 -> 178
- loss fell: cases/canonical/aggregates/class-counter raw vs old: +37 -> +21
- loss fell: cases/canonical/aggregates/class-counter gzip9 vs old: +22 -> +16
- loss fell: cases/canonical/aggregates/class-counter brotli11 vs old: +19 -> +13
- loss fell: cases/canonical/aggregates/class-scale raw vs old: +34 -> +19
- loss fell: cases/canonical/aggregates/class-scale gzip9 vs old: +22 -> +14
- loss fell: cases/canonical/aggregates/class-scale brotli11 vs old: +24 -> +19
- loss fell: cases/canonical/wins/aggregate-model raw vs old: +87 -> +71
- loss fell: cases/canonical/wins/aggregate-model gzip9 vs old: +65 -> +59
- loss fell: cases/canonical/wins/optimizer-pressure raw vs old: +21 -> +13
- loss fell: cases/canonical/wins/optimizer-pressure gzip9 vs old: +17 -> +12
- loss fell: cases/canonical/wins/optimizer-pressure brotli11 vs old: +6 -> +2
- loss fell: cases/catalog/aggregate/class-counter-0-8 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-0-8 gzip9 vs old: +24 -> +15
- loss fell: cases/catalog/aggregate/class-counter-0-8 brotli11 vs old: +28 -> +19
- loss fell: cases/catalog/aggregate/class-counter-3-10 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-3-10 gzip9 vs old: +21 -> +14
- loss fell: cases/catalog/aggregate/class-counter-3-10 brotli11 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/class-counter-5-12 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-5-12 gzip9 vs old: +23 -> +14
- loss fell: cases/catalog/aggregate/class-counter-5-12 brotli11 vs old: +25 -> +21
- loss fell: cases/catalog/aggregate/class-counter-7-9 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-7-9 gzip9 vs old: +24 -> +14
- loss fell: cases/catalog/aggregate/class-counter-7-9 brotli11 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/class-counter-11-11 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-11-11 gzip9 vs old: +23 -> +13
- loss fell: cases/catalog/aggregate/class-counter-11-11 brotli11 vs old: +32 -> +20
- loss fell: cases/catalog/aggregate/class-counter-2-15 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-2-15 gzip9 vs old: +23 -> +14
- loss fell: cases/catalog/aggregate/class-counter-2-15 brotli11 vs old: +25 -> +21
- loss fell: cases/catalog/aggregate/class-counter-13-8 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-13-8 gzip9 vs old: +23 -> +14
- loss fell: cases/catalog/aggregate/class-counter-4-14 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-4-14 gzip9 vs old: +22 -> +16
- loss fell: cases/catalog/aggregate/class-counter-4-14 brotli11 vs old: +27 -> +21
- loss fell: cases/catalog/aggregate/class-counter-9-13 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-9-13 gzip9 vs old: +23 -> +14
- loss fell: cases/catalog/aggregate/class-counter-9-13 brotli11 vs old: +25 -> +22
- loss fell: cases/catalog/aggregate/class-counter-1-16 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-1-16 gzip9 vs old: +23 -> +14
- loss fell: cases/catalog/aggregate/class-counter-1-16 brotli11 vs old: +32 -> +22
- loss fell: cases/catalog/aggregate/class-counter-8-7 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-8-7 gzip9 vs old: +24 -> +14
- loss fell: cases/catalog/aggregate/class-counter-8-7 brotli11 vs old: +26 -> +21
- loss fell: cases/catalog/aggregate/class-counter-6-18 raw vs old: +41 -> +21
- loss fell: cases/catalog/aggregate/class-counter-6-18 gzip9 vs old: +23 -> +14
- loss fell: cases/catalog/aggregate/class-counter-6-18 brotli11 vs old: +25 -> +21
- loss fell: cases/catalog/winner/win-aggregate-model raw vs old: +87 -> +71
- loss fell: cases/catalog/winner/win-aggregate-model gzip9 vs old: +65 -> +59
- loss fell: cases/catalog/winner/win-optimizer-pressure raw vs old: +21 -> +13
- loss fell: cases/catalog/winner/win-optimizer-pressure gzip9 vs old: +17 -> +12
- loss fell: cases/catalog/winner/win-optimizer-pressure brotli11 vs old: +6 -> +2
- loss fell: cases/catalog/winner/win-class-scale raw vs old: +34 -> +19
- loss fell: cases/catalog/winner/win-class-scale gzip9 vs old: +22 -> +14
- loss fell: cases/catalog/winner/win-class-scale brotli11 vs old: +24 -> +19
- loss fell: cases/catalog/aggregate/frontier-default-constructor raw vs old: +105 -> +77
- loss fell: cases/catalog/aggregate/frontier-default-constructor gzip9 vs old: +64 -> +59
- loss fell: cases/catalog/aggregate/frontier-default-constructor brotli11 vs old: +61 -> +46
- loss fell: cases/catalog/aggregate/frontier-inheritance-super raw vs old: +34 -> +10
- loss fell: cases/catalog/aggregate/frontier-inheritance-super gzip9 vs old: +21 -> +11
- loss fell: cases/catalog/aggregate/frontier-inheritance-super brotli11 vs old: +22 -> +9
- loss fell: apps/aggregate-model raw vs old: +87 -> +71
- loss fell: apps/aggregate-model gzip9 vs old: +65 -> +59
- loss fell: apps/optimizer-pressure raw vs old: +21 -> +13
- loss fell: apps/optimizer-pressure gzip9 vs old: +17 -> +12
- loss fell: apps/optimizer-pressure brotli11 vs old: +6 -> +2
- loss fell: cases/catalog/aggregate/frontier-default-constructor brotli11 vs competitor: +5 -> +0

