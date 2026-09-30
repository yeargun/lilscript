# S2: private record payloads

Source checkpoint `480337fc` includes record analysis v3, helper analysis v10 and the corrected reconstruction allocation owner. It excludes subsequent S1 loop evaluation and policy algorithm 3. Pinned compiler `~/lilscript-work/bin/s2-payload-1/lilscript` SHA-256 `de7b5b00c457b628293a2866a602d114e85ba5a5f93841aa2110308684d20499`; codec SHA-256 `977ab4afa57d7700dc2598a62331073a3ce92725275d28962c93f20b40a9e6d2`.

The private record and captured-helper proofs now admit checked non-integer payloads. Complete uses, constant keys, single initialization, dominance and capture timing remain required. Aliases, escapes and dynamic keys remain ineligible. Raw payload domains and value-copy semantics are independently proved.

Focused verification passes 47 tests, including independent JavaScript oracles for strings, booleans, NaN/negative zero, opaque and array references, absent/undefined normalization, value-struct copies and separate activations. Public service tests exercise on/off scalar permissions and exact delivered scores for every objective. The full library checkpoint passed 1,664 tests with nine existing ignores and the previously passing expensive table case excluded.

The [generic ratchet](ratchet.md), with [complete receipt](ratchet.json), passes 642 inputs × three objectives and the same 36 ledgered failures. Every passing lane retains its G1 size. The [maintained port suites](ports.json) pass markedlil 29/29, zodlil 1,353/1,353 and posthoglil 21/21 across nine artifacts.

[Paired compilation samples](compile-cost.json) compare the isolated C1 correction against S2 on the [frozen manifest](../2026-09-30-s1-evaluation/compile-workloads.json), three alternating measured pairs after warm-up. All three selected artifacts are byte-identical.

| Entry | Median CPU ratio | Brotli bytes |
|---|---:|---:|
| markedlil | 0.993 | 9,158 |
| zodlil | 0.979 | 26,642 |
| posthoglil | 1.001 | 5,224 |

These slices add coverage, not a measured size win in the existing corpus or ports. The dedicated scalar-veto test demonstrates new alternatives on a private string record. Broader escape/alias/field work remains open; functional success and these compiler timings do not establish runtime-speed parity.
