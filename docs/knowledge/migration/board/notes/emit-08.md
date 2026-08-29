# emit-08 — ordinary-object assignment semantics

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Can generated-JS fresh-object collection preserve inherited setter and
`__proto__` behavior without relying on an undeclared pristine-prototype
assumption?

## Current hypothesis

The text fold has no typed ownership proof. It must remain disabled unless the
existing `assume_pristine_builtins` contract is explicit; owned typed aggregate
construction should be optimized before this layer.

## Constraints specific to this task

Do not infer hook-free ordinary objects from `{}` syntax. Keep the sequential
assignment incumbent and preserve direct tests of the fold under the explicit
unsafe contract.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Parsed-pipeline pristine gate | `cargo test --release --lib fresh_object_assignment_collection_requires_pristine_builtins` | 1 passed, 0 failed | gate |
| 2026-08-29 | Emitter inherited-setter behavior | `cargo test --release --lib preserves_fresh_object_assignments_without_pristine_builtins` | 1 passed, 0 failed; observed setter call and no own property | gate |
| 2026-08-29 | Complete generated-JS peephole suite | `cargo test --release --lib js_peephole` | 524 passed, 0 failed | gate |
| 2026-08-29 | Canonical and codec gates | `node comparison/cases/run.mjs --canonical-only`; `node --test benchmarks/codec-contract.test.mjs` | 54/54 canonical and 10/10 codec passed | gate |
| 2026-08-29 | Full Rust library suite | `cargo test --release --lib` | 1,595 passed, 4 failed outside the changed object/rename paths; not a green gate | gate |
| 2026-08-29 | Current-tree fork behavior preflight | isolated MotionLil, MarkedLil, MobXLil, jQueryLil, and SolidLil builds/tests using the current release compiler | All maintained checks passed after correcting temp-copy isolation; diagnostic because sibling trees are not pinned | diag |
| 2026-08-29 | Full release and five-fork checkpoint | `cargo test --release --all-targets`; `node comparison/large-libraries/run.mjs --run --compiler migration,candidate ...` | 1,603 library tests plus binary targets passed; all 13 current candidate boundaries passed fresh semantics with no eligible selected-metric regression | gate |

## Log

- 2026-08-29 — Gated both parsed cleanup and direct candidate emission on the existing pristine-builtins contract and added inherited-setter coverage. Targeted/canonical gates pass; closure waits for the five-fork G2 evidence gate. — **OPEN**
- 2026-08-29 — The expanded five-fork G2 checkpoint passed every current candidate semantic boundary without an eligible selected-metric regression. — **LANDED**

## Next step

Keep the inherited-setter regressions in the standing release suite while
`gate-04` adds mandatory property-category admission for final artifacts.
