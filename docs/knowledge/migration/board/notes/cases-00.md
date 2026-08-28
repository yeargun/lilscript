# cases-00..06 — standing paired-case corpus

Parent: [ledger](../LEDGER.md). Status: ongoing. Order and phase status:
[migration](../../README.md).

## Question

Does the reviewed corpus still prove compressability — LilScript no larger than the
best valid Terser / Oxc / esbuild artifact, per metric, on every eligible case?

## Current hypothesis

The corpus is a standing net, not a task with an end. 47 hand-authored
`canonical/` folders exist. Catalog + algorithms already run from
`comparison/run-all.sh`. Risk: a port-shaped fight lands a fold and nobody
re-runs `--canonical-only`.

This is not a restart of phases 00–06. Exported constructor identity and
expression-if are 07 work, not “add more `if` cases.”

## Constraints specific to this task

- Never post-minify LilScript output to pass a case.
- `lt` is only for a named typed advantage; ordinary portable code is `le`.
- A loss is classified before the expect is moved to `le`.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Canonical configs really do search | `grep -n candidate_search comparison/cases/configs/*.toml` | `always`, `candidate_limit = 1536` in all three | diag |
| 2026-08-25 | Full cases after ident-08 | journal / `comparison/cases` | 617/617, strict wins 617/612/613 | diag |
| 2026-08-28 | Hand-authored corpus | `find comparison/cases/canonical -name case.toml` | 47 folders: scalars, strings, control, functions, aggregates, wins, collections, effects, host | diag |

## Log

- 2026-08-19 — Linked into the board so the corpus keeps a visible next step. — **OPEN**
- 2026-08-28 — Refreshed: 00–06 are standing, not a start-from-scratch plan. — **OPEN**

## Next step

Run `node comparison/cases/run.mjs --canonical-only` before and after each
identity or architecture change that touches a fold, and record any family that
moves. That is this lane's only job while `ident` / `arch` are active.
