# Brief — plan-02

For the second fresh-context compression-migration review. Read
[mission](../../../mission.md), this brief, and
[notes/plan-02.md](../notes/plan-02.md) before inspecting the named sources.

## Task

Deeply reread the latest `docs/knowledge/migration/compression-migration.md` after
the first review and directly improve it from a compiler-compression perspective.
Audit current LilScript source and every file under `differences/`; remove work the
compiler already implements, add missing high-value Closure-inspired behavior,
correct phase dependencies, and make each transform an elegant typed-IR or
hygienic-target design rather than a textual patch. Do not implement compiler code.

## Why this matters to the objective

Closure supplies mature candidate discovery while LilScript supplies stronger
typed proofs and exact codec selection. The plan must combine those strengths
without replacing LilScript's objective model with raw-size folklore.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/plan-02.md`
- `docs/knowledge/migration/compression-migration.md`
- every Markdown file under `differences/`
- `docs/knowledge/compilation/current-architecture.md`
- `docs/knowledge/compilation/planned-architecture.md`
- `docs/optimization-coverage.md`
- `src/optimizer.rs`, `src/value_analysis.rs`, `src/compress_passes.rs`
- `src/codegen_ir_js.rs`, `src/compiler.rs`, `src/decision_registry.rs`
- `src/js_peephole/mod.rs`, `src/js_peephole/binding.rs`, and `src/js_peephole/folds/`
- the pinned Closure sources linked from `differences/source-reference.md`

## May touch

- `docs/knowledge/migration/compression-migration.md`
- `docs/knowledge/migration/board/notes/plan-02.md`

Everything else is read-only.

## Must not

- The [standing refusals](../README.md#standing-refusals).
- Do not add implementation or test code.
- Do not add a transform because Closure has it if typed lowering already removes
  the representation or measured evidence does not justify a candidate.
- Do not call bounded search optimal or make a local estimate authoritative.
- Do not preserve a redundant work item under a new name.

## Prove it

```sh
node scripts/board.mjs check
node scripts/check-doc-links.mjs
```

Expected: both commands exit zero; each work unit names its semantic layer,
incumbent, objective decision, safety proof, and verification boundary.

## Report

Append evidence and one verdict line to `notes/plan-02.md`. Return at most 20
lines with changes, corrected assumptions, gate results, and the highest-value
remaining implementation unit. Do not edit `LEDGER.md`.
