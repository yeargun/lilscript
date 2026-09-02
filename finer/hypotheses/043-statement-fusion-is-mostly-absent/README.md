# 043 — statement fusion is mostly absent

**Status: OPEN — of Terser's seven boundary-absorbing shapes we have one, three partial and three
refused by a keyword list; is the `return E,V` fold a silent non-runner at level 13, and is the class
worth ≥ 100 Brotli on micromarklil?**
Lane: compiler. Objective: brotli. Ports: micromarklil first (reprint gain 0, so its −254
"+compress" band is all transforms; −1498 `;` / +932 `,` / −299 `if(` against Terser), then the
fleet. Opened: 2026-09-02.

## Prior art

Harvest of 2026-09-02, rows in [refs/competitor-techniques.md](../../refs/competitor-techniques.md)
Section B (`:80`) and the new B.1 table (`:87-118`).

- **Terser**: `sequencesize` (`lib/compress/tighten-body.js:1253-1286`) joins `a;b` into `a,b`;
  `sequencesize_2` (`:1305-1373`) absorbs a preceding simple statement into the next `return`,
  `if`, `for` init, `for-in` object, `switch` discriminant or `throw`. Refusals: the predecessor must
  be an `AST_SimpleStatement` (`:1371`); no declaration in a `for` init (`:1319`); a predecessor with
  an unparenthesised `in` is refused before a `for` (`:1320-1329`); `let`/`const` heads refused
  (`:1339`); a bare `return` gets `void 0` synthesised (`:1316-1317`) and dropped again later
  (`index.js:3868-3873`); passes capped at 800 runs (`index.js:330`). Then the folds that pay:
  `AST_If` (`index.js:1156-1208`), `lift_sequences` (`:2091-2101, 2217-2245, 3078, 3235-3240`),
  `AST_Sequence` (`:2053-2089`), `collapse` (`tighten-body.js:278, 713-714, 774-776`).
- **Oxc 0.147.0**: no `statement_fusion.rs`; the same rules live per handler in
  `peephole/minimize_statements.rs:448-455, 593-600, 668-676, 822-832, 847-854, 919-937, 986-1023`
  (`result.last_mut()` matched as `ExpressionStatement`); `var` merging kept separate (`:938-963`);
  no `for-of` rule; never absorbs into a bare `return`, sinks the argument backwards instead
  (`:803-807`); per-handler `substitute_single_use_symbol_in_statement` (`:1149`) is what fires next.
- **Closure**: `StatementFusion.java` (not vendored) — the same seven shapes.
- **LilScript**: `a;b`→`a,b` PRESENT (`js_peephole/folds/calls.rs:132-229`, top-level deliberately
  last `:137-146`). `e;return x` PARTIAL: `control.rs:1913-1927` (`fold_expression_suffix_returns`)
  is block-terminal only and a search-only per-site variant that runs only when
  `terminal_local_rounds > 0` — 6 at `compiler.rs:6012`, **0 at `:5920`, `:5998`, `:15575`**, and
  budget-charged at `:8153`. `e;if(c)` PARTIAL: guard-return shapes only (`control.rs:412-490`,
  `codegen_ir_js.rs:17198-17226, 10578-10592`). `e;for(i;;)` PARTIAL: bare `name=rhs;` runs only,
  no `in` guard (`loops.rs:2104-2259`). `e;switch(x)`, `e;for(k in o)`, `e;throw x` ABSENT: refused
  by the keyword skip list at `calls.rs:183-209` (`:196`, `:187`, `:194`). Ours would need Terser's
  refusals plus the line-terminator bail (`control.rs:1977, 2360`), parenthesising `for(k in(e,o))`,
  the directive-prologue bail (`calls.rs:212-220`), the `continue`/`case` bars
  (`keyword_space_tests.rs:680, 693`), and `tests.rs:428` (a sequence member is not a condition).

## Claim

**C2 first, the cheap one.** `fold_expression_suffix_returns` is a silent non-runner at the shipped
level 13, the class of 036/037/041. Confirms: the shipped micromarklil artifact has **< 10**
`return E,V` sites where Terser's output of the same file has **≥ 100**, and a build with
`terminal_local_rounds = 6` recovers most of them. Falsifies: the shipped artifact already carries
them — then 035's "no" was a probe artifact and C1 is the whole story.

**C1.** The five absorbed shapes, added as one codec-voted whole-artifact candidate (not canonical,
so the portfolio rule of 031 holds), move micromarklil **≥ −100 Brotli and ≥ −500 `;`**. Falsifies:
**> −30 Brotli**, or any port fleet-positive.

**C3, the mechanism of value.** The bytes are in what fires after the boundary is gone, not the
comma: `if(` on micromarklil drops by **≥ 150** of the −299 through `fold_if_expression_to_and` /
`fold_conditional_return_tails`. Falsifies: `if(` moves **< 50** because bodies stay
multi-statement for `var` reasons — then the −344 `var ` gap (013) is the prerequisite.

## Read

- `finer/objective.md`, `finer/status.md`, this folder; [035](../035-where-the-compiler-headroom-is/README.md) Lead 1; [013](../013-statement-density/README.md) Status line; [031](../031-admission-blocks-the-class-rewrite/README.md) Status line (the portfolio rule)
- `src/js_peephole/folds/calls.rs:120-230`, `control.rs:400-500, 1900-1990`, `loops.rs:2100-2260`; `src/compiler.rs:5910-6020, 8140-8160, 15570-15580`
- Terser `lib/compress/tighten-body.js:1253-1373` as the reference implementation

## May touch

- `src/js_peephole/folds/{calls,control,loops}.rs`, `src/compiler.rs` (the candidate wiring only), `src/js_peephole/tests.rs`; this folder; `finer/out/043/`

## Method

Host: one heavy hypothesis at a time; check `pgrep -f "release/lilscript"` before any build and
never share cores with a fleet pass. micromarklil's build is ~6 min on 8 cores.

1. **C2**: count `return\s*[^;]*,` sites (a `return` whose expression is a top-level sequence) in
   `../micromarklil/dist/micromark.raw.js` and in Terser's output of that same file
   (`compress: {defaults}, mangle: false`, `finer/out/043/`). Then one build with
   `terminal_local_rounds = 6` in a scratch copy of the port's config; count again; codec sizes.
2. **C1**: implement the five shapes as one fold family behind the existing candidate machinery
   (a whole-artifact variant the codec votes on), with Terser's refusals as tests (`tests.rs`,
   node as the oracle for every shape). Suite green. micromarklil: `;`, `,`, `if(` counts and codec
   sizes, base vs candidate-on.
3. **C3**: on the winning artifact, attribute the delta: how much comes from `if(` folds firing.
4. Fleet A/B before "landed", in the folder's terms, with jquerylil and markedlil on ≥ 4 cores.

## Result

| variant | port | `;` | `,` | `if(` | `return E,V` | raw | gzip9 | brotli11 | tests |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| shipped | micromarklil | | | | | 87117 | 30563 | 26097 | 1963/1963 |
| Terser over shipped | micromarklil | | | | | | | 25397 | |

## Verdict

<open>

## Next

<open>
