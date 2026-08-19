# cases-00..06 — the paired-case corpus lane

Parent: [ledger](../LEDGER.md). Status: ongoing. Detail lives in the
[phase plan](../../README.md), which this note does not restate.

## Question

Does the reviewed corpus still prove compressability — LilScript no larger than the
best valid Terser / Oxc / esbuild artifact, per metric, on every eligible case?

## Current hypothesis

The corpus is a standing net rather than a task with an end. Its risk is not that it
breaks loudly; it is that it stops being run while a port-shaped fight absorbs
attention, and a fold landed for the port regresses a family nobody looked at.

## Constraints specific to this task

- Never post-minify LilScript output to pass a case.
- `lt` is only for a named typed advantage; ordinary portable code is `le`
  ([working rules](../../README.md#working-rules)).
- A loss is a compiler or `.lil` bug, not a reason to move the case to `le`.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Canonical configs really do search | `grep -n candidate_search comparison/cases/configs/*.toml` | `always`, `candidate_limit = 1536` in all three | diag |

## Log

- 2026-08-19 — Linked into the board so the corpus keeps a visible next step while the
  identity lane runs. Not re-planned; the phase files stay authoritative. — **OPEN**

## Next step

Run `node comparison/cases/run.mjs --canonical-only` before and after each identity-lane
change that touches a fold, and record any family that moves. That is this lane's only
job while `ident` is active.
