# ident-08 — two more folds that changed a value's type

Parent: [ledger](../LEDGER.md). Status: landed. Family: [ident-06](ident-06.md).

## Question

Lifting the search budget on the remaining four packs exposed two failures that
the starved search never selected. What were they?

## Current hypothesis

Both confirmed. Same shape of mistake as `ident-06`: a fold recognized a
sub-expression and rewrote the enclosing expression as if the two had the same
value.

## Constraints specific to this task

Neither fold may simply be disabled: the single-expression cases they exist for
are still profitable, and the tests pin both directions.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | autocapture at the lifted budget | `node --test test/autocapture.compat.test.mjs` (posthoglil) | `Cannot access 'r' before initialization` | gate |
| 2026-08-25 | minimized | `cargo test --release --lib parameter_defaults_never_read_a_later_formal` | `function v(e,t,r){t===void 0&&(t=r)}` folded to `function v(e,t=r,r)` | gate |
| 2026-08-25 | surveys at the lifted budget | `node --test test/surveys.compat.test.mjs` | `detectSurveyLanguage` returned `true` where `'de'` was expected | gate |
| 2026-08-25 | minimized | `cargo test --release --lib assigned_truthy_ternary_needs_the_assignment_to_be_the_whole_condition` | `"string"==typeof e&&""!=(e=e.trim())?e:null` folded to `…\|\|null` | gate |
| 2026-08-25 | all five packs after both fixes, both configs | 10 compat runs | green | gate |

## Log

- 2026-08-25 — `fold_undefined_defaults_into_formals` moved a body assignment into the parameter list without checking declaration order. A parameter list is its own TDZ scope, so `t=r` where `r` is a *later* formal throws on every call that omits `t`. Guarded with `default_reads_later_formal`; backward reads and `e.r` member reads still fold. Exposed by the previous session's change that let the fold descend into nested bodies, so it was reachable on far more sites than before. — **LANDED**
- 2026-08-25 — `fold_assigned_truthy_ternaries` rewrites `(name=EXPR)?name:FALLBACK` to `EXPR||FALLBACK`, which is right only when the parenthesized assignment is the whole ternary condition. It checked what followed the `)` and never what preceded the `(`, so in `""!=(e=e.trim())?e:null` it folded an *operand*, returning the comparison's boolean instead of the trimmed string. Guarded with `condition_starts_at`. — **LANDED**
- 2026-08-25 — All three miscompiles found across this session and the last ([ident-06](ident-06.md) and both above) are the same class: **a fold treated a sub-expression's value as the enclosing expression's value.** A differential-oracle shape for that class would have caught all three without a library. — **OPEN**, and it is the strongest argument yet for [ident-03](ident-03.md).

## Next step

Seed [ident-03](ident-03.md) with the shared class: generate expressions where a
guarded or parenthesized sub-expression has a different value and type from its
parent, and check every fold preserves the parent's value.
