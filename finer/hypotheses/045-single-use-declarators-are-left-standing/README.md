# 045 — single-use declarators are left standing

**Status: OPEN — can a canonical declarator-substitution fold with Terser's stop rules take the 42
micromarklil sites Terser collapses and we leave, for ≥ −100 Brotli there and ≥ −40 on mobx?**
Lane: compiler. Objective: brotli. Ports: micromarklil, mobxlil, jquerylil, markedlil, then the
fleet. Opened: 2026-09-02.

## Prior art

Harvest of 2026-09-02, the D.1 block in
[refs/competitor-techniques.md](../../refs/competitor-techniques.md) (`:149-221`), one row per shape.

- **Terser** `collapse_vars` (`lib/compress/tighten-body.js:278-970`): per statement it harvests
  `x=E`, `var|let x=E` and `x--` right to left (`:653-731`), then scans forward in tree order through
  the *following* statements (`:494`) for the first read of `x`, which becomes `(x=E)` or, for a
  declarator with one remaining reference, `E` itself (`:347-390`). Target never `const`/`let`/
  `using`/`this`/a lambda name/a property of a constant (`:799-831, 158-176`). A read inside the
  right side of `&&`/`||`/`??`, a `?:` arm or an `if` body entered after the candidate aborts unless
  `x` is local and this read is its only other reference (`:340-346, 354-357, 893-904, 912-924`).
  Hard aborts: any other write to `x` (`:473-476, 405-407`), `await`/`yield`, optional chains,
  `try`/`with`/class/export, any loop but a `for` init (`:318, 325`), undeclared globals
  (`:328-332`). Soft stops: every call, `return`/`throw`, property access when `E` has effects or `x`
  is modifiable from another scope (`may_modify` `:926-937`), reads or writes touching `E`'s names,
  any external side effect when `E` may throw (`:392-413, 939-958`); functions are never entered
  (`:410, 518`); the candidate is removed at `:855-891`; up to 10 rounds (`:233`). `drop_unused`
  (`lib/compress/drop-unused.js:112-507`) then removes what has no reference left.
- **Oxc** `substitute_single_use_symbol_in_statement`
  (`peephole/minimize_statements.rs:1149-1190, 1192-1230, 1305-1730`): the declarator walk this
  folder ports — a single-use `var`/`let` declarator substituted into the next statement's first
  read, including reads inside literals and call arguments, with the same effect ordering rule.
  Oxc has no cross-statement assignment collapse (`:1160`).
- **LilScript**: same-statement collapse `copies.rs:1040-1137` and cross-statement
  `copies.rs:1185-1380` exist but are **beam-only** (`js_peephole/mod.rs:2804-2809`,
  `compiler.rs:8081-8092`), so nothing ships; declarator substitution `returns.rs:322-422` refuses
  an initializer with a call, `[`, `{` or a sibling declarator (`is_pure_read` `:459-507`) and any
  read two statements away; candidates after a `,` or inside a declaration are refused
  (`copies.rs:1240-1270`). The existing folds also *accept* what Terser refuses — a read on the
  right of `&&`/`||`/`??`/`?:` and inside `while` (`prefix_cannot_observe` `copies.rs:1163-1179`) —
  and must be closed before anything here is promoted to canonical.
- **Legality** (objective.md §7): the harvest's census on the four artifacts found 243/244,
  444/445, 231/231 and 140/141 candidate sites with *nothing observable* (call, `new`, member read)
  between the candidate and its read, so the getter-order concern blocks one site per file; the rest
  of our refusals are missing analysis, and `BindingResolution` plus `liveness.rs:56` already compute
  part of Terser's `lvalues` / `may_modify` / `side_effects_external`.

## Claim

A canonical (not beam-only) declarator-substitution fold — Oxc's walk with Terser's right-to-left
order and stop rules — collapses the 42 of 244 micromarklil sites Terser collapses and we leave
(effectful initializers, mid-list declarators, reads inside literals and arguments). Confirms:
**≥ −100 Brotli on micromarklil** (the harvest prices the class at −119 on the compiled file, −151
after the port's esbuild step) **and ≥ −40 on mobxlil** (−62), with jquery (−29) and marked (−40)
not worse, the suite green and every port's own tests passing. Falsifies: **< −40 on micromarklil**,
or any port fleet-positive (031's rule). Follow-ups if confirmed, from the same harvest: the
cross-statement collapse (claim 2, −82 micromark alone, −256 with this one) and the `unused` band
(claim 3, −132 on mobx).

## Read

- `finer/objective.md`, `finer/status.md`, this folder; [043](../043-statement-fusion-is-mostly-absent/README.md) C3; [013](../013-statement-density/README.md) Status line
- `src/js_peephole/folds/returns.rs:300-530`, `copies.rs:1000-1400`, `declarations.rs:840-990`; `src/js_peephole/mod.rs:2790-2820` and `src/compiler.rs:8070-8100` (the beam-only gates)
- Terser `lib/compress/tighten-body.js:278-970` and Oxc `minimize_statements.rs:1149-1230` as the reference implementations

## May touch

- `src/js_peephole/folds/{returns,copies,declarations}.rs`, `src/js_peephole/mod.rs` (fold order), `src/compiler.rs` (only to promote the fold), `src/js_peephole/tests.rs`; this folder; `finer/out/045/`

## Method

Work in an isolated worktree; another agent's fleet A/B may hold the cores — build only when
`pgrep -f "fleet.mjs|release/lilscript|044-bin"` shows nothing that is not yours.

1. Tests first, node as the oracle for every shape: the 42-site classes (effectful initializer
   substituted into the next read; mid-list declarator; read inside an object/array literal or a
   call argument) and Terser's refusals as negative tests (`&&`/`||`/`??`/`?:` right sides, `if`
   bodies, calls between, `await`/`yield`, loops, other writes, `may_throw` with external effects).
2. The fold, canonical, with those rules; close the two accept-what-Terser-refuses holes in the
   existing beam folds on the way (or leave them beam-only and document why).
3. Suite green. micromarklil, mobxlil, jquerylil, markedlil: codec sizes base vs change, one binary
   per side; the ports' own tests.
4. Fleet A/B before "landed", jquerylil and markedlil on four cores.

## Result

| variant | port | sites taken | raw | gzip9 | brotli11 | Δ brotli | tests |
|---|---|---:|---:|---:|---:|---:|---|
| base | micromarklil | 0 | 87117 | 30563 | 26097 | 0 | 1963/1963 |

## Verdict

<open>

## Next

<open>
