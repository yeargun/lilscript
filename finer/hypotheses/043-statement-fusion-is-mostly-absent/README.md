# 043 — statement fusion is mostly absent

**Status: FALSIFIED — C1 and C3 without a line of code: Terser's own `sequences`, removed from its
defaults, costs at most +3 Brotli on any of four ports, and micromarklil's shipped artifact already
carries the fusion because its build re-prints through esbuild `minifySyntax` (+20 when the
compiler's file arrives fused). C2: the `return E,V` fold lands 0 of 162 eligible sites at the
shipped budget and still 0 at 8192 probes; applied by hand it is worth −19 on the compiler's file
and 0 on the shipped one. Terser's band is `collapse_vars`/`unused`: 013's `var ` gap.**
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

Data and harness in `finer/out/043/`; sizes from `lilscript-codec`; `return E,V` AST-counted.
`raw.js` is what the compiler wrote, `esm.js` what ships: `raw.js` through esbuild `minifySyntax`
(`bundle.mjs` reproduces it exactly). Port tests 1963/1963 on the rebuilt dist.

| variant (micromarklil) | `;` | `,` | `if(` | `return E,V` | raw | gzip9 | brotli11 |
|---|---:|---:|---:|---:|---:|---:|---:|
| compiled `raw.js` | 958 | 4952 | 255 | 71, all `if(c)return E,V`; block-terminal **0 of 162** | 93758 | 32693 | 27722 |
| shipped `esm.js` | 643 | 4724 | 146 | 125, 0 sites left | 87117 | 30563 | 26097 |
| `raw.js` → esbuild, `minifySyntax` off | 988 | 4450 | 252 | 71 | 88550 | 30911 | 26408 |
| Terser `sequences` only over `raw.js` | 733 | 5209 | 255 | 244 | 93740 | 32597 | 27610 |
| … then esbuild (the shipped pipeline) | 634 | 4738 | 140 | 126 | 87123 | 30553 | **26117 (+20)** |
| Terser defaults over `raw.js` | 606 | 4962 | 140 | 125 | 91917 | 32101 | 27260 |
| … then esbuild | 569 | 4521 | 123 | 110 | 86032 | 30136 | **25767 (−330)** |
| base: this binary (54ab05a), shipped config; byte-identical | 958 | 4952 | 255 | 71 / 0 of 162 | 93758 | 32693 | 27722 |
| un-starved: `terminal_codec_probe_limit = 8192`; `esm.js` byte-identical | 960 | 4950 | 257 | 71 / 0 of 162 | 93766 | 32698 | 27691 |
| the fold by hand over `raw.js` (`tests.rs` `diagnose_043_…`), 141 sites; `esm.js` byte-identical | 812 | 5098 | 255 | 71 + 141 | 93763 | 32669 | **27703 (−19)** |

Leave-one-out over Terser's defaults: Brotli cost of removing the option (micromarklil after its
esbuild step; the others ship the compiler's file; full table `ablation*.txt`):

| removed | micromark | mobx | jquery | marked |
|---|---:|---:|---:|---:|
| (defaults vs shipped) | −330 | −241 | −359 | −95 |
| `sequences` | **−17** | **+3** | **−8** | **−11** |
| `conditionals` | +21 | +72 | +136 | +24 |
| `if_return` | +9 | −5 | +53 | −30 |
| `collapse_vars` | **+280** | +56 | **+136** | +60 |
| `unused` | **+296** | **+132** | +94 | +29 |
| `sequences` alone, against Terser's reprint | +20 | −20 | −50 | −19 |

## Verdict

**C2 — mechanism located, numbers falsified, value refuted.** The brief counted the wrong scope:
the compiled file has 71 `return E,V`, the shipped one 125 (Terser 125), but all 71 are the
guard-return folds' braceless `if(c)return E,V`; in `fold_expression_suffix_returns`'s own scope —
a block-terminal return after expression statements — the compiler lands **0 of 162** eligible
sites (`b.consume(a);return 92===a?h:g`). `terminal_local_rounds` is not a
config key but a call-site literal: 6 at the one live site (`compiler.rs:6012`, every terminal
finalist), 0 at the two pre-cleanup calls; the walk (`:8140-8195`) is gated by
`codec_budget.reserve_work_unit()`, i.e. `terminal_codec_probe_limit`. Measured
(`LILSCRIPT_TIMING=1`, `*.compile.log`): at the shipped budget 221 codec calls, the cleanup entered
4 times with 254 units in total and **unbudgeted 15 times**; at 8192, 7944 codec calls, 4 entries
with 8097 units — and still **15 unbudgeted**: the 0-round entries (`:5920`, `:5998`) meet a zero
ledger at any ceiling, by the reserve accounting, not the limit. Un-starved, the artifact still has
0 sites (`esm.js` byte-identical). By hand the fold takes 141 sites, passes
`analyze_generated_javascript`, and is worth **−19** on the compiler's file and **0 on what ships**,
because esbuild `minifySyntax` fuses the remaining 157 returns and 53 ifs itself. Whether the walk
ever scored it is moot at −19.

**C1 — falsified before implementation.** Terser's own `sequences` as the ceiling of the five
shapes: +20 on micromarklil (claim ≥ −100); −20 / −50 / −19 against reprint on mobx / jquery /
marked; removing it from Terser's defaults costs at most +3 anywhere; the shipped file has 643 `;`,
so "≥ −500 `;`" is unreachable.

**C3 — falsified.** `if(` on the shipped file moves 146 → 123 (−23) under all of Terser;
`conditionals` is +21..+136 and does not depend on `sequences` (removing it moves ≤ 3). The band is
`collapse_vars` + `unused` — a single-use assignment collapsed into its use and the declarator it
frees — 013's `var ` gap (`var ` 260 → 239 micromarklil, 444 → 416 jquery), the class of our
objective-only `fold_{statement,sequence}_assignments_into_first_use` (beam `compiler.rs:8080-8092`).

**Shipped ≠ compiled, new form.** micromarklil, playcanvaslil and rehype-katexlil ship esbuild's
`minifySyntax` re-print, a post-minifier by objective.md §7: −311 on micromarklil, of which 030
attributed 266 to `!0` re-picking, so ≈ −45 of esbuild's own transforms (109 `if(`, 157 returns)
hide compiler gaps from the fleet number; the other 22 ports ship the compiler's file. status.md's
"−1498 `;` / +932 `,` / −299 `if(` / −344 `var `" is stale: today −54 / −219 / −9 / −25 (shipped),
−352 / +10 / −115 / −21 (compiled).

## Next

1. **Open the `collapse_vars`/`unused` class** (013 → new folder): harvest Terser
   `tighten-body.js:278-1000` (`collapse`), `drop-unused.js:113`; Closure `InlineVariables` /
   `CollapseVariableDeclarations`; Oxc `substitute_single_use_symbol_in_statement`
   (`minimize_statements.rs:1149`); ours `copies.rs:1040, 1185`. Confirms at ≥ −150 on micromarklil
   and ≥ −50 on mobx and jquery.
2. **Decide the post-minifier** (§5, §7; lead 9): the three esbuild `minifySyntax` builds either
   ship the compiler's file (`minifySyntax: false`, the `!0` re-print fixed at the boundary, 030) or
   micromarklil's fleet number stays partly esbuild's.
3. **The zero ledger** (036, lead 7): 15 of 19 cleanup entries are unbudgeted at 330 and at 8192
   probes alike — the reserve accounting, not the ceiling, decides which cleanup calls run.
