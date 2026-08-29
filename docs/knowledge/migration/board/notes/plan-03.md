# plan-03 — verification and corpus review

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Can the migration be executed incrementally without semantic drift or cumulative
selected-metric regressions across the maintained real-library boundaries?

## Current hypothesis

The review found concrete corpus-readiness gaps: the tracked matrix omits
MotionLil, does not exercise MobXLil's true `production-min` artifact, and can
confuse direct compiler output with post-processed MotionLil/SolidLil package
artifacts. Major work units need progressive five-fork gates, isolated parallel
preflights, sequential authoritative resources, and classified regression
outcomes.

## Constraints specific to this task

Edit only the migration plan and this note. Do not generate new evidence or edit
sibling repositories. Preserve per-boundary/per-objective zero-regression rules.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Board integrity | `node scripts/board.mjs check` | `board check passed: 38 tasks, 30 notes.` | gate |
| 2026-08-29 | Documentation links | `node scripts/check-doc-links.mjs` | `documentation graph valid: 190 Markdown files, 114 canonical pages reachable` | gate |
| 2026-08-29 | Large-library contract | `node --test comparison/large-libraries/contract.test.mjs` | TAP printed `tests 11`, `pass 11`, `fail 0`, `skipped 0`, duration `84.354666 ms` | gate |
| 2026-08-29 | Immutable large-library evidence | `node comparison/large-libraries/run.mjs --check` | `large-library evidence valid: 14 observations, 12 metric rows` | gate |

## Log

- 2026-08-29 — Awaiting two earlier independent reviews before final gate audit. — **OPEN**
- 2026-08-29 — Added progressive five-fork gates, current-harness gap inventory, isolated preflight/sequential resource rules, zero-threshold rollback, and classification-driven generic/compiler-or-idiomatic-source outcomes; all proof commands passed. — **LANDED**

## Next step

Extend the phase-0 tracked matrix to all five forks, beginning with canonical
MotionLil direct-output boundaries and a true MobXLil `production-min` row, then
prove the first G2 corpus-readiness checkpoint.
