# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- One compiler; the existing typed IR, facts, edit machinery, formation, naming and exact codec judging are the foundation.
- **S1** evaluates bounded constant loops and scalar conversion/counting methods; generic objective gates and reference port suites pass. **S2** generalizes private record payloads under complete-use and capture proofs, with generic/port qualification; its target scalarizer now preserves initialization-time TDZ observations. **S3** now inlines explicit strict bodies into proven strict callers, with size and CPU qualification.
- **C1 is complete.** One registry owns permissions, producers, prerequisites, risk and the generated reference. Policy version 3 uses explicit runtime permissions; legacy version 2 stays compatible. CLI/TOML/environment precedence, outcome receipts, direct/search/reuse vetoes and unchanged corpus/port output are qualified. **D1**'s private application IIFE and explicit bare output have separate size reports and pass the reference port suites.
- C2 retains alternating compilation and runtime samples, explicit runtime oracles and input/artifact identities. Q2 reuses exact byte measurements and unchanged semantic views; stable-rule scheduling passes generic/port gates and paired CPU measurement. G1's alphabet and protected local-frequency trials are qualified. S4's typed record spread clears the last generic refusal: all 642 programs pass under each objective, with the reference port suites green.
- G2’s first private-field family passes generic/port gates and paired compilation measurement, including reflection, explicit preserved keys and TOML vetoes. Q2’s indexed call lookup now removes repeated target-rule work with unchanged artifacts and useful search preserved; further formation reuse remains open.
- Q3's deferred naming tail now defaults to effort 14+, with explicit opt-in at 13, and combines local naming/private-field refinements. Generic/port gates and paired costs qualify this slice; level 13 restores the established search. Final package wrappers can still reverse core rankings. Q2's statement-mention reuse is qualified: all active tests and unchanged generic/port output pass, with lower Marked/Zod CPU and unchanged peak accounted memory.
- **D3** resolves and searches any TOML/API objective set independently over one checked program, with all baselines admitted before shared optional work. The CLI writes separate codec directories and a combined manifest; single-objective output stays compatible. Library/CLI, generic and port qualification pass with ordinary outputs preserved.
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

**Active: C2 only.** Complete the independent-oracle coverage, fair pinned
benchmark contracts and paired runtime/counter evidence before advancing to C3.
Its acceptance checklist follows the existing-evidence audit; do not extend
other milestones while it is active.

C1 is [complete and qualified](../../benchmarks/migration-results/2026-09-30-c1-complete/README.md):
registry and permissions, precedence/provenance, versioned runtime-risk
migration, generated reference, direct/search/reuse coverage and release gates.
Then follow the [single milestone order](plan.md#8-working-order). Existing
partial work below stays assigned to its milestone.

## Work packages

| State | Task | Still to complete |
|---|---|---|
| [x] | [C1](plan.md#c1) Controls | Complete; new producers register their controls within their own milestone |
| [~] | [C2](plan.md#c2) Evidence | Broader oracle coverage, fair competitor contract and representative port runtime/counter qualification |
| [~] | [C3](plan.md#c3) Calibration | Larger generic programs that exercise Brotli pruning, protected held-out set and broader objective/work calibration |
| [~] | [S1](plan.md#s1) Folding | Broader supported evaluation, remaining propagation/defaults/removal |
| [~] | [S2](plan.md#s2) Objects | Broader aliases/escape, fields, flattening and copy elision |
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

- One active milestone and one heavy build, test or measurement job at a time. Implement coherent batches before testing; reserve full qualification for milestone completion.
- Size comes first at default effort 13 and above; expensive strategies with small measured returns may default to 14 or 15 with explicit overrides. Document every control's situation, defaults/gates and size/compile/runtime tradeoffs. Judge batch totals separately by objective; explain small local heuristic regressions and additional search cost.
- Every open defect has an owner in [coverage.md](coverage.md). Native completion remains owned separately from JavaScript qualification.
- Keep `ref`, public constructibility and `pure` termination semantics explicit while their language decisions remain open. D1 now supplies Y5's approved application IIFE default.
- On landing, update this checklist and add one evidence record to history. Do not copy batch measurements or old chronology into this file.
