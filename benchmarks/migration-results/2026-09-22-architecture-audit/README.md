# Does the new architecture deliver? (2026-09-22)

The owner's question, after milestones 009–013 reached the reference ports: the compiler was rebuilt around a typed semantic program, so it should compile faster and, being better typed inside, optimize better. Does it, and is the migration plan still right?

Everything here ran on this host (B8als_v2, shared with other sessions, so wall times carry a few percent of noise). The compiler is commit `577d472d`, binary SHA-256 in [data/compiler.sha256](data/compiler.sha256). Sizes come from the repository codec (Brotli 11, lgwin 22; gzip 9). Ports are scratch copies of `~/probelil`, `~/markedlil`, `~/zodlil` and `~/katexlil`, with the recorded migration patches applied to the semantic copies. Scripts are in [data/scripts/](data/scripts/).

## 1. Compile time: faster by architecture, but the saving is not being reinvested

Each cell is wall seconds / Brotli bytes. `l0` is the port's configuration with `optimization_level = 0` and the search switched off ([cfgvar.py](data/scripts/cfgvar.py)). `prod` is the port's own configuration. Raw rows: [timing-matrix.txt](data/timing-matrix.txt).

| Port | Semantic, l0 | Semantic, prod | Default route, l0 | Default route, dev | Default route, prod |
|---|---|---|---|---|---|
| probelil | 0.02 s / 2,424 | 0.24 s / 1,863 | 0.25 s / 1,624 | 0.22 s / 1,556 | 22–27 s / 1,492 |
| markedlil | 0.14 s / 11,312 | 0.81 s / 9,397 | 1.69 s / 10,179 | 2.02 s / 9,818 | 149 s / 9,360 |
| zodlil | 0.62 s / 38,843 | 2.83 s / 28,326 | 2.38 s / 33,995 | 6.49 s / 31,834 | 186–194 s / 29,682 |
| katexlil | 1.00 s / 67,820 | 3.49 s / 56,132 | 4.34 s / 59,999 | 32.1 s / 58,829 | 352 s / 55,404 |

What the rows say:
- **The 50–100× headline in 012 compares a searching route with a non-searching one.** With the search off on both routes, the semantic compiler is 3–12× faster. It also produces smaller output on the three module ports (markedlil 9,397 against 10,179; zodlil 28,326 against 33,995; katexlil 56,132 against 59,999).
- **The default route is slow because every candidate is a full re-emission from IR.** markedlil production: 267 emissions take 469 s of CPU (1.8 s each), against 16 s for all 573 codec calls ([explain](data/markedlil-default-route-production-explain.txt)). probelil: 381 emissions, 62 s of CPU out of 71 s ([timing](data/probelil-default-route-production-timing.log)).
- **The semantic route renders a candidate in about 14 ms** (katexlil: 6 prints in 84 ms), two orders of magnitude cheaper per candidate. Its production time is now mostly six final Brotli encodes: 2.48 s of katexlil's 3.5 s ([timing](data/katexlil-semantic-production-timing.log)). The compiler itself takes about 1 s.
- **The default route's search still runs out of budget.** On markedlil it exhausts its 264 proposals and leaves 44 of its scored emission families starved. That search buys 4–7% over the same route without it: markedlil 9,818 → 9,360, zodlil 31,834 → 29,682, katexlil 58,829 → 55,404.
- **The semantic search tries 6 candidates, because nothing proposes alternatives.** This is 010's "saturated" finding seen from the cost side. The same 264-candidate search would cost about 20 s of CPU on markedlil at the semantic route's render cost, against the default route's 506 s.

## 2. Optimization: the gains are real, but they come from a minifier on the target tree, not from types

**Where the semantic bytes come from.** 009's ablation, re-run after batch 5, shows it. Target compaction, naming and liveness carry the output. The typed proof families (scalar replacement, leaf-helper inlining, constant folding, call specialization, string pooling) add 0–48 bytes on every reference port ([009 ablation](../../../docs/migration/index.md#009-ablation-re-run-after-batch-5-2026-09-22)). The 009–013 batches are generic rewrites of the finished JS tree: forwarding, inlining, store folding, namespace flattening and spellings. Terser does the same kinds of rewrite.

**What a general minifier still finds in our output** ([terser-over-our-output.txt](data/terser-over-our-output.txt)): Terser (compress passes 2 + mangle, toplevel) applied to our own artifacts.

| Port | Ours | After Terser | Verdict |
|---|---|---|---|
| probelil | 1,863 | 1,700 | 8.7% of minifier slack left |
| markedlil | 9,397 | 9,506 | Terser makes it larger: no minifier slack |
| zodlil | 28,326 | 28,893 | Terser makes it larger: no minifier slack |
| katexlil | 56,132 | 55,195 | 1.7% left, mostly single-use functions (009's leave-one-out) |

On markedlil and zodlil the semantic output is past what minification can reach. Further bytes there must come from what a minifier cannot know: types, purity, layouts, closed-world names. That is the architecture's promise, and it is not being used yet.

**What the default route's IR optimizer buys** at `l0` ([default-route-pass-ablation.txt](data/default-route-pass-ablation.txt)). Each cell is Brotli bytes added when one pass is switched off.

| Pass off | probelil | markedlil | zodlil | katexlil |
|---|---|---|---|---|
| all optional passes (`preset = "none"`) | +316 | +89 | +1,218 | +4,258 |
| inlining | +271 | +42 | +521 | +645 |
| global optimization | +28 | 0 | +435 | +2,200 |
| constant folding | +18 | −19 | +31 | +44 |
| scalar replacement | +10 | 0 | 0 | 0 |

**probelil is the typed canary, and it shows the gap.** Compiled as a module, the semantic route gives 1,836 against the default route's 1,492. The script-frame rule (keeping frames a sloppy caller could observe) accounts for only 27 of the 371 bytes. The rest is typed optimization the default route does and the semantic route does not:
- inlining once-called functions into their caller;
- specializing calls with constant arguments (`f(a)` with defaults 10 and 100 becomes `a+110`);
- positional storage for `Shape`, where the semantic route keeps `{width:0,height:0}` plus setter functions;
- no `|0` on reads of int-typed fields.

**Shape counts** (semantic / default route):

| | probelil | markedlil | zodlil | katexlil |
|---|---|---|---|---|
| `\|0` | 175 / 143 | 84 / 56 | 254 / 26 | 119 / 20 |
| `if(` | 18 / 9 | 348 / 162 | 1,459 / 637 | 1,115 / 419 |
| declarations (`let/var/const`) | 72 / 21 | 238 / 112 | 852 / 576 | 1,238 / 544 |
| `;` | 205 / 254 | 788 / 445 | 2,866 / 1,835 | 4,283 / 1,588 |
| `++`/`--` and `op=` | 0 / 0 | 28 / 91 | 1 / 191 | 21 / 183 |
| `new RegExp` | – | 118 / 20 | – | – |

Measured levers from these counts:
- **Regex literals.** A constant `new RegExp("…")` becomes `/…/` under pristine builtins ([regex-lit.mjs](data/scripts/regex-lit.mjs)). markedlil −85, zodlil −18, katexlil +24. This one rewrite would put markedlil below the default route's full search (9,312 against 9,360), and katexlil's +24 shows it must be a scored choice, not a rule.
- **`|0` elision.** Removing every `|0`, which is unsound and so only a ceiling: probelil −73, markedlil −34, zodlil −79, katexlil −4. The typed subset is worth doing for raw size and runtime, but it is a small Brotli lever.
- **Statement density.** Two to three times as many `if`s, declarations and semicolons. The default route turns these into expressions, merged declarations and update operators, and its search chooses where each pays. On the semantic route those are spellings with no search behind them.

## 3. The plan's own instrument

`node finer/tools/migration-progress.mjs` exits 1 with 46 findings ([output](data/migration-progress-validator.txt)):
- 40 accepted receipts for 003–007 pin input files that have changed since. The receipts are stale by the plan's own rule.
- 6 progress rows (008–013) use states the plan does not declare ("implemented." with a period, "partial", "open").

## 4. What the code says

A read-only audit of `src/semantic_program/` and `src/structured_js/`, done the same day, explains the measurements. File and line references are to commit `577d472d`.

**The only interface is an untyped tree.** Formation hands `structured_js` a `js::Module` (`structured_js/mod.rs:954-984`):
- `Binding` carries its source symbol, scope and spelling, but no type (`mod.rs:924-932`).
- `Function` carries `arrow`, `strict`, `name`, `length` and `suspension`, but no effect or purity summary (`mod.rs:899-915`).
- The typed operation nodes (`ToInt32`, `IntBinary`, `IntNegate`, `Intrinsic`) and one `pristine_builtins` flag are the only semantic knowledge in it.

The edits that produce every 009–013 byte run at the end of formation (`semantic_program/javascript.rs:639-726`). Their inputs are the tree plus `strict`, `pristine` and `prunes`.

**They re-derive legality from JS syntax.** Examples:
- `inert` (`inline.rs:500-522`) rejects `a+b` even when both operands are checked ints.
- `runs_no_user_code` (`inline.rs:536-575`) rejects any call.
- "A binding no other function mentions" is a syntactic reach (`inline.rs:72-76`, `651-730`).
- Temporal-dead-zone guards use first mentions in a region (`mod.rs:1605-1637`).

None of them consults a type, an effect, purity or initialization order. The program has some of these facts; formation throws them away.

**The typed facts that exist are too weak to help** (`semantic_program/facts.rs`):
- Every call, member or index load, store and `CheckPlace` has `UNKNOWN` effects (`facts.rs:1073-1166`).
- Parameter and field loads never count as primitive: "Source annotations are language knowledge, not a proof of the raw value in a JS cell" (`facts.rs:1340-1352`).
- There are no range facts, no nullness beyond the static type, and no escape or alias analysis. The checker's `EscapeState` is never imported.
- There is no function effect summary. The source `pure` modifier (`declared_pure`, `semantic.rs:623`) is referenced nowhere in `semantic_program/` or `structured_js/`.

**The typed families admit almost nothing.**
- The helper family's leaf bodies may contain no call, intrinsic, branch, loop or member access (`helper_family.rs:1494-1551`).
- Scalar replacement refuses classes and generic structs (`product_family.rs:708-710`).
- String pooling skips literal producers and refuses cross-unit definitions (`search_opportunities.rs:156-199`, `string_family.rs:244-289`).
- The publication rewrites have no production caller.

**Typed constructs, as lowered:**
- Every int field, `int[]` element and int call result gets `ToInt32` "until producer facts establish that this normalization is redundant" (`javascript.rs:76-131`, `2326-2328`). No producer facts exist.
- `IntBinary` keeps `|0` unless a local `NumberFacts` range (literals, loop counters, lengths) proves it redundant (`javascript.rs:3674-3690`).
- Classes become objects keyed by declared field names, with no positional layout, no renaming and no scalarization (`from_source.rs:1905-1956`).
- `?.` becomes a ternary, and `??null`/`??""` are always emitted (`javascript.rs:118-129`).
- Equality does use declared types: same-class primitives compare with `==` (`javascript.rs:3707-3731`).
- Method dispatch is static by construction (`from_source.rs:536-545`).

The default route has what the semantic route lacks here:
- per-value, interprocedural and owned-field `I32Range` facts (`value_analysis.rs:164-327`);
- consumer-based `|0` elision (`codegen_ir_js.rs:24960-25050`);
- a function effect summary with pure-call removal (`optimizer.rs:11927-12158`, `compiler.rs:17804-17813`);
- positional class layout and opt-in property mangling (`codegen_ir_js.rs:13715-13729`, `6498-6520`);
- nullable-read elisions (`codegen_ir_js.rs:18351-19902`).

**Contract violations and duplicated analyses (A1/A2):**
- Foreign cells print as `Expr::Host(name)`, and the inliner decides "pristine standard global" by that spelling (`javascript.rs:2129`, `inline.rs:19-50`).
- Formation reads its own emitted tree back: `length_member` and `numeric()` (`javascript.rs:933-945`, `javascript_host.rs:236-259`).
- `debugLog` is recognized by its source name in two places.
- The contract's `world` and `pure_property_reads` are never read.
- Effects are modeled four times (`facts.rs:172`, `analysis.rs:42-66`, `inline.rs:500-575`, `mod.rs:1139`). Int ranges, constant folding, inlining, dead code, call counting, temporal dead zones and captures each have two or three owners.

**Code only tests reach.** `structured_js::lower::lower_slice`, the AST lowering that builds an `AnnotatedTree`, is called only from tests (`lower.rs:5595`, `semantic.rs:9709`, `structured_js/lower.rs:1701`). So `analysis.rs`, `flow.rs`, `optimize.rs`, `plan.rs`, `constants.rs` and `compact.rs` never run in production, about 4,000 lines plus the AST half of `lower.rs`. `selection.rs` is live, since `compiler_service.rs` uses it.

## 5. Answers

- **Faster compilation: yes, by architecture.** It is 3–12× at equal effort and about 100× per candidate. The saving is unspent, because the search has nothing to choose among.
- **Better optimization from better typing: not yet.** The semantic route beats the default route at equal effort on three module ports, but with minifier techniques. Its typed knowledge stops at formation. It is weaker than the default route's on every typed lever measured here: ranges, effects, purity, layouts and names. The default route's search then buys the 4–7% that puts it ahead on three of four ports.
- **Is the plan stale:** yes, in its starting point, its next action, its compute assumptions, its progress states and receipts, and its speed claim. Its structure is sound: the design contracts A1–A7 name exactly what is missing (facts independent of output, one owner per fact, choices the search can combine). The work that remains is 013's child tasks, recorded in the plan.
