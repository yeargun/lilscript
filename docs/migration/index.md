# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- **C1, C2 and C3 are complete. S1 implementation is complete.** Its generic behavior, full language matrix and reference ports are checked; final library/CLI reruns and the complete compile-cost study are explicitly deferred at the owner's request. [Evidence](../../benchmarks/migration-results/2026-10-01-s1-complete/README.md).
- **S2 implementation is complete**, including its controls and focused JavaScript/native checks. [Evidence](../../benchmarks/migration-results/2026-09-30-s2-complete/README.md). **S3 is complete** under its [acceptance record](s3-acceptance.md). **Q1 is complete** under its [acceptance record](q1-acceptance.md). **G1 is complete** under its [contract](g1-acceptance.md). **G2 is complete** under its [contract](g2-acceptance.md). **G3 is complete**, under its [contract](g3-acceptance.md). **S4 is complete** under its [contract](s4-acceptance.md) and explicit [public ABI scope](s4-public-abi.md). **Q2 is complete** under its [acceptance contract](q2-acceptance.md), including the explicit reuse/resource scope correction. **Q3 is complete** under its [acceptance contract](q3-acceptance.md). **Q4 is complete** under its [acceptance contract](q4-acceptance.md). **D1 is complete** under its [acceptance contract](d1-acceptance.md). **D2 is complete** under its [acceptance contract](d2-acceptance.md). **D3 is complete** under its [acceptance contract](d3-acceptance.md). **N1 is complete** under its [acceptance contract](n1-acceptance.md). **N2 is the sole active milestone**.
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

Complete **N2**: remaining native language/runtime/ABI support and performance
qualification. Then follow **V1 → V2**.

Q2's safe reuse boundaries and D3's shared graph/session integration are qualified.
D3 implements bounded, conservatively invalidated per-module elaboration reuse. Arbitrary
changed-source-unit fragment relocation is not implemented or claimed: validity
requires the complete cross-unit dependency closure, as corrected in the plan.
Resource receipts expose partial frontend/target allocator coverage.

S4's implemented author controls, fact consumers and supported ABI remain
qualified in [history](history.md). Unrestricted public erased/identity/mutable
collection ABIs are explicitly unimplemented future proposals under the
[scope decision](s4-public-abi.md), not hidden completion claims.

The owner requests substantial implementation batches followed by focused
checks that answer a correctness or size question. Do not restart the deferred
S1 qualification chain or repeatedly run the entire library suite. Preserve
verification limits honestly and use the [single milestone order](plan.md#8-working-order).

## Work packages

| State | Task | Still to complete |
|---|---|---|
| [x] | [C1](plan.md#c1) Controls | Complete; new producers register their controls within their own milestone |
| [x] | [C2](plan.md#c2) Evidence | Complete; each later optimization supplies its needed independent oracle and affected-workload qualification |
| [x] | [C3](plan.md#c3) Calibration | Complete; later policy changes belong to Q3 and external-library qualification remains V1 |
| [x] | [S1](plan.md#s1) Folding | Implementation complete; final library/CLI and complete paired cost verification deferred by owner |
| [x] | [S2](plan.md#s2) Objects | Bounded aggregate facts, fields, flattening, scalar banks and physical copy elision complete; focused evidence retained |
| [x] | [S3](plan.md#s3) Calls | Cross-module/nested-closure inlining, terminal returns, frequency and judged specialization/sharing complete |
| [x] | [S4](plan.md#s4) Contracts | Checked contracts, consumers/removals and author controls complete within the explicit public ABI; staged defaults remain V2 |
| [x] | [G1](plan.md#g1) Lexical names | Live printed-order/frequency allocation, full continuations, final-byte ties and replay controls complete; optional compaction is automatic at 14 |
| [x] | [G2](plan.md#g2) Property names | Private generic/observed eligibility, inherited/sibling slot reuse, reflected boundaries and adapter-name fixtures complete |
| [x] | [G3](plan.md#g3) JS generation | Stable per-site forms, guarded aliases, safe locality choices and structural printing complete |
| [x] | [Q1](plan.md#q1) Choices | Common source/target family contract, per-site call choices, bounded joint moves and replay permissions complete |
| [x] | [Q2](plan.md#q2) Reuse | Qualified dirty dependencies, shared/changed-site formation, persistent reuse and counted owners; scope correction explicit |
| [x] | [Q3](plan.md#q3) Search policy | Independent objective priors/estimates, bounded assignment evidence, protected effort and deterministic file workers complete |
| [x] | [Q4](plan.md#q4) Data | Checked const/define evaluation, static schemas, owned helpers and measured eager/lazy encodings complete within the acceptance scope |
| [x] | [D1](plan.md#d1) Formats | CJS/library-IIFE/UMD/bare, live linkage and exact manifest delivery complete |
| [x] | [D2](plan.md#d2) Consumers | Consumer controls, initialization/lazy cycles, typed hosts, public const graphs and source maps complete |
| [x] | [D3](plan.md#d3) Integration | Shared configured graph/editor/reuse, independent multi-format output and source-owned compiler-written port delivery complete |
| [ ] | [V1](plan.md#v1) Qualification | Maintained/held-out size and runtime gaps, independently per objective |
| [ ] | [V2](plan.md#v2) Release | Reproducible packages, sites and current receipts |
| [x] | [N1](plan.md#n1) Native integration | Shared optimized input, initialization proofs, toolchain/configuration owner and source-qualified capabilities complete |
| [~] | [N2](plan.md#n2) Native completion | Ownership, portable data batches, polymorphic arrays, exception completions and Unicode/typed regex implemented; remaining data/suspension/host ABI and performance qualification |

## Working rules

- One active milestone and one heavy build, test or measurement job at a time. Implement substantial coherent batches before focused checks; do not repeat full library qualification for small changes. The owner has deferred S1’s remaining broad reruns.
- Size comes first at default effort 13 and above; expensive strategies with small measured returns may default to 14 or 15 with explicit overrides. Document every control's situation, defaults/gates and size/compile/runtime tradeoffs. Judge batch totals separately by objective; explain small local heuristic regressions and additional search cost.
- Every open defect has an owner in [coverage.md](coverage.md). Native completion remains owned separately from JavaScript qualification.
- Keep `ref`, public constructibility and `pure` termination semantics explicit while their language decisions remain open. D1 now supplies Y5's approved application IIFE default.
- On landing, update this checklist and add one evidence record to history. Do not copy batch measurements or old chronology into this file.
