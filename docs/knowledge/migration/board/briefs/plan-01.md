# Brief — plan-01

For the first fresh-context compression-migration review. Read
[mission](../../../mission.md), this brief, and
[notes/plan-01.md](../notes/plan-01.md) before inspecting the named sources.

## Task

Deeply review and directly improve `docs/knowledge/migration/compression-migration.md`
as an architecture and language-design plan. Reconcile it with the canonical
migration and planned architecture. Verify that proposed language surfaces state
reusable semantics rather than encode JavaScript/minifier shapes, that objective-
specific compilation cannot alter public semantics/ABI, and that dependencies and
exit criteria are implementable. Remove glue, speculative frameworks, accidental
duplication, and unsafe assumptions. Do not implement compiler code.

## Why this matters to the objective

LilScript's durable compression advantage must come from language facts and typed
proofs before JavaScript is fixed, while exact raw/gzip/Brotli scoring chooses among
legal private representations. A roadmap that starts with target patches would
erase that advantage.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/plan-01.md`
- `docs/knowledge/migration/compression-migration.md`
- `docs/knowledge/migration/planned-migration.md`
- `docs/knowledge/compilation/current-architecture.md`
- `docs/knowledge/compilation/planned-architecture.md`
- `docs/knowledge/language/compressor-surface.md`
- `docs/language-v0.1.md`
- `differences/index.md`
- `differences/compression-opportunities.md`
- `differences/objective-and-search.md`
- `src/ast.rs`, `src/parser.rs`, `src/semantic.rs`, `src/lower.rs`, `src/ir.rs`
- `src/compilation_contract.rs`, `src/decision_registry.rs`

## May touch

- `docs/knowledge/migration/compression-migration.md`
- `docs/knowledge/migration/board/notes/plan-01.md`

Everything else is read-only.

## Must not

- The [standing refusals](../README.md#standing-refusals).
- Do not add compiler, parser, test, config, benchmark, or port code.
- Do not turn provisional syntax spelling into a committed language contract.
- Do not weaken the existing canonical migration or skip C0-C3 foundations.
- Do not claim an optimization is missing before checking current source.

## Prove it

```sh
node scripts/board.mjs check
node scripts/check-doc-links.mjs
```

Expected: both commands exit zero; the plan remains subordinate to the canonical
migration and every language proposal has semantic, ABI, lowering, and test gates.

## Report

Append evidence and one verdict line to `notes/plan-01.md`. Return at most 20
lines with changes, corrections, gate results, and the next risk. Do not edit
`LEDGER.md`.
