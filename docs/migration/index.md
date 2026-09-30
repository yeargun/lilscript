# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- One compiler; the existing typed IR, facts, edit machinery, formation, naming and exact codec judging are the foundation.
- **S1** now evaluates bounded constant loops; the generic objective gates and reference port suites pass. **S2** generalizes private record payloads under complete-use and capture proofs, with generic/port qualification.
- **C1** enforces target-rule, pooling/packing and reconstruction permissions, records implemented producer stages and prerequisites, and diagnoses unavailable or conflicting settings. Its reconstruction correction has paired measurements and port qualification. **D1**'s private application IIFE and explicit bare output have separate size reports and pass the reference port suites.
- C2 retains alternating compilation and runtime samples, explicit runtime oracles and input/artifact identities. Q2 reuses exact byte measurements and unchanged semantic views; stable-rule scheduling passes generic/port gates and paired CPU measurement. G1's alphabet and protected local-frequency trials are qualified. S4's typed record spread clears the last generic refusal: all 642 programs pass under each objective, with the reference port suites green.
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

1. **Q2:** remove the remaining repeated formation and scoring after the qualified stable-rule scheduler; preserve useful search results.
2. **S1 with C2/C3:** continue propagation/defaults/removal beyond the implemented bounded primitive evaluator. The raw-bound-as-codec-proof claim is corrected; calibration precedes tuning new heuristic defaults.
3. **C1/D1:** remaining family/risk coverage, compatibility migration, formats and manifests; producer availability, prerequisite vetoes, contradictory aliases and the application wrapper are covered.
4. **S2 and G1/G2:** escape/field facts and flattening; lexical naming and safe private-property mangling as their facts allow. Continue D1's existing format work when its batch is ready.

After these, follow the [working order](plan.md#8-working-order). Closure parity is V1's combined qualification goal, not a prerequisite for implementing naming or representation choices.

## Work packages

| State | Task | Still to complete |
|---|---|---|
| [~] | [C1](plan.md#c1) Controls | Remaining family/risk registry coverage, precedence audit and compatibility migration |
| [~] | [C2](plan.md#c2) Evidence | Broader oracle coverage, fair competitor contract and representative port runtime/counter qualification |
| [ ] | [C3](plan.md#c3) Calibration | Generic calibration, protected held-out set, objective/work reports |
| [~] | [S1](plan.md#s1) Folding | Broader supported evaluation, remaining propagation/defaults/removal |
| [~] | [S2](plan.md#s2) Objects | Broader aliases/escape, fields, flattening and copy elision |
| [~] | [S3](plan.md#s3) Calls | Remaining inlining, devirtualization, specialization and sharing |
| [~] | [S4](plan.md#s4) Contracts | Remaining identities, fact consumers, catalog and language coverage |
| [~] | [G1](plan.md#g1) Lexical names | Broader final-byte tie and workload qualification; local frequency allocation and protected final refinement are qualified |
| [ ] | [G2](plan.md#g2) Property names | Private renaming and safe reuse, with reflection and ABI coverage |
| [~] | [G3](plan.md#g3) JS generation | Per-site spellings, ordering/locality and printer cleanup |
| [~] | [Q1](plan.md#q1) Choices | One family mechanism, coupled moves and deletion of bespoke mechanisms |
| [~] | [Q2](plan.md#q2) Reuse | Dirty scheduling, incremental formation, persistent reuse and resource completion |
| [~] | [Q3](plan.md#q3) Search policy | Gzip's policy, calibrated estimates/proxies and useful effort progression |
| [~] | [Q4](plan.md#q4) Data | Const data, helpers, encodings and schema specialization |
| [~] | [D1](plan.md#d1) Formats | CJS/library-IIFE/UMD and remaining manifests |
| [~] | [D2](plan.md#d2) Consumers | Shakeability, placement, lazy effects/cycles, host modules and source maps |
| [~] | [D3](plan.md#d3) Integration | Shared graph/API, multi-objective requests and compiler-written port builds |
| [ ] | [V1](plan.md#v1) Qualification | Maintained/held-out size and runtime gaps, independently per objective |
| [ ] | [V2](plan.md#v2) Release | Reproducible packages, sites and current receipts |
| [~] | [N1](plan.md#n1) Native integration | Optimized shared program, toolchain owner and capabilities |
| [ ] | [N2](plan.md#n2) Native completion | Remaining language/runtime/ABI coverage and performance qualification |

## Working rules

- One heavy build, test or measurement job at a time. Verify coherent batches, roughly every 20 minutes of implementation or after a critical shared-path change.
- Size comes first at the default effort and above. Judge batch totals separately by objective; explain small local heuristic regressions and the CPU cost of additional search.
- Every open defect has an owner in [coverage.md](coverage.md). Native completion remains owned separately from JavaScript qualification.
- Keep `ref`, public constructibility and `pure` termination semantics explicit while their language decisions remain open. D1 now supplies Y5's approved application IIFE default.
- On landing, update this checklist and add one evidence record to history. Do not copy batch measurements or old chronology into this file.
