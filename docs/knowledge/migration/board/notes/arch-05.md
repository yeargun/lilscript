# arch-05 — 07.5 peephole is contraction

Parent: [ledger](../LEDGER.md). Status: active. Plan:
[07.5](../../07-global-compressor.md#075--peephole-is-contraction).

## Question

Does parsed JS only contract already-legal programs, always codec-scored or
skipped — including when `candidate_search = off`? Can those folds migrate to a
hygienic target JS AST with binding/property IDs instead of reparsing text?

## Current hypothesis

Search-on leaves, the canonical winner challenger, and search-off's configured
function-preserving challenger are now exactly scored. Canonical finalization is
charged to the terminal ledger and uses full priority/startup ranking. IR emits
proof-marked classes from 07.4. Parsed text still owns contraction and must move
to a hygienic target AST.

## Constraints specific to this task

- Blocked on arch-04. Do not grow `classes.rs` while IR still cannot emit `class`.
- Search-off: skip peephole or score one clone against the untouched emit.
- Every terminal challenger uses the same `javascript_candidate_rank`, startup,
  ABI, and explicit-obligation checks as the main selector.
- Move one contraction family at a time to target AST; require byte/semantic
  ablations before deleting the text fold.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | architecture audit | `compiler.rs` `:8448` / `:8501` | scored canonical vs unscored search-off split holds | diag |
| 2026-08-28 | canonical finalist obeys budget/rank | `cargo test --release --lib selected_canonical_peephole_uses_only_reserved_terminal_work` | pass | gate |
| 2026-08-28 | search-off clone is measured | `cargo test --release --lib search_off_finalization_scores_the_artifact_it_returns` | pass; returned raw score equals returned bytes | gate |
| 2026-08-28 | legacy pre-finalist search-off substitution removed | `cargo test --release --lib explicit_constructor_export_preserves_named_class_identity search_off_finalization_scores_the_artifact_it_returns` | search-off keeps the untouched mandatory baseline until the one measured finalization clone | gate |

## Log

- 2026-08-28 — Scheduled as 07.5. — **OPEN**
- 2026-08-28 — Removed both unscored finalization paths. Target AST migration remains. — **OPEN**
- 2026-08-28 — Removed the duplicate pre-finalist search-off substitution found by class-field ABI coverage. — **OPEN**

## Next step

Introduce a target JS AST with resolved bindings, migrate contractions one
family at a time, then retire the generated-JS parser from production. Contract:
[size-first libraries](../../07-global-compressor.md#size-first-library-contract).
