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

| 7.9 | **the text layer, measured** (owner asked: does it still run?). Yes: 47,281 lines, 124 registered folds, 78 runs per markedlil compile (21 s of 136 s single-thread). Census with the 7.8 binary — probe + 73 cases in 3 lanes, plus markedlil: **52 folds active somewhere, 72 registered folds never fire** (boolean 18, classes 14, loops 11, declarations 10, control 7, syntax 6, copies 3, members 2, calls 2). On markedlil's single emission 29 folds rewrite, 33 activations in chain order; two of the four 6.x ports still fire there: the inner `;`→`,` join (320 sites: 6.2 did the module level only) and the `var`-list for-init (2, deliberately left). **Chain-head rule:** a fold at chain position *k* can be ported to the emitter byte-neutrally only when every fold before *k* is idle on that text — the earliest active folds are the neutral ports. Ported the two chain heads: `fold_assignment_guards` (`a=f();if(a)` → `if(a=f())`, `merge_assignment_guard` at push time, by binding) and `remove_unused_standalone_vars` (`prune_unreferenced_declarators` after `build_module`, by binding, the `ScopeTree` as the reference count); hoisted bare declarators and the if/else merge's declarator now carry their binds (`claim_remaining_declarations` returns declarators). One token still moved: the leading `let`/`var` is scored by the codec on the *pre-peephole* emission, and the changed emission flipped a tie the final text loses (+43 markedlil, search off) — **the 012 scoring pathology, reproduced** — so the terminal slot now re-decides the leading keyword on the final text (`terminal_leading_declaration_respell`, strictly non-regressing; `terminal_keyword_flipped`/`kept` counters). Prior art for G4/G5 read and recorded (refs §J) | markedlil search off: byte-identical to 7.8; `fold_assignment_guards` 1→0 active, `remove_unused_standalone_vars` 2→1 (the last one at chain position 21, on a var an earlier fold emptied). Cases: 73/73 identical in every lane (maps_sets −1 by the terminal keyword). Twin 146/146, tests 1719, probe 21/22, pool 292/292. **Ports with the search on: markedlil 9,322 → 9,344 (a different spelling family won: 2,437 token changes), zodlil 32,415 → 32,427 (dev config, no peephole: the emitter now writes the guard shape and drops 2 dead vars, raw −38, Brotli +12).** The ranking among candidates is done on pre-peephole text, so any emission-side port moves the winner even when each candidate's final text is unchanged: byte identity under the search needs 7a first, as 012 ordered |

| 7.10 | **7a built and measured.** `EmissionPeephole`: every emission passes the text peephole before anything scores it, codec-verified per emission (the folds are not monotone: the async single-use IIFE lost to the binding it replaced), under the finalist stage's own function-elision and startup-limit rules (jquerylil's `max_nesting` rejected every candidate before the guard existed). Knob `javascript.emission_peephole`, env `LILSCRIPT_EMISSION_PEEPHOLE=0|1` forwarded by the pool; the function-scope wrapper's "exported binding written from a nested scope" check made scope-aware (a local shadowing an export is not a write to it). Fleet A/B on the pool, 27 ports, knob on vs off | **markedlil, search on, local: 9,234 → 9,190; wall 136 → 178 s single-thread (peephole 78 → 322 runs); all 259 changed emissions won at the codec, 0 lost.** Cases: shipped 7517 → 7508, none 11389 → 11383, zodlike 7467 → 7475, behaviour 0 diffs, twin 146/146, tests 1719, probe 21/22, pool 292/292. **Fleet: 19 ports built in both arms; on 342,388 vs off 341,735 — on is +653 worse**: mobxlil +429, remark-gfm +173, unifiedlil +88, markedlil +42; wins posthog 9, remark-math 10, remark-breaks 13, mdast-util-to-hast 48; ten ports identical. The search made fewer scored measurements with the knob on (remark-gfm codec calls 1,722 → 2,711 of which ~1,100 are the per-emission verification, mobxlil's main compile 1,156 → 938): folded emissions collide, and exploration keys on distinct text (`seen_code`, the frontier dedup, the entropy sources), so the search explores less. **Default off** per the owner's rule; the knob is the byte-neutrality instrument for fold retirements until the search keys on plan identity (7e). jquerylil, motionlil, react-markdownlil, solidlil failed in both arms — see live-15 |
| 7.11 | third chain head: `x=x±1\|0;` and the member form are `x++`/`x--` at push time (`Update(JsUpdate)` node, `unit_counter_update`; `fold_unit_counter_updates`); the guarded-decrement rotation now carries its condition tree (the twin caught `a>0` beside `a--`) and fires only under an explicit prefix/postfix spelling, as before. Both push-time shapes gated on `IrJsOptions.text_peephole` (the compile runs the peephole): zodlil read +49 when the unit update reached its postfix variants unasked, and the option that owns the spelling (`mutation_spelling`) must keep its choice where no fold overrides it | markedlil search off byte-identical; the 8 node-shaped `x=x-1\|0` become `x--`, the 8 `x=x+1\|0` are `Raw` bindings from the phi-copy path (the residue). Cases: none identical, shipped 73/74 (probe +3), zodlike 72/74 (−3); behaviour 0; twin 146/146; tests 1719; pool 292/292. **Ports under the search: markedlil 9,344 → 9,355, zodlil 32,427 → 32,476** — the same perturbation as 7.9: with pre-peephole ranking every emitter-side shape moves the winner. Probe now 22/22: live-9's level-15 configuration takes a different candidate; the fold is unchanged, live-9 stays open |
| 7.12 | raw-site ranking on markedlil after 7.9–7.11 (`LILSCRIPT_RAW_SITES=1`, kept-name hits): the if/else merge zone (`emit_structured_path` 12977: 207 hits, 13226: 95, 12941: 53, 13309: 46), `render_intrinsic` (16837: 121, 16941: 37), the phi-edge copies (`emit_phi_edge_cached` 15250: 81 over 180 nodes), named callee clusters as text (9861: 48), `render_op` stores (15973: 48). Statement kinds pushed on markedlil's emission: 238 `Binding` with a `Raw` value, 144 `Expression Raw` | the queue for G10/G4: the merge zone first (401 hits over four sites), then the phi copies |

| 7.13 | **the if/else merge zone as nodes (G10, first three arms).** The merged single assignment (`statement_single_assignment_node`, `block_merge_conditional_assignment_nodes`: the value node and the target's bind), the merged value built as `nullish` / the condition tree / its negation / `binary(Or\|And)` / `conditional` nodes, `push_merge_declaration` and the deferred merge carrying nodes; the effect ternary and the guard compaction (`cond&&arm`, `!cond\|\|arm`) as nodes through `block_compact_arm_node`, offered only when the node spells the text the zone would have written, so the zone's remaining text rewrites keep their cases. A raw operand under `&&`/`\|\|` is grouped by the zone's old text scan rather than its declared precedence (`binary_operand`), until the raw sites are gone | markedlil search off: six redundant parentheses gone (`s=(s>=48&&s<=57)\|\|…` → `s=s>=48&&s<=57\|\|…`: the old scan read `>=` as an assignment), Brotli identical. Raw ranking on markedlil: the zone's 207 + 95 + 53 + 46 hits are gone; the head is now `render_intrinsic` (105), the phi copies (80 over 180 nodes), `render_op` stores (48), the callee clusters (46). Cases identical in every lane; twin 146/146; tests 1719; pool 292/292. **Ports: markedlil 9,355 → 9,310, zodlil 32,476 → 32,413** — both below 7.8 (9,322 / 32,415) for the first time since the chain-head ports |

| 7.14 | the intrinsic constructor `new C(a,b)` is a `New` node over its argument nodes; `HostFieldSet` is an `Assign` node over a `Member` target (the last raw store) | byte-identical everywhere: markedlil search off, cases in every lane, ports (same digests as 7.13); twin 146/146; tests 1719; pool 292/292. Raw ranking head on markedlil: the phi copies (72 hits over 180 nodes), the callee clusters (44), the `+""` coercion (37) |

| 7.15 | **the phi-edge copies as nodes (G4, the copy statements).** `PhiCopy { target, bind, source: JsExpression }`; the single copy, the declarator list, the ordered scalar copies and the tuple (`[a,b]=[x,y]`: an `Array` node over the sources, the pattern still text) keep their nodes and binds; the dependency ordering reads the sources' text as before; the swap through a temporary stays textual (it rewrites names inside sources). Loop increments now reach the push-time unit update, so `fold_index_postfix_updates`, which precedes the counter fold in the chain, sees `s++` first on one markedlil loop and `for(s=0;…;s++){…N[s]=U}` becomes `s=0;while(…){…N[s++]=U}` — **a port that makes a shape appear earlier can wake an idle earlier fold; the chain-head check must be re-run on the new text** | markedlil search off: Brotli 10011 → 10008 (that loop, plus the terminal keyword flipping `var Fd` to `let`); raw ranking: the phi site (72 hits over 180 nodes) gone; head is now the callee clusters (40), the `+""` coercion (37). Cases: shipped 7520 → 7509, none 11389 → 11395 (33_algorithms: the destructuring copy now takes 6.3's for-init absorption, 350 → 337 raw), zodlike 7464 → 7459; the other diffs are name-order shifts where the renamer now sees the copies' binds; behaviour 0 diffs; twin 146/146; tests 1719 (the copy fixtures are `PhiCopy`s); pool 292/292; probe 21/22 (live-9 visible again). **Ports: markedlil 9,310 → 9,297, zodlil 32,413 identical** |

| 7.16 | the last intrinsic text: `x===void 0` is the `UndefinedTest` node, `x==null` and `x+""` are `binary_in_order` nodes (no constant-first swap, no coercion rewrite), the `""` an interned literal that follows `string_quote` (the text always wrote `""`, even inside a single-quote candidate) | markedlil search off and every case lane byte-identical; twin 146/146; tests 1719; pool 292/292. **Raw ranking on markedlil: only the two callee-cluster bodies remain (31 + 20 kept-name hits); every other raw site is under 3.** Ports: markedlil 9,297 → 9,364 (a different winner: the consistent quote inside single-quote candidates moves the pre-peephole ranking; raw −221), zodlil 32,413 → 32,391 |

| 7.17 | string constants are interned `Str` nodes (the `Const` op), the type check `"number"==typeof x` a `binary_in_order` over a literal and a `typeof` unary (`Array.isArray` a call), the absent-to-null coalescing the `NullNormalized` node (grouped under `&&`/`\|\|` like `??`); the callee clusters' root bodies and the wrapping IIFE are closure trees behind `Closure` nodes (`render_closure_statement`), the wrapper's callee at the lowest precedence so the call spells its parentheses. The renamer's typeof round trip fixed (`typeof ` with its space) | **print twin on the cases: `string_quote` 35 same / 1 diff (was 33/5; the one left is a module-level array of string constants built as text), `elide_call_chain_parentheses` 11/11, `compact_boolean_literals` 18/4 (the 4 are literal pooling reading the spelled length)** — the two clean fields are 7b's first re-prints. Byte-identical on markedlil search off and every case lane at each step; twin 146/146; tests 1719; ports identical (markedlil 09d2b39c, zodlil 9475d724). **markedlil's raw ranking is empty above 3 hits: the emission is nodes.** Pool 290/292 caught the typeof round trip on `type_guards` in the renamer lanes |

| 7.18 | **live-15 fixed**: a raw `throw ..` expression statement becomes a `Throw` statement at the push hook, so no compaction reads it | jquerylil compiles and parses again (search off: 92,829 raw / 31,219 Brotli; the pool's search-on number follows in 7.19). Cases identical in every lane; twin 146/146; tests 1719; pool 292/292 on the typeof build |

| 7.19 | jquerylil on the pool with the live-15 fix, the search on | **82,318 raw / 28,382 Brotli, against the pre-migration baseline `54e1948` at 86,072 / 28,764: −382 (−1.3%)** — the first giant measured on the branch, and the first end-state-style number: same source, same config, the tree's emitter beats the text-era one. markedlil and zodlil identical to 7.18; pool 292/292 |

| 7.20 | **7b, first three axes as re-prints.** A plan that differs from an emitted one only in `string_quote`, `elide_call_chain_parentheses` or `compact_boolean_literals` re-prints that emission's frozen tree; both emission paths (`emit`, `emit_frozen`) share one cache keyed by the erased options per IR context. The boolean inline-or-bind decision costs the compact spelling whichever prints, so the tree is the same under both spellings. Knob `javascript.reprint_spellings` (default on), env `LILSCRIPT_REPRINT_SPELLINGS` | print twin on the cases: `string_quote` 37/1, `compact_boolean_literals` 23/1, `elide_call_chain_parentheses` 11/11 (the two residuals are one struct literal still built as text). **Honest yield on markedlil: 3 re-prints of 278 — within one IR context the search proposes almost no spelling-only siblings (0 quote/paren, 4 with the alphabet, 16 with the frequency order, 78 with all eight naming policies); 7.8's "18 quote-only" had lumped the 20 unlabelled seed emissions.** The 30% cut is 7d's, the naming policies as a renamer pass over the frozen tree. Cases: 15_strings and callable_defaults move by noise; none lane identical; behaviour 0; tests 1719; pool 292/292; probe 21/22; ports identical (09d2b39c / 9475d724) |

| 7.21 | **7d, the naming policies as a rename pass over the cached tree.** `FrozenModuleTree::rename_reprint`: the tree's own scopes, the renamer in emission order (`RenameOrder::Emission`, the bind table's order) under the plan's alphabet, then the print. The cache entry carries its base's naming key; a plan with the same key re-prints, another key re-names — for the three policies that decide only which spellings a binding may take: `identifier_alphabet`, `local_name_reserve`, `reserved_local_name_prefix`. The others proved structural on the cases: `frequency_order_local_names` orders the hoisted declarators, the shadowing flavours decide whether a value is bound or inlined (17_struct: `var point=[13,29]` vs the literal in the call), `stable_local_names` needs the source hints on the tree; without identifier mangling every policy is a plain re-print. Re-prints count as attempted emissions (the resource-count test wants the same count at any thread count). Knob `javascript.reprint_names`, env `LILSCRIPT_REPRINT_NAMES`, off until the fleet measures it | markedlil, search on: emissions 278 → 263, 16 rename re-prints in 0.25 s, Brotli identical (9,190 local; pool 09d2b39c, wall 23.7 → 23.0 s); zodlil identical. With all eight policies erased it was 219 emissions and −7, but the none lane moved by −618 (the pass renamed unmangled names) and then +19 (the structural policies), which is what fixed the field set. Cases with the knob on: none identical, shipped 7513 → 7511, zodlike 7463 → 7475 (the renamer's spellings, not the emitter's), behaviour 0; tests 1719; pool 292/292; probe 22/22 |

| 7f (interim) | **end-state measurement, first pass**, the pool's workers, same source and config, the pre-migration `54e1948` against `18df6cc` (7d, knob on) | markedlil 9,470 → **9,364** (−106); zodlil 32,489 → **32,391** (−98), wall 2.39 → 1.76 s; jquerylil 28,764 → **28,382** (−382, at `a36a6cd`). markedlil's four artifacts: walls 45.2/44.0/24.8/27.2 s → 23.0/22.2/32.6/31.0 s (the pool's contention moves these; a same-worker sequential pair is owed). Search off, single thread, this host: markedlil emit 1,427 → 257 ms and wall 2.02 → 0.78 s; jquerylil emit 621 → 541 ms, wall 6.5 → 4.6 s. jquerylil with the search on, the pool: 450 → 576 s wall with the peephole 1,102 → 1,467 CPU-s over 455 → 626 runs and emit 482 → 940 CPU-s over 901 → 914 emissions — **open**: the per-emission cost fell everywhere measured, so the search reaches heavier emission kinds or more finalists; measured locally next |

| 7.22 | the positional struct literal and the inlined class value are `Array` nodes over their field nodes (the named object forms stay text until an object-literal node exists); field defaults as literal nodes; `LILSCRIPT_ATOM_SITES=1` names the source line of every bracketed atom (`#[track_caller]` on `atom`) | **print twin on the cases: `string_quote` 38/38, `compact_boolean_literals` 24/24, `elide_call_chain_parentheses` 11/11 — 7b's three axes proved per option.** markedlil search off byte-identical; cases identical except interprocedural_finite_values in the none lane (+9: the literal's quote now follows the option); tests 1719; pool 292/292; ports identical |

| 7.23 | fleet A/B of `reprint_names` on the pool (`18df6cc`, 27 ports, 20 built in both arms) | **on 370,105 vs off 370,069: +36 over 20 ports — a wash.** remark-gfm −187 (on better), jquerylil +102, mobxlil +54, posthoglil +28, four small ports +1 to +14; twelve ports identical. Emissions with the knob on: markedlil 276 → 263, remark-gfm 544 → 501, posthoglil 1,212 → 1,083 (191 rename re-prints), micromark 59 → 53. The default stays off (the owner's rule: a flip needs a measured win); the compile-time gain is real and the size is noise, which says the three pure naming policies were never where the bytes were — the structural ones (frequency, shadowing, the source hints) are, and those need the tree to own the bind-or-inline and declarator-order decisions (7e) |

| 7.24 | **the push-time render is gone.** `push_statement_with` rendered every statement to count `for(`/`while(` (the `LoopSpelling::Auto` heuristic) and to read its `;`, and `pop_statement` rendered again; a function body was therefore rendered once per enclosing level (the gdb sampler on the no-cluster variant: the innermost emitter frames were `EmittedStatement`/`JsStatement`/`JsExpression` renders). Both are read off the shape now: `loop_keywords_in_statement` (loop heads and the leaves the tree does not own — raw nodes, closures' code, atoms — where the old count found them too) and `statement_is_terminated`; the block's `tail` needle buffer is gone. `[emission-ms]` per-emission trace under `LILSCRIPT_EMISSION_OPTIONS`; `pmp2.sh` samples a markedlil compile | markedlil search off: emit 403 → 155 ms (baseline `54e1948`: 2,446), the no-cluster variant 1,007–1,632 → 305 ms (baseline 2,340); **search on, single thread: wall 169 → 109 s, emit 130 → 66 s, output identical**. Cases identical in every lane; tests 1719. jquerylil on this host, 4 threads, before this fix: baseline 1,361 s wall / 28,649 vs `18df6cc` 1,605 s / 28,332 — emit 726 → 975 CPU-s over 901 → 815 emissions was the whole regression; measured again with this fix in 7.25 |

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
| live-15 | jquerylil: "startup limits rejected every JavaScript candidate" — every candidate failed admission because the emission does not parse: `Za==bh.promise()&&throw new TypeError(..)`. A throwing host alias (`throwTypeError`, `JsHostAliasConvention::ThrowConstruct`) expands to `throw ..` text as an *expression*, and since `3bb2322` (migration 3, the structural classifiers) the guard compaction reads that expression statement as compactable. Bisected on the pool with `git bisect run` (99 commits, 7 steps): last good `7e5975e`, first bad `3bb2322`; the pre-migration baseline `54e1948` builds jquerylil at 28,764 Brotli | 7.10 fleet A/B | **fixed 7.18**: the push hook turns a raw `throw ..` expression statement into a `Throw` statement, which no compaction reads |
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
