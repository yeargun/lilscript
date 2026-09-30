# Generic corpus ratchet

Compiler `0fd5430d4bc578ae`, 642 items, bars `770649437963ff5b`.

Delivery: classic script, explicit `bare`.

| Corpus | Metric | Bar | Losses | Items | Ours | Bar total |
|---|---|---|---:|---:|---:|---:|
| cases | raw | competitor | 122 | 624 | 53650 | 60614 |
| cases | raw | old | 518 | 624 | 53650 | 45935 |
| cases | gzip9 | competitor | 120 | 624 | 50369 | 56410 |
| cases | gzip9 | old | 501 | 622 | 50195 | 46478 |
| cases | brotli11 | competitor | 179 | 624 | 40449 | 44919 |
| cases | brotli11 | old | 521 | 623 | 40400 | 34677 |
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

Failures (compile, crash or oracle): 0. Verdict: **pass**.

## Improvements (tighten the baseline with --update-baseline)

- now passes: cases/canonical/collections/record-json raw (was refused)
- now passes: cases/canonical/collections/record-json gzip9 (was refused)
- now passes: cases/canonical/collections/record-json brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-1-left raw (was refused)
- now passes: cases/catalog/record/edge-record-json-1-left gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-1-left brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-2-right raw (was refused)
- now passes: cases/catalog/record/edge-record-json-2-right gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-2-right brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-3-middle raw (was refused)
- now passes: cases/catalog/record/edge-record-json-3-middle gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-3-middle brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-4-alpha raw (was refused)
- now passes: cases/catalog/record/edge-record-json-4-alpha gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-4-alpha brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-5-beta raw (was refused)
- now passes: cases/catalog/record/edge-record-json-5-beta gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-5-beta brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-6-gamma raw (was refused)
- now passes: cases/catalog/record/edge-record-json-6-gamma gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-6-gamma brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-7-delta raw (was refused)
- now passes: cases/catalog/record/edge-record-json-7-delta gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-7-delta brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-8-first raw (was refused)
- now passes: cases/catalog/record/edge-record-json-8-first gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-8-first brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-9-hot raw (was refused)
- now passes: cases/catalog/record/edge-record-json-9-hot gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-9-hot brotli11 (was refused)
- now passes: cases/catalog/record/edge-record-json-10-live raw (was refused)
- now passes: cases/catalog/record/edge-record-json-10-live gzip9 (was refused)
- now passes: cases/catalog/record/edge-record-json-10-live brotli11 (was refused)

