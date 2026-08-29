# ident-09 — total terminal rename closure

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Can every whole-artifact terminal rename prove that all rewritten occurrences
retain their declaration, including unsupported nested scopes and template
expressions, and that fixed descendant names cannot capture renamed outers?

## Current hypothesis

`BindingResolution::is_total()` exists but convergence and the older remap
admission paths did not consistently require it. A total-resolution gate,
reservation of fixed descendant names, conservative template rejection, and an
exhausted bounded generator close the known V-02 gaps without another scope
approximation.

## Constraints specific to this task

Use the existing resolver as authority. Do not add neighboring-token scope
heuristics, widen search, alter ABI policy, or rename properties/templates whose
binding identity is not represented.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Focused convergence tests | `cargo test --release --lib js_peephole::rename` | 20 passed, 0 failed | gate |
| 2026-08-29 | Focused two-character remap tests | `cargo test --release --lib two_character_remapping` | 2 passed, 0 failed | gate |
| 2026-08-29 | Complete generated-JS peephole suite after all remap gates | `cargo test --release --lib js_peephole` | 524 passed, 0 failed | gate |
| 2026-08-29 | Canonical paired cases | `node comparison/cases/run.mjs --canonical-only` under Node 24.11.1 | 54/54 passed; strict wins raw 54, gzip 53, Brotli 53 | gate |
| 2026-08-29 | Codec contract | `node --test benchmarks/codec-contract.test.mjs` under Node 24.11.1 | 10 passed, 0 failed | gate |
| 2026-08-29 | Full Rust library suite | `cargo test --release --lib` | 1,595 passed, 4 failed outside the changed rename/object paths (`keeps_js_push_and_empty_array_factories_prototype_observable`, two config-policy tests, `stringify_elision_crosses_intervening_constants`); not a green gate | gate |
| 2026-08-29 | Current-tree fork behavior preflight | isolated MotionLil, MarkedLil, MobXLil, jQueryLil, and SolidLil builds/tests using the current release compiler | Motion 9/9; Marked 29/29; MobX 769 passed/11 skipped plus package smoke; jQuery 6/6; Solid 49/50 then isolated JFB rerun passed; diagnostic because sibling trees are not pinned | diag |
| 2026-08-29 | Full release and five-fork checkpoint | `cargo test --release --all-targets`; `node comparison/large-libraries/run.mjs --run --compiler migration,candidate ...` | 1,603 library tests plus binary targets passed; all 13 current candidate boundaries passed fresh semantics with no eligible selected-metric regression | gate |

## Log

- 2026-08-29 — Added total-resolution admission, fixed-descendant reservation, bounded name exhaustion, and declaration-resolved two-character remapping. Whole-artifact one-byte and function-local candidates now also reject unresolved/template-bearing artifacts. Targeted and canonical gates pass; closure waits for the five-fork G2 evidence gate. — **OPEN**
- 2026-08-29 — The expanded five-fork G2 checkpoint passed every current candidate semantic boundary without an eligible selected-metric regression. — **LANDED**

## Next step

Keep the regressions in the standing release suite while `gate-04` moves binding
identity into mandatory final-artifact admission.
