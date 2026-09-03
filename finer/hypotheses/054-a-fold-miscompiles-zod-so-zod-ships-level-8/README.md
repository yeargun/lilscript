# 054 — A fold miscompiles zod, so zod ships at level 8

**Status:** confirmed and isolated to one fold. Not yet fixed.

## What was found

`zodlil/scripts/build.mjs` selected `lilscript.dev.toml` unless `--prod` was
passed, and nothing passes it: not `npm run build`, not `npm test`, not
`prepublishOnly`, not the fleet. So the port has been building, testing,
measuring **and publishing** its development configuration — optimization level
8 with candidate search off.

That is not an oversight. At the port's real configuration the artifact is
2,736 Brotli bytes smaller and **341 of its 1,353 tests fail**:

| build | raw | Brotli | tests |
|---|---|---|---|
| `lilscript.dev.toml` (level 8, search off) | 133,468 | 32,489 | 1353 / 1353 |
| `lilscript.toml` (level 15, search on) | 124,426 | **29,753** | **976 / 1317, 341 failed** |

The dev default is a workaround for a compiler bug, and it costs the published
package 2,736 Brotli bytes.

## Narrowing it

Level 8 passes and level 9 fails, at every level from 9 to 15 with the same 341
failures. Level 9 turns on exactly two things, and an explicit `optimizations`
list separates them:

| level 9 with | tests |
|---|---|
| neither | 1353 / 1353 |
| `structural-loop-variants` | 1353 / 1353 |
| **`parsed-peephole`** | **341 failed** |

So it is the token peephole. 111 folds run on this port, and skipping a set
makes the suite pass exactly when the set contains the culprit, which bisects in
seven builds:

```
control: no skips        -> fail
control: skip all 111    -> pass
skip 55 of 111 -> fail      skip 3 of 7 -> fail
skip 28 of 56  -> pass      skip 2 of 4 -> fail
skip 14 of 28  -> pass      skip 1 of 2 -> pass
skip  7 of 14  -> fail
CULPRIT: folds::declarations::declare_implicit_assignment_bindings
```

Confirmed alone: `LILSCRIPT_SKIP_FOLDS=declare_implicit_assignment_bindings`
gives 1353 / 1353, and removing the skip gives 341 failures. One fold, all of it.

## Why it was invisible

`LILSCRIPT_VALIDATE_FOLDS` already catches a fold that emits JavaScript the
parser rejects. This fold emits *valid* JavaScript for a *different program*,
which nothing catches: the artifact ships and the port's own suite is the only
witness. `LILSCRIPT_SKIP_FOLDS` and `LILSCRIPT_LIST_FOLDS` are added here so
that witness can be bisected without a rebuild per candidate.

This is the third wrong-program fold found this week, after
`fold_ident_ternary_to_or` and `fold_assigned_truthy_ternaries` in folder 050.
All three are token rewrites reasoning about structure — grouping in those two,
scope in this one — that a token stream does not carry. That is the pattern
worth acting on, not the three bugs individually.

## The suspected mechanism

The fold looks for `identifier =` whose name it believes is undeclared and
inserts `var name;` to declare it. It guards with
`name_is_declared_in_enclosing_function_scope` and `name_is_module_var_binding`.
If either guard misses a binding the name actually resolves to, the inserted
`var` **shadows** it, and every write that was meant for the outer binding
lands on a fresh local instead. That shape matches the failure count: a handful
of shared bindings, hundreds of dependent assertions.

Not yet proven on a minimal case. Doing that is the next step, because the fix
has to be a correct scope answer rather than another guard.

## Next

1. Minimise: one function where the fold inserts a `var` for a name that
   resolves to an outer binding.
2. Fix the scope query, not the symptom.
3. Then flip zodlil's build to its release configuration and take the 2,736.
4. Check the same fold against the ports that currently pass — a shadowing bug
   that changes behaviour only when the outer binding is written *after* the
   inserted declaration can be silent elsewhere.
