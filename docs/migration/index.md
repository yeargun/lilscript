# Migration checklist

Start here. [plan.md](plan.md) owns the design, dependencies and gates. [history.md](history.md) owns measurements. [coverage.md](coverage.md) maps unfinished legacy tasks and carried defects. Previous plans and records are in [old-history](../old-history/README.md).

`[x]` complete · `[~]` partly implemented · `[ ]` open. A mark describes implementation, not whether the new plan has been written.

## Now

- One compiler; the existing typed IR, facts, edit machinery, formation, naming and exact codec judging are the foundation.
- Last implementation batch: **Y5a/b**, private application roots and frames. Its accepted ratchet is the baseline; default IIFE wrapping remains open.
- C1's target-rule and pooling/packing veto slice is implemented and passes the library suite and generic ratchet with unchanged objective totals. S1 constant-call evaluation is in progress; C1's remaining controls stay open.
- Goal: smaller compiler-written output for **each** selected raw/gzip/Brotli objective, reliable TOML behavior, runtime parity and useful compilation-effort tradeoffs.

## Next

1. **C1:** complete the remaining producer/risk registry and control compatibility; the terminal pooling/packing and fixed target-rule vetoes are covered.
2. **S1 with C2/C3:** known-method and small pure constant-call evaluation, supported by the necessary oracle and measurement slices. Audit the folding claim that a raw bound proves a codec win. Calibration precedes tuning new heuristic defaults.
3. **Q2:** remove repeated formation and scoring, starting from the markedlil trace; preserve the best existing output.
4. **S2 and G1/G2:** escape/field facts and flattening; lexical naming and safe private-property mangling as their facts allow. Continue D1's existing format work when its batch is ready.

After these, follow the [working order](plan.md#8-working-order). Closure parity is V1's combined qualification goal, not a prerequisite for implementing naming or representation choices.

## Work packages

| State | Task | Still to complete |
|---|---|---|
| [~] | [C1](plan.md#c1) Controls | Registry, permission coverage, precedence and compatibility migration |
| [~] | [C2](plan.md#c2) Evidence | Oracle coverage, fair benchmark contract, runtime counters and paired runner |
| [ ] | [C3](plan.md#c3) Calibration | Generic calibration, protected held-out set, objective/work reports |
| [~] | [S1](plan.md#s1) Folding | Known methods, bounded pure calls, remaining propagation/defaults/removal |
| [ ] | [S2](plan.md#s2) Objects | Escape, uniqueness, fields, flattening and copy elision |
| [~] | [S3](plan.md#s3) Calls | Remaining inlining, devirtualization, specialization and sharing |
| [~] | [S4](plan.md#s4) Contracts | Remaining identities, fact consumers, catalog and language coverage |
| [~] | [G1](plan.md#g1) Lexical names | Local reuse, measured seeds/alphabets and final-byte tie handling |
| [ ] | [G2](plan.md#g2) Property names | Private renaming and safe reuse, with reflection and ABI coverage |
| [~] | [G3](plan.md#g3) JS generation | Per-site spellings, ordering/locality and printer cleanup |
| [~] | [Q1](plan.md#q1) Choices | One family mechanism, coupled moves and deletion of bespoke mechanisms |
| [ ] | [Q2](plan.md#q2) Reuse | Dirty scheduling, incremental formation, score memo and resource completion |
| [~] | [Q3](plan.md#q3) Search policy | Gzip's policy, calibrated estimates/proxies and useful effort progression |
| [~] | [Q4](plan.md#q4) Data | Const data, helpers, encodings and schema specialization |
| [~] | [D1](plan.md#d1) Formats | CJS/IIFE/UMD/bare, application wrapping and manifests |
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
- Keep `ref`, public constructibility and `pure` termination semantics explicit while their language decisions remain open. Complete Y5's already-approved IIFE default in D1.
- On landing, update this checklist and add one evidence record to history. Do not copy batch measurements or old chronology into this file.
