# ident-06 — comma sequences folded into `||` and `?:` operands

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Why did `@itslil/posthog-js/error-tracking` produce a wrong `mechanism` object at
some candidate-search settings and a right one at others, with no `.lil` change?

## Current hypothesis

Falsified and replaced by a proven cause: `parse_single_assignment`
(`src/codegen_ir_js.rs`) reported a comma sequence as a single assignment.
Callers splice its `value` into an expression position, so a guarded branch body
became unguarded.

## Constraints specific to this task

`parse_single_assignment` has 13 callers, every one of which uses the result as
"this statement assigns `value` to `target`". The `var` path already cuts at the
first top-level comma and its callers validate the `trailing` declarators, so
only the non-`var` path needed the guard.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | pre-peephole emission of `builderBuildFromUnknown` at `candidate_search=always` | temporary `LILSCRIPT_DUMP_PREPEEP` hook in `optimize_generated_javascript` | `r=r||{},r.handled=!0,r.type="generic"` present **before** the peephole; a sibling candidate emitted the correct `if(!i){…}` | diag |
| 2026-08-25 | compat after the `parse_single_assignment` guard | `POSTHOGLIL_ERROR_TRACKING_ARTIFACT=<artifact> node --test test/error-tracking.compat.test.mjs` (posthoglil) | mechanism subtest green on every setting measured | gate |
| 2026-08-25 | size cost of refusing the unsound fold | `target/release/lilscript-codec --json` on zodlil `src/entry.lil` | Brotli-11 31,772 → 31,804 (+32); jQuery −866 in the same build | gate |

## Log

- 2026-08-25 — Source semantics are `if(!mechanism.truthy()){mechanism=JS.object();mechanism["handled"]=true;mechanism["type"]="generic";}`. Emission produced `r=r||{},r.handled=!0,r.type="generic"`, which overwrites a caller-supplied mechanism because `||` binds tighter than `,`. Root cause is `parse_single_assignment` accepting `r={},r.handled=!0,r.type="generic"` as one assignment with value `{},r.handled=!0,r.type="generic"`. Guarded the non-`var` path with `split_top_level_comma`. — **LANDED**
- 2026-08-25 — `fold_statement_or_assigns` (`src/js_peephole/folds/boolean.rs`) had the same class of bug in all five of its rewrite shapes, reachable from `!m&&(m={},m.x=1)`. Added `spans_top_level_comma` and refused each shape; two regression tests pin refusal and continued folding of single-expression right-hand sides. This was a latent second path, not the one error-tracking hit. — **LANDED**
- 2026-08-25 — `fold_or_empty_object_assign` (`classes.rs`) existed to rewrite `l=l||{},Object.assign(l,{…})` into `l=l||{…}`. That fold was papering over this bug on the `batch_property_assigns` spelling only; with individual property stores the miscompile was visible. The fold is still useful for size but is no longer the only thing keeping the shape correct. — **OPEN** (consider whether it is now redundant)

## Next step

Seed the shape into `target/debug/lilscript-differential` so a comma sequence in
a conditional operand is caught without a library, and pair it as a canonical
case. That is [ident-03](ident-03.md) / [ident-04](ident-04.md) work.
