# Scoped target reach: accounting correction, with compile cost

Source `82fdf87d11aa903ffcfb4565314ad56c7a07f4c7`, policy algorithm 13,
compared with the qualified objective-set pin `1484b18d` (algorithm 10).
`identity.json` records immutable compiler/codec hashes; `compile-cost.json`
retains all three alternating pairs and warmups per workload. Frozen effort-13
inputs and encoder settings match.

All nine output hashes, complete search records and exact-codec work match.
The canonical policies differ only in algorithm version; every handoff releases
all retained storage. `summarize.py` verifies these claims and writes
`comparison.json` with sample ranges, logical work, phase elapsed measurements
and peak accounted memory. Accounted bytes are not process RSS.

This is **not a speed improvement**. The change admits previously unaccounted
reach storage, avoids unused capture facts and releases every consumer's
analysis at the end of its real lifetime. Its timing cost is retained explicitly.
The Zod gzip median improvement has overlapping ranges and is not a speed claim.

| Workload | Candidate / previous CPU | Sample ranges overlap | Peak accounted byte delta |
|---|---:|---|---:|
| markedlil-raw | 1.054 | no | +3164 |
| markedlil-gzip | 1.051 | no | +3164 |
| markedlil-brotli | 1.013 | no | +0 |
| zodlil-raw | 1.029 | yes | +8457 |
| zodlil-gzip | 0.926 | yes | +8457 |
| zodlil-brotli | 1.049 | yes | +0 |
| posthoglil-raw | 1.066 | no | +1386 |
| posthoglil-gzip | 1.117 | yes | +0 |
| posthoglil-brotli | 1.009 | yes | +0 |

All 1,758 ordinary active library tests passed on this source, with four existing
ignores and the large data-table oracle filtered. Comprehensive size, CLI and
large-table verification is combined with the subsequent C1 completion
checkpoint rather than repeated for this intermediate pin. This record alone
does not qualify the whole migration or the complete Q2 milestone.
