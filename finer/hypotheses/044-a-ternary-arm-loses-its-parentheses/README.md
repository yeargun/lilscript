# 044 — a ternary arm loses its parentheses

**Status: OPEN — `fold_common_conditional_arms` moves a comma sequence into a ternary arm without
its parentheses, emitting `a?b,c:d`, and the validator admits it. Latent today; it blocks the −733
of 041.**
Lane: compiler. Objective: brotli (correctness first). Ports: jquerylil, where 041's narrowed rename
exposed it; every port re-measured. Opened: 2026-09-02.

## Prior art

- **Terser** never emits it because parenthesisation is the printer's decision, not the transform's:
  `lib/output.js` `PARENS(AST_Sequence, …)` wraps a sequence whenever its parent is a conditional,
  call argument, unary, binary, property access or arrow body; `AST_Conditional` arms print through
  `print_expression` with that rule. Its `AST_If` → conditional folds (`lib/compress/index.js:1156-1208`)
  build `AST_Sequence` nodes freely and rely on it.
- **Oxc** prints from an AST (`oxc_codegen`, outside `finer/refs/`): `print_expr` takes a precedence
  and wraps a `SequenceExpression` below `Comma` precedence in parentheses.
- **Closure**'s `CodeGenerator` adds parentheses by precedence (`addExpr` with the parent's
  precedence) — the same rule.
- **LilScript** folds text, so precedence is each fold's responsibility. 040 put the printer's
  *adjacency* rule at the one splice point; there is no equivalent for *precedence*. The validator
  (`analyze_generated_javascript`) parses `a?b,c:d` as a valid expression region and does not refuse
  it; node refuses it (`SyntaxError`).

## Claim

The fold at `src/js_peephole/folds/boolean.rs:2352-2360` (`fold_common_conditional_arms`) takes the
text of an arm that is a parenthesised sequence and re-emits it bare. Confirms: a unit shape
`x?(a,b):c` through the fold keeps or restores the parentheses, node runs it, and the artifact 041
produced (`finer/out/041/jquery.esm.narrowed.js`) rebuilt with the fix passes `node --check` with
the same 28571 Brotli as the hand-repaired one. Falsifies: the parentheses are dropped elsewhere
(then the trace names the fold, and it gets the same fix). Second claim: the validator refuses
`?a,b:c` so a future fold cannot ship it — confirms: the 041 narrowed artifact is rejected at
admission before the fix and admitted after.

## Read

- `finer/objective.md`, `finer/status.md`, this folder; [041](../041-the-local-rename-starves/README.md) Result (the failing site) and [040](../040-splices-have-no-adjacency-rule/README.md) (the sibling rule)
- `src/js_peephole/folds/boolean.rs:2300-2400`; the validator's expression-region parser (`parse_expression_regions` in `src/js_peephole/mod.rs`) for where a ternary arm is checked
- Terser `lib/output.js` `PARENS` rules for `AST_Sequence` as the reference

## May touch

- `src/js_peephole/folds/boolean.rs`, `src/js_peephole/mod.rs` (validator), `src/js_peephole/tests.rs`; this folder; `finer/out/044/`

## Method

1. Unit shapes through `fold_common_conditional_arms` and through the whole peephole, node as the
   oracle: `x?(a,b):c`, `x?c:(a,b)`, `x?(a,b):(c,d)`, nested ternaries, a sequence as an arrow body,
   a sequence as a call argument.
2. The validator: `?a,b:c` refused with a named error; the suite still green.
3. Rebuild the 041 narrowed jquerylil artifact through the fixed pipeline (`git apply
   finer/out/041/narrow-the-bail.patch`, port build on four cores); `node --check`, the 039 harness
   6/6, codec sizes.
4. Then 041's Next: suite, fleet A/B with the narrowing, both folders' Result and Status.

## Result

| variant | evidence |
|---|---|
| 041 narrowed build, before | `…?o=…,s=u in e,s&&(r=e[u]):s=o`: `node --check` fails |
| unit shapes | |
| 041 narrowed build, after | |

## Verdict

<open>

## Next

<open>
