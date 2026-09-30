# S1: bounded constant evaluation

Compiler source: `8826918a`. Release binary:
`~/lilscript-work/bin/s1-1/lilscript`, SHA-256
`4bd8b39b22560cd2024b77a5d2a6fac7a34a53e6e191382b0e4383397d3514f0`.
The comparison compiler is C1 (`8bb9dc3c`), pinned in the
[previous receipt](../2026-09-30-c1-controls/README.md).

The [full generic ratchet](ratchet.md) passes without new problems. Its
[JSON receipt](ratchet.json) covers 642 inputs and 1,926 lanes, with the
same 36 ledgered failed lanes. Deltas against C1/Y5:

| Corpus | Raw | Gzip | Brotli |
|---|---:|---:|---:|
| Cases | −176 | −157 | −145 |
| Applications | −27 | −15 | −20 |
| Algorithms | 0 | 0 | 0 |
| Total | **−203** | **−172** | **−165** |

The [production case matrix](cases.json) covers 401 cases × nine lanes
(script, module and native C under each objective), with no failure outside
the ledger. The [new-case matrix](new-cases.json) adds low-effort lanes and
covers original versus patched string methods, argument effects, captures,
UTF-16, integer radix conversion and signed zero. Host-prelude native lanes
are masked explicitly, not counted as passes.

The complete library run passed 1,649 tests, with nine existing ignores,
and exposed one recipe fixture that required original helper sites.
The fixture's test-only bypass now preserves those sites; it and all 39
rule tests pass on the final source. Independent interpreter/Node oracles
cover the new evaluator, including throwing and nonterminating inputs.
Native tests use the local Ubuntu Clang 18.1.3 toolchain.

The [paired compilation report](compile-cost.json), produced by
`scripts/compile-cost.py` from the [frozen manifest](compile-workloads.json),
retains three alternating measured pairs per workload after separate
warm-ups. Source inputs, resolved policies and codec identities match.
All three final artifacts are byte-identical between C1 and S1.

| Entry | Previous median CPU, s | Candidate median CPU, s | Ratio |
|---|---:|---:|---:|
| markedlil | 4.198 | 4.201 | 1.001 |
| zodlil | 7.566 | 7.565 | 1.000 |
| posthoglil | 0.773 | 0.783 | 1.013 |

These samples show similar compilation cost, not a general speed guarantee.
Warm-up phase telemetry reports elapsed time, separately from measured
process CPU. Markedlil still makes 49 terminal judgments and forms 86
targets; exact Brotli encoding takes about half its warm-up elapsed time
and formation about a third. C1 added independent pooling/packing moves;
this batch leaves those opportunities and the previous output intact.

S1 remains partial. Unsupported evaluation returns unknown; aggregates,
loops, broader propagation and objective-sensitive literal choices retain
their active owners. No default-policy calibration or complete runtime/fleet
qualification is claimed by these measurements.
