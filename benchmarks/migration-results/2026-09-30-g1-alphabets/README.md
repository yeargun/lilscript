# G1: measured identifier alphabets

Source `a605cdc9`, pinned release `~/lilscript-work/bin/g1-1/lilscript`,
SHA-256 `6786095b8b11cf16a4fd7a960123015116e24246d0a05e4d4ba080535975e5a5`.
Codec SHA-256 `09dc7591f4b563283dff1148fc967f69c121f101420b36a2e97da784a7cd875e`.
The encoder libraries and settings are unchanged.

The sequential alphabet remains the allocator seed. Walk schedule v3 offers
a frequency-ranked alphabet as a joint move, governed by `naming-alphabet`,
`naming-search` and `identifier-mangling`. Only exact whole-artifact improvement
keeps it. The retained plan, memo identity and admission evidence include the
alphabet. This batch also contains Q2's output-owner lifetime repair.

The [generic ratchet](ratchet.md), with its [complete receipt](ratchet.json),
passes all 1,926 lanes across 642 inputs or matches the same 36 ledgered
failures. There are no new regressions. The supported baseline-update run
accepts these totals against Q2/S1:

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Cases | 0 | −319 | −529 |
| Applications | 0 | −11 | −6 |
| Algorithms | 0 | −38 | −20 |
| Total | 0 | −368 | −555 |

The [perturbation report](perturbations.json) retains nine generated sources:
12/56/80 captured bindings, each with three identifier spellings. All 54
artifacts (three objectives, alphabet permission on/off) pass an independent
Node arithmetic and host-event oracle with two separate closure activations.
Source, configuration, hashes and all metric sizes are retained. These are
development cases, not held-out evidence. Selected-objective deltas total
0 raw, −6 gzip and −10 Brotli bytes, with no cell regression.

The [paired CPU report](compile-cost.json) compares Q2 with G1 using the
frozen main-entry manifest, three alternating measured pairs after warm-up,
and identical contracts, inputs, effort and codec settings. The runner records
the intentional tactic/schedule change explicitly; this is a compiler-strategy
comparison, not an identical-effective-policy cache benchmark.

| Entry | CPU ratio | Brotli before → after | Formation calls | Exact Brotli calls |
|---|---:|---:|---:|---:|
| markedlil | 1.044 | 9,158 → 9,158 | 86 → 89 | 51 → 54 |
| zodlil | 1.024 | 26,645 → 26,645 | 44 → 45 | 30 → 31 |
| posthoglil | 3.391 | 5,235 → 5,185 | 36 → 125 | 22 → 78 |

Markedlil and zodlil remain byte-identical. Posthoglil also loses 198 raw and
56 gzip bytes in its Brotli-selected artifact. Its alphabet improvement exposes
further improving structural/spelling moves: two passes become five and 19
terminal exact judgments become 75. This is a substantial CPU tradeoff for
50 Brotli bytes, not a speed improvement. The explicit family veto disables
this exploration; Q2 still owns formation reuse and Q3 calibration of effort.
Median CPU seconds are 4.262 → 4.448, 7.618 → 7.802 and 0.766 → 2.596;
the report retains every sample and its spread.

Compiler verification: combined full library run and one repaired handoff
assertion rerun give 1,658 passing tests, with nine existing ignores. This
includes hygiene/captures, public names, each objective's exact delivery,
dedicated and parent vetoes, retained replay, deterministic search and codec
cache work/lifetime checks. Python policy-comparison tests pass three cases.
The [maintained port suite report](ports.json) passes markedlil **29/29**,
zodlil **1,353/1,353** and posthoglil **21/21** tests. The runner copied the
already-migrated checkouts, applied no historical patches, and built all nine
declared artifacts with the pinned compiler. These suites qualify behavior;
they do not establish runtime-speed parity or replace full fleet qualification.
