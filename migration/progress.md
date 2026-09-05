# Migration progress — the dense ledger

Parent: [index](index.md). Plan: [009](009-phases.md), re-sequenced by [012](012-second-look.md).
Narrative and evidence per step: [history](history.md); frozen phase 0–2 evidence: [status](status.md).

**This file is the authoritative state**, updated in the same commit as the work it describes.
One row per landed step. Numbers are Brotli-11 from `lilscript-codec` unless marked raw; ports are
pool-built from arm-isolated binaries; "cases" is `tests/cases` (73 files) plus `~/probelil`.

Updated 2026-09-05, branch `migration/target-tree`, 111 commits ahead of `main` (`54e1948`).

---

## State in one screen

| Phase | State | Gate |
|---|---|---|
| 0 instrument | **done** (0.4 narrowed to two ports + probe + cases, by the owner) | reverting a fix turns the probe/cases red; the pool runs the matrix in 11 s |
| 1 expression tree | **done** | 44/44 artifacts identical on three canaries; twin witness with negative control |
| 2 statements, block | **done** | escapes 26 → 0; block is a type; canaries identical |
| 3 tree authoritative | **done on the emitter** | `Raw` gone, text deleted, classifiers structural; G1/G2 deletion moved to 6 |
| 4 facts delivered | **done on the node** | `IrFacts` delivered; `NodeId` required; 8-bit fact word stamped; byte-identical |
| 5 naming post-layout | **mechanism done; deletions deferred** | binds, spelling table, respell, scope tree, sound renamer, census; idiom g1 as a per-port pin (−1,168 net on six ports); default byte-identical |
| 6 fold groups | **4 printer-side retirements landed** (6.1–6.4) | each: solo residue 0, Brotli same-or-better on ports, behaviour/twin/tests/pool green |
| 7 candidate derivation | **evidence only** | L11 vs L13; emission-count profile; scoring-order pathology named — **next**, see 012 |
| 8 retire the text layer | not started | — |

**The gate** (owner, 2026-09-04): behaviour never relaxes; Brotli and compile time must be same-or-better
when the migration is *finished*, not at every step. Byte identity is evidence, never a requirement.

**The two verification ports against the pre-migration compiler `54e1948`:**

| port | `54e1948` | now (`acace54`) | Δ |
|---|---:|---:|---:|
| markedlil `marked.esm.js` | 9,470 | **9,322** | −148 |
| zodlil `zod.core.js` | 32,489 | **32,415** | −74 |

Both are below the baseline they must end at or below, with every correctness fix's cost inside
(live-12 alone was +35 on zodlil). Compile time is not yet measured end-to-end against `54e1948` on
one worker; the current profile is under *Measurement facts*.

---

## The steps

### Phase 0 — repair the instrument

| commit | step | evidence |
|---|---|---|
| `fe51558` | `[profile.release-assert]`; `portgate.mjs`; random differential seed; fleet F1/F3/F4/F5; `fleet-compare.mjs`; live-3 (config discovery) and live-4 (`charCodeAt`) fixed | posthoglil `trust: ok`; markedlil portgate 406 s, suite green, 77.7% idle folds |
| `5fc2aac` | live-7: a two-use inlined value gets a name; parameters never skipped | release gate green again (`optional_constructor_callback` compiles) |
| `76d3c4f` | live-6: `.length` ToNumber elision closed at the family's *admission*, not the option default | the search had re-proposed the wrong spelling after the default flip |
| `152b830` | live-5: two folds that deleted a live binding refused (`remove_unused_standalone_vars`, `fold_single_use_literal_bindings`) | found by `LILSCRIPT_SKIP_FOLDS` bisection in minutes |
| `bf3a89f` | first arm-isolated pool A/B (`54e1948` vs `152b830`) | 12 artifacts: 4 identical, rest −39..+46 — correctness came free; compiler digests recorded |
| `6d0a741` | 0.3b: the differential's interpreter models classes, `super`, `this`, lexical capture | finds live-8 within minutes |
| `68801c4`, `d07a529` | live-8: a phi region that duplicates a side effect is never proposed (`may_have_effects` on the tree) | 0 Brotli over 5 ports; 10 wrong candidates per compile stop being built |
| `a5dee05`, `8cb7979` | `workers.mjs check`: the case matrix on the pool; configs travel with the tests; pool driven from a worktree | 144 case-lanes in 11 s; the first run's 72 false failures were config discovery (rule: discovery is never load-bearing in a harness) |
| `aaae9a4` | fold census on the fleet (`LILSCRIPT_FOLD_REPORT=all`) | 53 of 128 folds never fire on four ports; never-active folds are ~3% of CPU |
| `80fac19` | phase 7 evidence: the effort ladder | L11 beats the shipped L13 on cnlil/posthoglil/mobxlil in bytes *and* time; markedlil prefers 13 |
| `3fe40f3` | `~/probelil`: one feature-dense library, both lanes, golden output, 22-config matrix | 4.6 s inner loop; 21/22 agree (`level15` is live-9) |
| `5b289d6`, `53a4275` | the gate revised to behaviour + end-state; this ledger created | owner directive 2026-09-04 |

### Phase 1 — the expression tree (`56fc86d` … `4c10031`)

| commit | step | evidence |
|---|---|---|
| `56fc86d`, `0081beb` | the arity table, written down then enforced (`render(..).expect` per kind) | found `NullNormalized` built with no operands, the `Member`/`Index` tag collision, an operandless `>>>0` |
| `e7fa02f`, `b9ebd20`, `697c735`, `8ed77c3`, `0570fea` | one owner for children; `code` derived by `render` for every non-leaf kind; `JsRenderOptions` replaces 57 hand-threaded copies of one flag | 0 of 72 artifacts differ per step; 1,705 tests |
| `b31c5d6` | `LILSCRIPT_TWIN=1`: every node rebuilt from its children, compared to its own text; negative control | 72/72 both lanes; a corrupted arm fails 53 of 72 |
| `4c10031` | proved on cnlil | 8/8 artifacts identical; twin clean on a 26.8 KB artifact at +2% time |

### Phase 2 — statements, functions, the module (`404ec93` … `7adb6e0`)

| commit | step | evidence |
|---|---|---|
| `404ec93` | 2a: `type JsBlock = String`, 63 signatures | pure rename |
| `292803b`, `00206f1` | 2b: the block is a type with counters; the loop-keyword census stops rescanning the artifact | first counters were byte-identical and **23% slower**; bounded: cnlil 76.2 s → 76.4 s |
| `db4ede9`, `98666ab` | IDENTICAL on cnlil, posthoglil, markedlil | 44/44 artifacts; time +0.3% / +0.4% / +0.006% |
| `76859e6` | `take_trailing_expression_statements` measured | 5.1 ms of 31.2 s (0.016%) — retracted as a superlinearity in 009 |
| `afacdc1` … `9ac8c49`, `7adb6e0` | statements become nodes (bindings, `return`/`throw`, loop control, module clause, declaration groups, `if`/`else`, loop bodies as values, fusion runs held back, terminators as facts); escapes 26 → 0; escape methods deleted | each family found a disagreement the text hid (two `return` sites, two `export` emitters, a heuristic reading the wrong buffer) |

### Phase 3 — the tree becomes authoritative (`ec19c3e` … `ac50e29`)

| commit | step | evidence |
|---|---|---|
| `ec19c3e` … `9254eeb`, `7012629`, `5622552`, `d0667b5` | the block is a statement list with the text as its cache; statement bytes arriving as nodes 61% → 100% on the probe and all cases; the state machine, module `let` list, cluster IIFEs, class sites as nodes | ports identical at every batch except `2d7a01a` (zodlil +18: one concise arrow lost, recovered in `229b728`) |
| `523ca17` | `Declarators` node; live-10 (a name declared twice in a `let` run) fixed | two cases smaller (1238 → 600); zodlil +6 |
| `7e5975e`, `3bb2322` | classifiers get structural twins under the witness, then the call sites read the list | live-11 found (`1==x?..` accepted as an assignment to `1`); **live-12** found and fixed (`if` body of `[try{..}, stmt]` spelled braceless — a wrong program on `main`, zodlil +35 as the correct program's price) |
| `f6d515a`, `29b8ed7` | `Raw` gone; the block's text deleted; `JsBlock` renders from its list | probe wall 19.8 s → 16.7 s, emit CPU 104 s → 83 s |
| `9ed41cb`, `e5c40c3`, `78aaf49` | G1 emitter side: keyword separator; braceless `if`/`else` unless the `else` would be captured; presence test as a node (`UndefinedTest`) | raw residue 0 for all three; zodlil −72 (keyword spaces); braceless: zodlil +47 / markedlil −114 |
| `1516ad0`, `ac50e29` | `braceless_control_bodies` is a knob and a scored variant; the G1/G2 "unreachable" assumption corrected | a fold that fires on other folds' output is deleted with them (phase 6) |

### Phase 4 — deliver the facts (`f609a79` … `a24ee0f`)

| commit | step | evidence |
|---|---|---|
| `f609a79` | `IrFacts` (effect summaries, finite values, array-parameter lengths) delivered per emission | byte-identical; `facts_delivered` counted |
| `46df98b` | `NodeId` required on every instruction, module-wide allocator; 18 `None` sites derive | two subsumption tests caught the first attempt |
| `ed20326` | `JsOrigin` + `JsFacts` on every node; **`PairGraph`** replaces the BFS in `safe_two_address_phi_pairs` | probe wall 16.7 s → 5.8 s, emit CPU −80%; the "+17%" before it was a code-alignment lottery, proved with an uncalled function |
| `a24ee0f` | `PURE`, `NO_THROW`, `OWNED_SLOT`, `NON_NULLISH` from oracles the tree had (`ValueKind`) | byte-identical; 1,714 tests |

### Phase 5 — naming post-layout (`bac9678` … `8d255e1`)

| commit | step | evidence |
|---|---|---|
| `bac9678` | the name-request-order trace (`LILSCRIPT_NAME_TRACE=1`, `name-trace-diff.sh`) — the gate lands before any naming moves | 759 requests per emission on the probe; self-gate 0/12 |
| `cc88070`, `e640a1c`, `61df684` | `Bind(u32)` on references, declarations, heads, loop heads, catch, module names | 0 byte / 0 trace diffs each; census `decl_bound` 53,505 / unbound 31,277 |
| `6cdbe83`, `c491f98`, `578625f` | heads as pieces; closures as nodes with kept trees (`ClosureId` per rendering); spelling as a side table with the `Respell` walk, witnessed both ways | ports byte-identical each step |
| `abbceb9` | `ScopeTree`, the sound renamer, `NameOrdering::FrequencyDesc` off by default | a closure-tree key collision (wrong program) caught and fixed before landing |
| `491a856`, `a38f42c`, `30c30a1` | conditions keep their trees; three raw sites keep their node; function/global references carry binds | probe renameable bindings 100 → 7,549 of 14,545 |
| `d84992b`, `9b54a4c`, `66a36ab`, `e9d7e5d` | `LILSCRIPT_NAME_ORDERING`, `_SEARCH`, the `rename_kept_<kind>` census, the priority-slot family | **frequency-desc pinned on the fleet: markedlil −35, zodlil +55, cnlil +77, micromarklil +591, mobxlil +22 — off** |
| `e20d700` | the port twin lane (`port-twin.sh`); the `this`/`arguments` receiver bind leak fixed | remarklil, cnlil, micromarklil clean; live-13 recorded (pre-existing) |
| `e0436a3`, `8d255e1` | `IdiomConverged` on binding identity, the collision guard, one idiom group per candidate, the `name-ordering` Cartesian axis | **g1 pinned on six ports: remarklil −811, micromarklil −391, mobxlil −37, markedlil −5, cnlil +3, zodlil +73 — net −1,168; ships as a per-port pin, default byte-identical**; as a late candidate it wins nothing (names steer layout) |

### Phase 7′ — one emission, many prints (`6ee42eb` …)

| commit | step | evidence |
|---|---|---|
| `6ee42eb` 7.0 | the reading: Terser 5.44.0, Oxc 0.147.0, Closure master — each runs its last transform before the printer and scores nothing in between; `refs/competitor-techniques.md` §I | file:line for the pipeline order, the printer's own options, and `best_of` |
| 7.1 | `ModuleTree` (block, closure trees, bind table) kept by `emit_with_tree`; `reprint(options)` is a forced `Respell` under new printer options; `LILSCRIPT_PRINT_TWIN=1` compares re-print with re-emission per candidate field | **census, 148 emissions (74 files × shipped-off, none-off):** every field with an effect is emission-dependent — `function_spelling` 40, `string_quote` 38, `conditional_expressions` 33, `comma_expressions` 30, `mutation_spelling` 28, `update_loop_layout` 27, `compact_boolean_literals` 24, `loop_spelling` 10, `elide_new_parentheses` 7, `truthy_nullable_checks` 6, `elide_call_chain_parentheses` 5 of 11, `unused_catch_binding_elision` 4, `effect_ternary` 3, `compact_generator_star` 2, `braceless_control_bodies` 1; `elide_block_terminal_semicolons` has no effect any more (6.4). Byte-identical to `acace54`; 1,717 tests |

| 7.2 | booleans are a `Bool(bool)` leaf the printer spells (`JsRenderOptions.compact_boolean_literals`); stores and fused-run members are `Assign` nodes (`Member`/`Index`/`Name` targets) instead of raw text; `statement_expression_node` feeds the fusion run | byte-identical to `cd50de7` in three lanes (one swap-canonicalisation regression caught by the none lane and fixed: `is_constant_literal` keyed on the atom root); twin: `compact_boolean_literals` 24 effects → 18 print-ok / 6 emit-dep (the six: the inline-cost shape `let a=true` vs `!0` inlined, ternary arms and array elements still text), `elide_call_chain_parentheses` 11/11 print-ok; renamer `rename_kept_raw` on the probe 16 → 6; 1,717 tests |

| 7.3 | string literals are a `Str(Lit)` leaf over a per-emission `LiteralTable` (interned: equal contents, one id — an arm merge compares nodes), spelled by the printer from `JsRenderOptions.string_quote`; the table rides the `Mangler` so the naming context's inlined constants are leaves too | byte-identical to `a32eb07` in three lanes after two parity fixes the none lane caught (the constant-operand swap must not widen to template quotes; equal strings must compare equal); twin: `string_quote` 38 effects → 24 print-ok / 14 emit-dep (call arguments, array elements and ternary arms are still text); 1,717 tests |

| 7.4 | array literals are an `Array` node over element nodes (the packed `"a,b".split(",")` spelling stays an atom, chosen by length as before) | byte-identical to `bf6d05c` in three lanes; twin: `compact_boolean_literals` 19 of 24 print-ok, `string_quote` 25 of 38; 1,717 tests |

| 7.5 | `typeof x` is a `Unary(TypeOf)` node and `Symbol(..)` a `Call` node (two raw producers gone); `[emission-options]` trace and `emission-axes.py` count the option tuples the search emits | byte-identical to `f80e669`; **the emission axes:** probe 381 emissions / 130 tuples — `string_quote` varies on 168, `identifier_alphabet` 102, `stable_local_names` 165; markedlil 277 / 141 — `string_quote` 64, the naming-policy fields (`stable_local_names` 107, `local_name_reserve` 32, `precise_cross_scope_shadowing` 22, `frequency_order_local_names` 19) ≈ 196, structural (`constructor_initializer_fusion` 72, `iife_private_callee_clusters` 66, `function_layout` 25) the minority; 1,717 tests |

| 7.6 | the first search-side re-print: `FrozenModuleTree` (a `ModuleTree` with its tables out of their `Rc`s, so it crosses threads), `emit_javascript_candidate_frozen`, `reprint_javascript_candidate` (finished by the same six memoized folds an emission gets, timed as `reprint`), and under `LILSCRIPT_REPRINT_QUOTES=1` the emission contexts keep the first tree per (context, options with `string_quote` erased) and print a later plan that differs only in the quote from it | **probe:** 381 → 279 emissions, 102 re-prints at 2.3 ms against 48 ms per emission, wall 5.39 → 5.11 s, Brotli 1,514 → 1,517 (the residue sites keep the base quote; parity waits on them); cases +1..+3 on six files, behaviour 0 wrong, probe both lanes ok. **markedlil: 1 re-print of 277** — its 64 quote variants are paired with identifier-alphabet variants (the spelling family emits `(alphabet, quote)` pairs whose `Double` base is never emitted), so the cache misses; the axes that multiply markedlil's emissions are the naming ones, which need a name-table replay, not a re-print. Off by default; 1,717 tests |

| 7.7 | the conditional return keeps its node (`ConciseNode` / `Return { value }` instead of `into_minimal` text); class construction is a `New` node (identity classes) or a `Call` node with `value_atom`/`function_atom` (positional classes); record field stores are `Assign` nodes | byte-identical to `9dba734` in three lanes; twin: `string_quote` 30 of 37 print-ok, `compact_boolean_literals` 20 of 24; the residue: `default_value` text (`b=''`), `coalesce_absent_to_null`'s `??null` wrapper (kept raw: its text precedence is `Conditional`, the node's `LogicalOr`, and `a??null||b` must stay grouped), `Set.has` arguments, the `typeof` comparison; 1,717 tests |

| 7.8 | `[emission-context]` beside the options trace, so the axes are counted per IR context; ports and pool re-verified for 7.1–7.7 | **markedlil, per context:** 277 emissions over 13 IR contexts; 17 exact duplicates (same context, same options); erasing `string_quote` alone frees 18, the identifier alphabet 17, the nine naming-policy fields 91, naming + quote **99 of 277 (36%)**; the remaining 178 are structural bases. Ports byte-identical to 6.4 (markedlil 9,322, zodlil 32,415, same digests); pool 292/292 |

### Phase 6 — the fold groups (`ea045ca` … `acace54`)

| commit | step | evidence |
|---|---|---|
| `ea045ca` | `fold-census.sh`, `fold-residue.py`; the first two push-time retirements reverted (33 byte diffs; the `mutation_spelling` contract) | the finding: a mid-pass fold cannot be retired byte-identically while the search scores pre-peephole text |
| `b2a789e` 6.1 | adjacent declarations merge at the join (`push_statement_with`, across `push_block`); `let f=…` arrows absorbed as declarators | cases shipped 7,512 → 7,493, zodlike 7,453 → 7,439; solo residue 0 with the search off |
| `06edd9b` 6.2 | the module-level `;`→`,` is the printer's (`JsBlock::render`, `top_level`); `Comma` node with operands; `[emission]`/`[peephole] input` traces; `fold-origin.py`; `LILSCRIPT_ONLY_FOLDS`; **live-14** fixed | ports markedlil 9,340 → 9,323, zodlil 32,464 → 32,438 (281 module `;` → 77); cases +25 (tiny artifacts prefer `;`); solo residue 0 in every lane |
| `4edeb38` 6.3 | the `for(` head takes the plain assignments written before it (G8 first item) | ports byte-identical; none lane identical; the `var`-list shape reverted (it hides a single-use literal from a later fold: +5/+8) |
| `acace54` 6.4 | the `;` before `}` and at EOF is always the printer's (`close_branch`, `into_braced_body`) | solo residue 79/134/514 → 0/0/0; markedlil 9,323 → 9,322, zodlil 32,438 → 32,415 |

---

## Live wrong programs

Fourteen found, all reproduced. **Every one was found by an instrument this migration built**
(reading, the twin, the probe matrix, the differential at classes, fold bisection, the port twin
lane) — none by the fleet's own test gate.

| # | defect | found by | state |
|---|---|---|---|
| 1 | `extern` rewritten by source spelling (105 names) | reading | **open** — wants the declared host binding (003); an owner decision (012) |
| 2 | closure `this`/`arguments` rebound by arrow spelling | reading, 061 | **open** — port fixed; the compiler fix is a fleet-rule change (004 §2) |
| 3 | `lilscript.toml` ignored for a bare relative path | reading | fixed `fe51558` |
| 4 | `charCodeAt` out of range yields `NaN` under `size-first` | reading | fixed `fe51558` |
| 5 | `preset = "none"` deletes a live binding — two folds | `SKIP_FOLDS` bisection | fixed `152b830` |
| 6 | `JS.number(x["length"])` loses `ToNumber` under `size-first` | agent | fixed `76d3c4f` at the family's admission |
| 7 | `optional_constructor_callback` fails to compile | release gate | fixed `5fc2aac` |
| 8 | a phi region duplicates a side-effecting call (10 of 137 candidates wrong; the search picked one) | differential at classes | fixed `d07a529` |
| 9 | at level 15 a `\|\|`/`&&` chain loses one side-effect call | probe matrix | **open** — pre-existing; minimisation pending |
| 10 | a name declared twice in a `let` run joined the next declarator onto a bare assignment | Declarators node | fixed `523ca17` |
| 11 | `parse_single_assignment` accepted `1==x?..;` as an assignment to `1` | classifier twin | fixed `7e5975e` |
| 12 | an `if` body of `[try{..}, stmt]` spelled braceless, the statement outside the `if` — on `main` | classifier twin, zodlil diff | fixed `f6d515a`; `tests/cases/live12_braced_try_in_if.lil` |
| 13 | mobxlil fails standards-parser admission with the search off | port twin lane | pre-existing, on the owner's list |
| 14 | `fold_dead_pure_identifier_assigns` deletes a braceless loop body's carried write (an infinite loop) | probe matrix (`gzip` row) | fixed `06edd9b`; the sixth wrong-program fold |

Caught before landing, not numbered: closure trees keyed by IR function re-rendered one clone from
another's tree (5.3); the receiver parameter's bind leaked into `Name(bind,"this")` (5.4d).

---

## Instruments (what proves what)

| instrument | proves | how |
|---|---|---|
| `LILSCRIPT_TWIN=1` | every node reproduces from its children; the list equals the text at every boundary; the identity respell changes nothing; condition text equals its tree | env-gated, runs in release; negative control recorded |
| `~/probelil` + `scripts/configs.mjs` | one feature-dense program has one answer under 22 configurations | 4.6 s; found live-9, live-14; run compiled outputs under `timeout` |
| `tests/cases` in three lanes | behaviour and Brotli per step against the pinned previous binary | shipped / none / zodlike; `$SP/bin/<sha>/lilscript` |
| `workers.mjs check --lanes …` | the case matrix on the pool, four lanes | 292/292 in ~20 s |
| `lilscript-differential --random-seed` | generated programs with classes, closures, `this` | found live-8 |
| `LILSCRIPT_NAME_TRACE=1`, `name-trace-diff.sh` | naming did not move when it should not have | 0 diffs required on byte-neutral steps |
| `LILSCRIPT_FOLD_REPORT=all`, `fold-census.sh` | which folds still rewrite, per lane | deletion needs 0 everywhere |
| `LILSCRIPT_ONLY_FOLDS`, `fold-origin.py`, `fold-residue.py` | a fold's residue on the emitter's own text, split from text-derived candidates and from other folds' output | the phase 6 work list is the *emitted* column |
| `LILSCRIPT_SKIP_FOLDS` | bisects a wrong program to its fold | seven builds over 111 folds |
| `LILSCRIPT_PEEPHOLE_TRACE=1` (+ `RAYON_NUM_THREADS=1`), `LILSCRIPT_STATEMENT_TRACE=1` | per-run snapshots, every rendered candidate | parallel emissions interleave stderr — single-thread it |
| `port-twin.sh` | the witnesses on a real port with the search off and under an ordering | found the receiver bind leak |
| `LILSCRIPT_TIMING=1` | effort buckets, `emit_calls`, `lex_calls`, `facts_delivered`, `name_bound`, `rename_kept_<kind>` | counters, never wall clock |

---

## Measurement facts that steer the work

- **Where compile time is, on the current binary (markedlil, pool D16 worker, `acace54`):** wall
  29.9 s; **emit 66.7 s CPU over 269 emissions (248 ms each)**; peephole 11.6 s over 72 runs (idle
  folds 6.3 s / 9,624 calls, active 5.1 s / 2,507); codec 6.5 s over 492 calls; optimize 1.7 s;
  analyze 1.6 s; lex 1.1 s over 7,666. On posthoglil (pre-`PairGraph`) codec was 87.7 s and emit
  60.1 s over 1,246 emissions. **The emission count is the multiplier**; per-emission cost is
  naming, out-of-SSA and rendering, paid again for every spelling variant.
- **zodlil ships one emission and no peephole** (`emit_calls` 1, `peephole_calls` 0, level 8, its dev
  config): its artifact is pure emitter output, so emitter-side changes show there directly.
- **The text layer is worth 189 Brotli on markedlil** (all folds on 9,397, off 9,586); a
  semantically empty change moves Brotli by about −125..+30 per artifact; tiny artifacts prefer
  `;` over `,` by three bytes while ports prefer `,`.
- **The search scores pre-peephole text on the candidate stages and the folds normalise afterwards**:
  option-off spellings (`elide_block_terminal_semicolons`, `comma_expressions`, `mutation_spelling`)
  only ever existed for what the intermediate folds saw; a printer-side retirement is neutral exactly
  when the fold ran after everything that could have consumed its input (6.3's lesson).
- **The string-surgery candidates re-create shapes the emitter no longer writes**
  (`top_level_declaration_variants`, the function-leading respelling, pooling): the *derived* column
  of the origin census — `merge_adjacent_declarations` 146, `fold_prior_assign_into_for_init` 536,
  all derived. That is candidate derivation's residue, not the emitter's.
- **Effort is not monotone downward**: L11 is smaller and faster than the shipped L13 on three of
  four ports; the ladder wants re-deriving per port after the cost model changes.
- **Name convergence loses to frequency because frequency ordering is entropy coding** (056/059);
  one idiom group applied as the baseline of the whole search wins (−1,168 net), the same group as a
  late re-spell wins nothing, because names steer `CompressionSimilarity` layout and every
  text-scored decision downstream.
- **The identifier stream is where katexlil's gap lives (+2,113), and the port's transliteration
  is why** (047/050): function by function we already beat Terser there; the loss is collective.
- **Never-active folds are ~3% of CPU**; guarding them is safe and small. 53 of 128 never fire on
  four ports; 81 of 129 never fire on the corpus.

---

## What is open, in order

1. **Phase 7′ — one emission, many prints** ([012](012-second-look.md)): score final text only;
   spelling axes as prints, not emissions; the budget ladder re-derived. Compile time's lever.
2. **Phase 6 under that regime**: the remaining G1 printer folds (inner comma join, unit-counter
   spelling, keyword spaces on other folds' text), then G3/G7/G11, G4/G5 as the single-use collapse
   on the tree with the fact word (the measured byte lever, +280/+296 on micromark), G6, G8, the
   scored groups as `ShapeTransform`s, G12 last.
3. **live-9** (open, level 15), **live-1** and **live-2** (owner decisions), the G8 latch.
4. **Phase 5.5 deletions** once the tree's idiom axis matches the text pass on the fleet.
5. **End-state measurement**: both verification ports and compile time on one worker against
   `54e1948`; then the remaining clean ports (jquerylil, mobxlil, posthoglil, cnlil).
