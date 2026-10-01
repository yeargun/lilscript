# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- **C1, C2 and C3 are complete. S1 implementation is complete.** Its generic behavior, full language matrix and reference ports are checked; final library/CLI reruns and the complete compile-cost study are explicitly deferred at the owner's request. [Evidence](../../benchmarks/migration-results/2026-10-01-s1-complete/README.md).
- **S2 implementation is complete**, including its controls and focused JavaScript/native checks. [Evidence](../../benchmarks/migration-results/2026-09-30-s2-complete/README.md). **S3 is complete** under its [acceptance record](s3-acceptance.md). **Q1 is complete** under its [acceptance record](q1-acceptance.md). **G1 is complete** under its [contract](g1-acceptance.md). **G2 is complete** under its [contract](g2-acceptance.md). **G3 is complete**, under its [contract](g3-acceptance.md). **S4 is the sole active milestone**, under its [contract](s4-acceptance.md).
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

Complete **S4**: identities/defaults, the record fact/type fix, typed
receivers/rest, checked array reads and value updates are implemented in its
[contracts batch](../../benchmarks/migration-results/2026-10-01-s4-contracts/README.md).
Continue its remaining shared-consumer and language/boundary work before Q2.
Concrete generic structs, nullable/default/rest value adapters, published
constructor/prototype adapters and dual type/constructor module bindings are
implemented in the [boundaries batch](../../benchmarks/migration-results/2026-10-01-s4-boundaries/README.md).
Host callable defaults/rest/receivers, generic/nullable `JS.assume` and zero-use
`object` singleton retirement are implemented in the
[host batch](../../benchmarks/migration-results/2026-10-01-s4-host-boundaries/README.md).
Checker-owned definite assignment and the explicit TOML field contract are
implemented in the [field batch](../../benchmarks/migration-results/2026-10-01-s4-field-contracts/README.md).
Native UTF-16 indexing, binary nullable reads and preservation of required bounds
traps are implemented in the [index batch](../../benchmarks/migration-results/2026-10-01-s4-index-contracts/README.md).
Internal constructor visibility and actual ABI observation are separated in the
[sealed-module batch](../../benchmarks/migration-results/2026-10-01-s4-sealed-modules/README.md).
Observed generic constructors, fixed struct callable contracts and canonical source
interfaces are implemented in the [generic batch](../../benchmarks/migration-results/2026-10-01-s4-observed-generics/README.md).
Declared shapes, checked spread/intersections/tags and concrete public product
storage are implemented in the [shape batch](../../benchmarks/migration-results/2026-10-01-s4-shapes/README.md).
Unified absence, pinned crossings, default arguments and public optional keys are
implemented in the [absence batch](../../benchmarks/migration-results/2026-10-01-s4-absence/README.md).
The opt-in number/NaN `charCodeAt` contract is implemented on both targets in
the [character-code batch](../../benchmarks/migration-results/2026-10-01-s4-char-code/README.md).
ABI enums/flags, declared storage, conversions and shared/native evaluation are
implemented in the [enum batch](../../benchmarks/migration-results/2026-10-01-s4-enums/README.md).
Payload bindings and sealed dispatch, including captured scopes and initialization
order, are implemented in the [variant batch](../../benchmarks/migration-results/2026-10-01-s4-variants/README.md).
The [handoff](handoff.md) records the current scope and compiler pins.

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
| [~] | [S4](plan.md#s4) Contracts | Payload variants/sealed calls, catalog/generic crossings and fact-consumer audit; staged language-default adoption belongs to V2 |
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
