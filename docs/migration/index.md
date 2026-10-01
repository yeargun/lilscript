# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- **C1, C2 and C3 are complete. S1 implementation is complete.** Its generic behavior, full language matrix and reference ports are checked; final library/CLI reruns and the complete compile-cost study are explicitly deferred at the owner's request. [Evidence](../../benchmarks/migration-results/2026-10-01-s1-complete/README.md).
- **S2 implementation is complete**, including its controls and focused JavaScript/native checks. [Evidence](../../benchmarks/migration-results/2026-09-30-s2-complete/README.md). **S3 is complete** under its [acceptance record](s3-acceptance.md). **Q1 is complete** under its [acceptance record](q1-acceptance.md). **G1 is complete** under its [contract](g1-acceptance.md). **G2 is complete** under its [contract](g2-acceptance.md). **G3 is complete**, under its [contract](g3-acceptance.md). **S4 is complete** under its [contract](s4-acceptance.md) and explicit [public ABI scope](s4-public-abi.md). **Q2 is the sole active milestone.**
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

Complete **Q2** under its [acceptance contract](q2-acceptance.md): dirty
scheduling, shared formation, persistent transparent reuse and resource
accounting. The rooted use-index consumer and admitted module graph are
implemented. Edit journals, in-place renumbering and complete retained target copies have admitted storage;
local normalization proofs, dependency-qualified effect components and optional
disk codec/whole-build reuse and explicit decision locks are qualified. Delivery
planning, trial copies, simulation and setter payloads are admitted; print proofs
are revalidated after creating setters. Source arena edits and shared copies are
admitted before allocation, with exact publication checks. Rendered bundles and
artifact metadata have complete reservations and refusal cleanup. Repeated
family tails now share immutable targets under deterministic admission stages;
changed-unit formation and per-module elaboration remain open. Graph, effect and range
analyses are admitted through source normalization, pure-contract checks and JavaScript
demand. Primitive classes, aggregate origins, activation/default transport and
forwarding/dead-code, folding, signature, inline and aggregate rewrite plans are admitted.
Exact-value evaluation, local-fact initialization and local normalizer plans use
their bounded owners; stable local proofs replay identical logical stages in both reuse modes.
Primitive classes now propagate through explicit bounded dependencies instead of global sweeps.
Remaining dirty dependencies, incremental formation and per-module elaboration stay open. Keep logical admission, hard TOML
permissions, source/target identities and selected output independent of cache
occupancy. Then follow **Q3 → Q4 → D1 → D2 → D3 → N1 → N2 → V1 → V2**.

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
| [~] | [Q2](plan.md#q2) Reuse | Dirty scheduling, incremental formation, persistent reuse and resource completion |
| [~] | [Q3](plan.md#q3) Search policy | Gzip's policy, calibrated estimates/proxies and useful effort progression |
| [~] | [Q4](plan.md#q4) Data | Const data, helpers, encodings and schema specialization |
| [~] | [D1](plan.md#d1) Formats | CJS/library-IIFE/UMD and remaining manifests |
| [~] | [D2](plan.md#d2) Consumers | Shakeability, placement, lazy effects/cycles, host modules and source maps |
| [~] | [D3](plan.md#d3) Integration | Remaining graph consumers and compiler-written port builds; independent TOML/API/CLI objective sets implemented |
| [ ] | [V1](plan.md#v1) Qualification | Maintained/held-out size and runtime gaps, independently per objective |
| [ ] | [V2](plan.md#v2) Release | Reproducible packages, sites and current receipts |
| [~] | [N1](plan.md#n1) Native integration | Optimized shared program, toolchain owner and capabilities |
| [ ] | [N2](plan.md#n2) Native completion | Remaining language/runtime/ABI coverage and performance qualification |

## Working rules

- One active milestone and one heavy build, test or measurement job at a time. Implement substantial coherent batches before focused checks; do not repeat full library qualification for small changes. The owner has deferred S1’s remaining broad reruns.
- Size comes first at default effort 13 and above; expensive strategies with small measured returns may default to 14 or 15 with explicit overrides. Document every control's situation, defaults/gates and size/compile/runtime tradeoffs. Judge batch totals separately by objective; explain small local heuristic regressions and additional search cost.
- Every open defect has an owner in [coverage.md](coverage.md). Native completion remains owned separately from JavaScript qualification.
- Keep `ref`, public constructibility and `pure` termination semantics explicit while their language decisions remain open. D1 now supplies Y5's approved application IIFE default.
- On landing, update this checklist and add one evidence record to history. Do not copy batch measurements or old chronology into this file.
