# 047 — declarations are spelled one per binding

**Status: OPEN — three generic declaration folds landed in the peephole (initializer `void 0`
dropped off fresh `var` bindings, adjacent declarations joined, the var-into-assign fold widened
and repeated); unit tests green; katexlil and a three-port portfolio building on the pool.**
Lane: compiler. Objective: brotli. Ports: katexlil (found there), mobxlil, micromarklil,
remark-gfmlil (portfolio). Opened: 2026-09-02.

## Prior art

- **Terser** `join_vars` (on by default, `compress/index.js:250`): `join_consecutive_vars`
  (`compress/tighten-body.js:1440-1500`) concatenates adjacent `AST_Definitions` of one type and
  pulls a following `var` into a `for` initializer; `join_object_assignments` (`:1376`) folds
  `o.k=v` statements after `var o={…}` into the literal. The `var x=void 0` initializer is not a
  join case there: `reduce_vars` marks it a dead store and `unused` drops it. Ablated on our
  katexlil artifact (046): `join_vars` off costs Terser +140 Brotli / +1924 raw, `unused` +362,
  `reduce_vars` +123 — the three are one class on this shape.
- **Oxc** `handle_variable_declaration` (`OXC/peephole/minimize_statements.rs:352-410`): every
  declarator is appended to the previous declaration when the kinds match (`join_vars`), unused
  declarators are dropped and side-effecting initializers kept as statements.
- **esbuild** `mangleStmts` (not vendored — verify) merges adjacent declarations of one kind and
  `var x;x=v` pairs into `var x=v`.
- **Closure** `CollapseVariableDeclarations` (not vendored — verify) does the join; `Normalize`
  splits first and the collapse re-joins after other passes.
- **LilScript** had `fold_uninitialized_var_into_assign` (`LS/js_peephole/folds/declarations.rs:546`)
  for `var x;x={…}` — but the emitter writes `var x=void 0` for a `JsValue x = undef()` global (an
  explicit initializer, faithfully printed), one `var` statement per binding, and the fold refused
  both the initializer and any non-adjacent assignment. On katexlil that is 231 `=void 0;`
  initializers, 674 `;var ` boundaries and 78 assignments that could fold in. Rows added to
  refs/competitor-techniques.md §B.1.

## Claim

The three folds are worth ≥ −150 Brotli on katexlil's shipped ESM (the hand rewrite measured −240
for the first two and −388 with the third, before interaction with the rest of the pipeline) and
≥ 0 on each portfolio port; every port's tests stay green and every artifact passes `node --check`.
Falsifies: a portfolio port grows by more than 20 Brotli (the join changes a declaration layout the
codec had preferred), or the search's own declarator reorder (`reorder_uninitialized_var_declarators`)
and this join fight and the net is under −50 on katexlil.

## Read

- `finer/objective.md`, `finer/status.md`, [046](../046-katexlil-is-untyped-so-closed-equals-open/README.md) Result
- `LS/js_peephole/folds/declarations.rs` (the three folds, at the end of the file and `:546`),
  `LS/js_peephole/mod.rs:2560-2568` (session order)
- `TERSER/compress/tighten-body.js:1440-1500`; `OXC/peephole/minimize_statements.rs:352-410`

## May touch

- `src/js_peephole/folds/declarations.rs`, `src/js_peephole/mod.rs`, `src/js_peephole/tests.rs`;
  this folder; `finer/out/047/`; refs row; status/log

## Method

1. Hand rewrites of the shipped ESM with AST positions (legal shapes only), codec each: initializer
   off; + joins; + assignments folded (046's `decl.v1/v2/v3`).
2. The folds, unit-tested with node as the oracle (loop bodies, earlier writes, duplicate
   declarators, `export`, `for(` heads, `let`/`const`, a chain of four module globals).
3. Same binary (main 20f4e09 + this change), pool build of katexlil, mobxlil, micromarklil,
   remark-gfmlil into a separate dist dir; codec against 046's build B (katexlil) and
   `finer/out/044/scoreboard.new.json` (the others, built on 20f4e09); each port's tests.

```sh
cargo test --release --lib -- js_peephole::tests::drops_void_initializers_only_where_the_binding_is_fresh_and_runs_once js_peephole::tests::joins_adjacent_declarations_of_one_kind js_peephole::tests::module_globals_declared_then_assigned_become_one_declaration
node finer/tools/workers.mjs up 2 && node finer/tools/workers.mjs build --ports katexlil,mobxlil,micromarklil,remark-gfmlil --dist-dir finer/out/047/dist --compiler target/release/lilscript
```

## Result

Compiler main 20f4e09 plus this folder; base binary feature/source-maps 4e799a8 (byte-identical
output). Sizes `lilscript-codec`; portfolio ports rebuilt on the pool with the base binary first
(they reproduce `finer/out/044/scoreboard.new.json` exactly: 15578 / 26097 / 10559).

| variant | raw | gzip9 | brotli11 | counters / CPU | tests |
|---|---:|---:|---:|---|---|
| katexlil ESM, 046 build B (base) | 289019 | 80966 | 66819 | | 17/17, Jest 1230/1230 |
| hand: initializer off | −1575 | −102 | −88 | 225 sites | |
| hand: + joins | −3531 | −299 | −240 | 489 joins | |
| hand: + assignments folded | −3762 | −500 | −388 | 78 folds; the rewrite itself mis-nested once | |
| **C**: folds unconditional in the per-declaration pass | 289019 | 80966 | 66819 | katexlil **byte-identical**: the pass sees one declaration at a time, so there is never a second `var` to join | |
| C on mobx / micromark / remark-gfm | −77 / −31 / −35 | | **−12 / +64 / +41** | the joins never fire there either; the `void 0` drops and the widened fold perturb the local rename (remark-gfm: every short name reassigned) and lose a repeated `void 0` match (micromark) | |
| **D**: the folds as a scored late-cleanup family (`shape_declarations`) | 285228 | 80423 | **66464** (−355) | compiler output 227668 / 56880 (−434); `cleanup_shaped_pushed=1`, 320 codec bytes | 17/17 |
| D on mobx / micromark / remark-gfm | +7 / −99 / −78 | | **+10 / +11 / +13** | the family reserves one probe per beam member; on a default budget (`level_limit` scaled to 1/4–1/12 by artifact size) those probes starve the later families | |
| whole-artifact peephole over build B, offline (`optimize_generated_javascript`) | 222374 | 67651 | **56125** (−1189) | 8 → 28 `class`, 213 → 96 prototype assigns, 29 → 0 `arguments[`; parses; this is what the late cleanup's canonical candidate would be | |
| **E**: + the resolver fix (`declarator_names` stops at an unbalanced closer), pool | 279731 | 79534 | **65589** (−1230) | compiler output 222171 / 56122 — the whole-artifact rewrite lands (8 → 5 `class`, 213 → 96 prototype assigns, 29 → 0 `arguments[`, 101 → 0 `+literal`) | 17/17, Jest 1230/1230 |
| E on mobx / micromark / remark-gfm | 0 / −99 / −88 | | **+10 / +64 / −22** | micromark: the shaped candidate is renamed differently from the original (short-name churn), the family ran before the rename | |
| **F**: shaping after the rename; canonical candidate admitted through `validate_selected` | | | ESM 65618 but **invalid** | `cleanup_canonical_pushed=1`; `fold_while_trailing_increments` hoisted the `h++` ending an `else` arm into the `for` header and left `if(c)…;else}` — esbuild refuses it, 14/17 port tests fail. Main has no terminal-parser gate (that is feature/source-maps 4e799a8), so admission let it through | 3/17 |
| F on mobx / micromark / remark-gfm | 0 / −36 / −88 | | **0 / +13 / −22** | the rename-first order removes the naming churn | |
| **G1**: F + the lift guard (`increment_is_body_level`), built on feature/source-maps (the parser gate), pool | | | | | |
| **G2**: G1 + the port's `JS.push` → `JS.invoke(…, "push", …)` (140 sites; `JS.push` *is* `Array.prototype.push.call` by definition) | | | | | |

**Why the canonical candidate never lands on katexlil.** The late cleanup re-opens the canonical
peephole on the whole finalist and admits it as one candidate (`compiler.rs`
`apply_late_javascript_cleanup`). With the new counters: `cleanup_canonical_refused=1`,
"unresolved generated export binding" at `_ as __defineFunction`. Reproduced offline on build B:
`BindingResolution` marks `_` and `j` *ambiguous* at module scope because `declarator_names`
(`binding.rs`) has no case for an unbalanced closer — a block-terminal `var c=…,e=!0}` whose
semicolon the emitter elided (`elide-block-terminal-semicolons` is a scored family) let the scan
run on through the rest of the function and read `,_(j),j={…}` at module level as declarators.
Every whole-artifact candidate on this port was refused for it, on the old binary too
(`rename_candidates=1`: a beam of one). One `")" | "]" | "}" => break` arm and a regression test.

**Two more defects on the way to the canonical candidate.** (1) Admitted through `validate`, the
candidate was refused a second time for "unclassified static properties: [constructor]": the class
rewrite's own keyword, which 031 already exempts for the one re-check of a selected artifact
(`validate_selected`); the late cleanup's canonical candidate is that re-check, and it is
codec-scored before it enters the beam, so it now uses it. (2) Admitted, it was invalid:
`fold_while_trailing_increments` (`loops.rs`) lifted the `h++` that ends an `else` arm into the
`for` header and left the other arm's `h++` in place plus a bare `else}` — a wrong program that the
main branch's admission accepts (no terminal parser gate). `increment_is_body_level` now refuses
an increment nested in any control arm, ternary arm or arrow; node-oracle test.

**Also found and taken (generic, same session):** `+7227`, `a-+1`, `b/+18`, `b=+0` — the number
coercion the emitter writes for a `JsValue` operand survives constant propagation onto a literal:
101 sites on katexlil; Terser's `evaluate` and Oxc's `constant_evaluation/mod.rs:489` fold it;
`fold_unary_plus_on_numeric_literal` in `integers.rs` now does (`a+ +1` and `++` guarded).

## Verdict

<pending the pool build>

## Next

<pending>
