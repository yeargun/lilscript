# Brief — plan-03

For the third fresh-context compression-migration review. Read
[mission](../../../mission.md), this brief, and
[notes/plan-03.md](../notes/plan-03.md) before inspecting the named sources.

## Task

Deeply reread the latest `docs/knowledge/migration/compression-migration.md` after
two independent reviews and directly improve its verification, corpus, rollout,
and regression-reasoning design. Inspect current harnesses and sibling fork
scripts. Ensure major steps test MotionLil, MarkedLil, MobXLil, jQueryLil, and
SolidLil progressively; allow safe parallel preflights but preserve isolated
outputs and sequential authoritative resource measurements. Ensure regressions
lead to classification and generic fixes or idiomatic source work, never hidden
thresholds or package-specific compiler logic. Do not implement compiler code.

## Why this matters to the objective

A compression architecture is successful only when real maintained boundaries
stay semantically correct and improve in their selected metric. Progressive,
fingerprinted corpus gates prevent elegant local work from accumulating into a
large-library regression.

## Read

- `docs/knowledge/mission.md`
- `docs/knowledge/migration/board/notes/plan-03.md`
- `docs/knowledge/migration/compression-migration.md`
- `docs/knowledge/migration/planned-migration.md`
- `docs/current-status.md`
- `docs/knowledge/verification/README.md`
- `docs/knowledge/verification/failure-triage.md`
- `docs/knowledge/evidence/`
- every Markdown file under `differences/`
- `comparison/large-libraries/README.md`, `matrix.json`, `contract.mjs`, and `run.mjs`
- package scripts in `/Users/yeargun/motionlil`, `/Users/yeargun/markedlil`,
  `/Users/yeargun/mobxlil`, `/Users/yeargun/jquerylil`, and `/Users/yeargun/solidlil`

## May touch

- `docs/knowledge/migration/compression-migration.md`
- `docs/knowledge/migration/board/notes/plan-03.md`

Everything else is read-only.

## Must not

- The [standing refusals](../README.md#standing-refusals).
- Do not edit sibling repositories, compiler code, tests, configs, or evidence.
- Do not treat a skipped suite, stale `dist/`, cross-metric win, or aggregate
  multi-library win as a passing gate.
- Do not parallelize jobs that share output/cache files or authoritative timing.
- Do not weaken zero-regression policy to make the plan appear achievable.

## Prove it

```sh
node scripts/board.mjs check
node scripts/check-doc-links.mjs
node --test comparison/large-libraries/contract.test.mjs
node comparison/large-libraries/run.mjs --check
```

Expected: all commands exit zero; every major work unit has proportional tests,
periodic fork gates, regression classification, and rollback criteria.

## Report

Append evidence and one verdict line to `notes/plan-03.md`. Return at most 20
lines with changes, gate results, corpus risks, and the first implementation
checkpoint. Do not edit `LEDGER.md`.
