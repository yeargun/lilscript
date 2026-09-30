# S1: bounded constant loops

Source checkpoint `50d2369b` extends the primitive evaluator to completed structured loops, including nested exits, update expressions and local initialization. Unsupported effects, unknown captures, exceptions and exhausted evaluation preserve the call. The existing shared 1,024-step, eight-call-depth and 64-operation limits are unchanged.

Pinned compiler `~/lilscript-work/bin/s1-loops-1/lilscript` SHA-256 `5a1b5da099dfca52bd028f6b905aa75ddb4ec59e12e2527ca21c74a91b7f73f5`; codec SHA-256 `8c75e173201806aab6eb4847dba17ab22973e5aa2f76bb51a97dfc83dc11b273`. This includes S2 payload proofs and the reconstruction ownership correction, and excludes subsequent producer-policy and private script-container changes.

All 42 shared-rule tests and the public JavaScript/native permission test pass. The interpreter provides independent execution comparisons for supported loops; a direct Node oracle covers exceptions the interpreter cannot execute. Later C1's complete library run also covers this checkpoint.

The [generic ratchet](ratchet.md), with [full receipt](ratchet.json), passes 642 inputs × three objectives and the same 36 ledgered failures. Against the previously accepted G1 baseline:

| Corpus | Raw delta | Gzip delta | Brotli delta |
|---|---:|---:|---:|
| Cases | −4,756 | −3,966 | −3,414 |
| Applications | −231 | −177 | −160 |
| Algorithms | 0 | 0 | 0 |
| Total | **−4,987** | **−4,143** | **−3,574** |

The [supported baseline-update run](ratchet-baseline.json) independently passes and records these results. Both runs use the then-current bare classic-script contract. The subsequent D1 private wrapper requires a separately identified comparison.

The [reference port suites](ports.json) pass markedlil 29/29, zodlil 1,353/1,353 and posthoglil 21/21 across nine artifacts. [Three alternating measured compilation pairs](compile-cost.json), after warm-up on the [frozen manifest](../2026-09-30-s1-evaluation/compile-workloads.json), compare S2 against S1:

| Entry | Median CPU ratio | Raw / gzip / Brotli bytes |
|---|---:|---:|
| markedlil | 0.986 | 36,243 / 10,209 / 9,158 |
| zodlil | 1.017 | 89,496 / 29,815 / 26,642 |
| posthoglil | 1.012 | 16,936 / 5,833 / 5,224 |

All three selected port outputs are byte-identical before and after. The small timing differences overlap sample spread; no compiler-speed gain is claimed. These functional checks do not establish runtime-speed parity. Broader S1 propagation, defaults and call evaluation remain open.
