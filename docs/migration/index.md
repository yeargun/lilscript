# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- **C1, C2 and C3 are complete. S1 implementation is complete.** Its generic behavior, full language matrix and reference ports are checked; final library/CLI reruns and the complete compile-cost study are explicitly deferred at the owner's request. [Evidence](../../benchmarks/migration-results/2026-10-01-s1-complete/README.md).
- **S2 implementation is complete**, including its controls and focused JavaScript/native checks. [Evidence](../../benchmarks/migration-results/2026-09-30-s2-complete/README.md). S3 is next; this checkpoint does not start another milestone.
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

Finish **S3**: remaining inlining, known-call devirtualization, specialization
and identical-body sharing. Record its finite acceptance checklist before
implementation. Build on S2's shared allocation/field facts and the existing
record/product families. The [S2 contract](s2-acceptance.md) records completion
and dependency boundaries; the [handoff](handoff.md) identifies the retained
compiler and evidence.

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
| [~] | [S3](plan.md#s3) Calls | Remaining inlining, devirtualization, specialization and sharing |
| [~] | [S4](plan.md#s4) Contracts | Remaining identities, fact consumers, catalog and language coverage |
| [~] | [G1](plan.md#g1) Lexical names | Broader final-byte tie and workload qualification; local frequency allocation and protected final refinement are qualified |
| [~] | [G2](plan.md#g2) Property names | Broader observed/generic eligibility, field ordering and held-out calibration; private-field renaming/reuse is qualified |
| [~] | [G3](plan.md#g3) JS generation | Per-site spellings, ordering/locality and printer cleanup |
| [~] | [Q1](plan.md#q1) Choices | One family mechanism, coupled moves and deletion of bespoke mechanisms |
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
