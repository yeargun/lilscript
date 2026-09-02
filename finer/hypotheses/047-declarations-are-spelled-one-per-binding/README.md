# 047 — declarations are spelled one per binding

**Status: CONFIRMED, landed (e0c1c22) — katexlil's shipped ESM 66819 → 65586 Brotli (−1233), the
portfolio +4 / +13 / −13, every gate green. Most of it is not the declaration folds (−320 codec
bytes as a scored late candidate) but the whole-artifact canonical peephole candidate finally
entering the beam: two validators refused it wrongly (a declarator scan that ran past an elided
block-terminal semicolon; the class rewrite's `constructor` counted as a new property) and one fold
behind it was wrong (`fold_while_trailing_increments` lifting an increment out of an `if/else`
arm). Measured on feature/source-maps + the patch, which has the terminal parser gate main lacks.**
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
- **esbuild** `mangleStmts` and **Closure** `CollapseVariableDeclarations` (neither vendored) do the join.
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

Hand rewrites of the shipped ESM with AST positions (046's `decl.v1/v2/v3`); the folds with node
as the oracle; then the same binary, pool builds of katexlil, mobxlil, micromarklil and
remark-gfmlil into a separate dist dir, codec against 046's build B and the base binary's own
rebuild of the three, every port's tests.

```sh
cargo test --release --lib -- js_peephole   # the fold, resolver and lift tests
node finer/tools/workers.mjs build --ports katexlil,mobxlil,micromarklil,remark-gfmlil --dist-dir finer/out/047/dist --compiler .claude/worktrees/agent-a5be67e76a2b44fdd/target/release/lilscript
```

## Result

Compiler main 20f4e09 plus this folder; base binary feature/source-maps 4e799a8 (byte-identical
output). Sizes `lilscript-codec`; portfolio ports rebuilt on the pool with the base binary first
(they reproduce `finer/out/044/scoreboard.new.json` exactly: 15578 / 26097 / 10559).

| variant | raw | gzip9 | brotli11 | counters / CPU | tests |
|---|---:|---:|---:|---|---|
| katexlil ESM, 046 build B (base) | 289019 | 80966 | 66819 | | 17/17, Jest 1230/1230 |
| hand rewrites (AST positions, legal shapes): initializer off / + joins / + assignments folded | | | −88 / −240 / −388 | 225 / 489 / 78 sites; `finer/out/047/` | |
| **C**: folds unconditional in the search's peephole | 289019 | 80966 | 66819 | katexlil **byte-identical** — corrected below: the peephole does see the whole artifact, but every peepholed candidate was refused by the resolver defect, so the port was shipping un-peepholed output | |
| C on mobx / micromark / remark-gfm | −77 / −31 / −35 | | **−12 / +64 / +41** | no joins there either; the `void 0` drops perturb the local rename (remark-gfm: every short name reassigned) | |
| **D**: the folds as a scored late-cleanup family (`shape_declarations`) | 285228 | 80423 | **66464** (−355) | compiler output 227668 / 56880 (−434); `cleanup_shaped_pushed=1`, 320 codec bytes | 17/17 |
| D on mobx / micromark / remark-gfm | +7 / −99 / −78 | | **+10 / +11 / +13** | one probe per beam member, taken from a default budget the later families then lack | |
| whole-artifact peephole over build B, offline (`optimize_generated_javascript`) | 222374 | 67651 | **56125** (−1189) | 8 → 28 `class`, 213 → 96 prototype assigns, 29 → 0 `arguments[`; parses; this is what the late cleanup's canonical candidate would be | |
| **E**: + the resolver fix (`declarator_names` stops at an unbalanced closer), pool | 279731 | 79534 | **65589** (−1230) | compiler output 222171 / 56122 — the whole-artifact rewrite lands (8 → 5 `class`, 213 → 96 prototype assigns, 29 → 0 `arguments[`, 101 → 0 `+literal`) | 17/17, Jest 1230/1230 |
| E on mobx / micromark / remark-gfm | 0 / −99 / −88 | | **+10 / +64 / −22** | micromark: the shaped candidate is renamed differently from the original (short-name churn), the family ran before the rename | |
| **F**: shaping after the rename; canonical candidate admitted through `validate_selected` | | | ESM 65618 but **invalid** | `cleanup_canonical_pushed=1`; `fold_while_trailing_increments` hoisted the `h++` ending an `else` arm into the `for` header and left `if(c)…;else}` — esbuild refuses it, 14/17 port tests fail. Main has no terminal-parser gate (that is feature/source-maps 4e799a8), so admission let it through | 3/17 |
| F on mobx / micromark / remark-gfm | 0 / −36 / −88 | | **0 / +13 / −22** | the rename-first order removes the naming churn | |
| **G1**: F + the lift guard (`increment_is_body_level`), built on feature/source-maps (the parser gate), pool | 279814 | 79536 | **65586** (−1233) | compiler output 222254 / 56236; `cleanup_canonical_pushed=1` | **17/17, Jest 1230/1230** |
| G1 on mobx / micromark / remark-gfm | +1 / −36 / −65 | | **+4 / +13 / −13** | fleet net −1229 | |
| **G2**: G1 + the port's `JS.push` → `JS.invoke(…, "push", …)` (140 sites) | 283262 | 80308 | 66282 (**+696 against G1**) | raw +3448: the intrinsic (`Array.prototype.push.call` by definition) is what the compiler's array families fold — fresh-array pushes into literals, `alias-array-prototype-methods` — and an opaque method call is not. The 86 prototype calls left in G1 are the ones nothing could fold. **Reverted**; not a port lever | |

**Correction.** `top_level_declaration_variants` spells the *whole* artifact; the search's
peephole sees it all. C was byte-identical because, with `_` ambiguous, every peepholed candidate
failed admission at the search level too: katexlil had been shipping essentially un-peepholed
output, and E's 29 `arguments[` sites and classes are that peephole landing, not the late one.

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

**Also taken:** `+7227`, `a-+1` — a number coercion left on a literal (101 sites; Terser
`evaluate`, Oxc `constant_evaluation/mod.rs:489`): `fold_unary_plus_on_numeric_literal`.

## Verdict

Confirmed, and the claim was the smaller half of the answer. The declaration folds are worth 320
codec bytes on katexlil as a scored late candidate and nothing when applied per declaration; the
−1233 comes from the late cleanup's canonical whole-artifact candidate, which every earlier
katexlil compile refused for a resolver defect nobody could see (`rename_candidates=1` was the only
trace; the new `cleanup_canonical_*` counters name the exit now). What status.md carries: (1) on a
port with many module globals the per-declaration peephole cannot join anything, and the late
canonical candidate is the only path for cross-declaration folds — its counters are the first
thing to read; (2) main's admission has no terminal parser gate, so a wrong fold ships on main and
is merely refused on feature/source-maps: compiler changes are measured there; (3) unconditional
raw-motivated folds perturb the local rename on other ports (remark-gfm +41 from name churn
alone) — a fold that is not obviously codec-neutral goes in as a scored candidate after the
rename; (4) `JS.push` is not a port smell: the intrinsic is what the array families fold.

## Next

Budget is not the limiter here: katexlil at `terminal_codec_probe_limit = 512` enters all three
late cleanups (`cleanup_entered=3`, canonical pushed 3, shaped pushed 2 / lost 2 / refused 3) and
is **byte-identical** to G1 at 256 (1024 too). The gap left to Terser of the published graph is
+2542, to the source lane +3828. Terser still extracts −811 from G1's ESM (was −1234; `unused`
+241, `collapse_vars` +175, `evaluate` +162 — 045's class and constant evaluation;
`finer/out/047/terser-on-g1.txt`); the ~1.7 KB no post-minifier reaches is the port's `JsValue`
shape (046), and typing the port is the next step.
