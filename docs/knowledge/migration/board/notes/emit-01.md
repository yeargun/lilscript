# emit-01 — emitted JS must be valid and direct

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Is there an emission path that can place a statement in expression position — and does
it exist independently of the receiver coloring that revealed it?

## The shape

`?:break` was emitted while an experimental receiver coloring was active. `break` is a
statement; a conditional expression cannot hold one. The coloring was backed off
([ident-01](ident-01.md), REJECTED), which removed the symptom without answering
whether the emitter can still produce it under some other pressure.

## Current hypothesis

The emitter decides "this branch is short enough to spell as `?:`" from the shape of
the branch bodies, and some path lets a control-transfer body through. If so, the bug
is reachable whenever a fold rewrites a branch body into something that *looks*
expression-like, coloring or not.

## Constraints specific to this task

- The fix is a validity rule, not a heuristic that avoids the shape. `?:` selection
  must be unable to swallow a control transfer, rather than unlikely to.
- Directness is the standard: the emitted JS is what a careful human would write. Do
  not fix a validity bug by inserting an indirection.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-19 | Symptom observed under experimental coloring; not reproduced since backing off | — | no minimized repro on disk yet | diag |
| 2026-08-29 | Direct branch-fold control-transfer regression | `cargo test --release --lib expression_branches_reject_control_transfer_statements` | `break`, `continue`, `return`, and `throw` in either arm all remained statements; 1 passed | gate |

## Log

- 2026-08-19 — Split out of [ident-01](ident-01.md) so backing off the coloring does not
  close a real emission bug by accident. — **OPEN**
- 2026-08-29 — The suspected path already delegates every arm to `ExpressionParser::parse_complete`, which rejects control-transfer statements. Added direct coverage for both arms and all four transfer forms; no production change was needed. — **LANDED**

## Landed in this lane — do not re-derive

- `emit-02` — String / Regex / `JS.encodeURI` lower to JS members. The host-trampoline
  spelling (`pickRegex3`, `stringTrim`) is rejected permanently: the web surface is the
  typed member, and the compiler emits the obvious JS. — **LANDED**
- `emit-03` — `if`/`return` regex picks emit `?:`. — **LANDED**
- `emit-04` — Identifier inlining follows JS precedence rather than `|0` patches. — **LANDED**

## Next step

Keep the direct regression in the generated-JavaScript suite; any future target
AST branch contraction must preserve the same expression-only type boundary.
