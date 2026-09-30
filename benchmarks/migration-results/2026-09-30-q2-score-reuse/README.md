# Q2: reuse completed compression measurements

Compiler source: `11f249c3`. Pinned release binary
`~/lilscript-work/bin/q2-1/lilscript`, SHA-256
`1ace4226c23f44eda3c498018869a4eff3c7e3be31340dd671718f07f3d5e4e6`.
Codec binary SHA-256:
`46d5df5ad1776625d7d38e32b68aa4f0da7dbdd57dcb996fe3a27e7cd5346307`.
The encoder libraries/settings are unchanged; the codec executable's linked
measurement implementation changed.

The [generic ratchet](ratchet.md) and [complete receipt](ratchet.json) pass:
642 inputs, 1,926 lanes, the same 36 ledgered failures, unchanged totals and
unchanged artifact hash prefixes in every passing lane against S1. The
accepted baseline remains S1.

The [paired CPU report](compile-cost.json) compares the pinned S1 binary
with Q2 on the [same frozen manifest](../2026-09-30-s1-evaluation/compile-workloads.json).
Three alternating measured pairs per workload follow separate warm-ups;
every sample, policy/input identity and artifact is retained. All three
final artifacts are byte-identical between versions.

| Entry | S1 median CPU, s | Q2 median CPU, s | Ratio | Exact Brotli encodes | Proxy encodes |
|---|---:|---:|---:|---:|---:|
| markedlil | 4.200 | 4.065 | 0.968 | 55 → 51 | 61 → 57 |
| zodlil | 7.556 | 7.571 | 1.002 | 30 → 30 | 39 → 39 |
| posthoglil | 0.776 | 0.770 | 0.993 | 22 → 22 | 27 → 27 |

Markedlil reuses eight measurements; the other two have no hits. Its retained
sample ranges are 4.180–4.213 s before and 4.061–4.084 s after. Formation
counts remain 86, 44 and 36 respectively. The measured saving is modest:
most of markedlil's second walk examines distinct byte streams. This cache
does not remove its useful search opportunities or explain away the
remaining formation cost.

Verification: 51 relevant codec, artifact, search and public-service tests
pass, with one existing ignore. The explicit cold/reused audit exercises
real hits and preserves logical work, decisions and bytes for all objectives.
Resource tests cover cold work/scratch admission; table tests cover byte,
settings/role separation, eviction and bounded retained storage.

`scripts/verify-bundles.mjs` passes with the local Clang 18.1.3 toolchain.
Initially its exact placement assertions failed in `split` and `lazy-cycle`:
S1's constant-call evaluation had removed their shared helper files. The
failure reproduces with the S1 binary. Those placement fixtures now disable
constant folding, alongside the existing split inlining veto, so they retain
the calls whose placement they test. Behavioral expectations and expected
shared-file plans are unchanged; the relocated CLI copy uses the same veto.

Q2 remains partial: dirty scheduling, reusable formation, broader resource
accounting and persistent-cache/decision-lock interfaces remain open.
