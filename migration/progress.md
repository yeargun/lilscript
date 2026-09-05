# Migration progress

Parent: [index](index.md). Plan: [009](009-phases.md). Narrative and evidence: [status](status.md).

**This file is the authoritative state.** `status.md` is where findings are written up at length; it
is long, and parts of it went stale while the work moved. This is the short table you open to answer
"where are we", and **it is updated in the same commit as the work it describes**.

Updated 2026-09-04, branch `migration/target-tree`, 37 commits ahead of `main`.

---

## Where we are in one line

**Phase 0 is complete** (the baseline frozen on the two verification ports); **phase 1 is complete and
gated**; **phase 2b is
complete** — `out` is append-only, every edit of already-emitted text is gone and the methods that
did it are deleted — with 22 statement kinds on nodes and 246 fragment appends still to move; phases
3–8 are not started, and phase 7 has its measurement.

**The gate is behaviour and end-state compression, not byte-identity** — revised by the owner
2026-09-04, see [D3](001-directives.md#d3) and [009](009-phases.md#the-gate-vocabulary). A step may
move bytes; it may not break a program. Only the *finished* migration has to be same-or-better on
Brotli and on compile time.

| Phase | State | Gate met |
|---|---|---|
| 0 — repair the instrument | **7 of 7 items** (0.4 narrowed) | — |
| 1 — the tree exists, proved against the incumbent | **complete** | BEHAVIOUR + NEUTRAL + witness (byte-identical, as it happens) |
| 2 — statements, functions, module | **2a, 2b complete; statement tree at 22 kinds** | BEHAVIOUR + NEUTRAL |
| 3 — the tree becomes authoritative | **complete on the emitter** — `JsBlock` is a statement list rendered on demand; no `Raw`, no text-appending API, no text classifier; the raw emission has zero residue for the G1 folds measured (keyword spaces, negated comparisons, if/else braces — the last now a knob). G1/G2 deletion moves to phase 6 with the folds that feed them (corrected in 009); `repair_fused_keyword_identifiers` and `keyword_space_tests.rs` police peephole splices, so they go with phase 8 | BEHAVIOUR + NEUTRAL |
| 4 — deliver the facts | **4a landed** — `IrFacts` (effect summaries, finite values, array-parameter lengths) delivered to every emission beside the integer analysis; the escape lattice already rides on the IR; no consumer yet. Next: provenance made required (`NodeId` on every instruction, the 18 `node_id: None` sites derive theirs) | BEHAVIOUR + NEUTRAL (byte-identical) |
| 5 — naming moves post-layout | not started | — |
| 6 — the fold groups | not started (census taken) | — |
| 7 — candidate derivation and budgets | not started (**premise measured**) | — |
| 8 — retire the text layer | not started | — |

---

## Phase 0 — repair the instrument

| Item | State | Evidence |
|---|---|---|
| 0.1 assertions are real | **landed** | `[profile.release]` + `[profile.release-assert]` — `fe51558` |
| 0.2 ports are a gate | **landed** | `finer/tools/portgate.mjs`; posthoglil `trust: ok` — `fe51558` |
| 0.3 differential is generative | **landed** | `--random-seed`, seed printed before work — `fe51558` |
| 0.3b domain reaches classes/closures | **landed** | reference interpreter models instances, `super`, `this`, lexical capture — `6d0a741` |
| 0.4 baseline frozen | **landed, narrowed** | the pre-migration compiler `54e1948` on the two verification ports, table below; the 61-config version is not going to be run (owner: do not build the fleet) |
| 0.5 live wrong programs | **6 of 9 fixed** | table below |
| F1–F5 fleet gaps | **landed** (F2 partial) | `fleet-tests.mjs` still to promote |

### The frozen baseline (0.4)

The number the finished migration is measured against. Pre-migration compiler `54e1948` -- the
commit this branch grew from -- against HEAD, both arms built on the pool from the same sources,
scored with the pinned codec, provenance recorded per arm:

| port | `54e1948` | HEAD | Δ Brotli |
|---|---:|---:|---:|
| markedlil `marked.esm.js` | 9,470 | 9,431 | **−39** |
| zodlil `zod.core.js` | 32,489 | 32,603 | **+114** |

Net **+75** over the two verification ports, mid-migration. zodlil's +114 is **attributed**: every
landmark commit from `0081beb` onward already measures 32,603, so the whole delta sits between
`54e1948` and `0081beb` — the phase 0 correctness fixes (the `.length` ToNumber elision that was two
programs, the two folds that deleted live bindings). Those bytes were what the wrong programs were
"saving". The end condition is: both of these at or below the `54e1948` column when the migration is
finished — which for zodlil means winning back 114 bytes honestly.

### Live wrong programs

| # | Defect | State |
|---|---|---|
| 1 | `extern` rewritten by source spelling (105 names) | **open** — wants the declared host binding ([003](003-target-representation.md)); a hard error breaks ports that rely on the table |
| 2 | closure `this`/`arguments` rebound by arrow spelling | **open** — port fix shipped; the compiler fix is a fleet-rule change, and [004 §2](004-legality-by-construction.md) records why the obvious fix is wrong |
| 3 | `lilscript.toml` ignored for a bare relative filename | fixed — `config_search_parent` |
| 4 | `charCodeAt` out of range yields `NaN` not `0` | fixed — no longer elidable |
| 5 | `preset = "none"` deletes a live binding | fixed — **two** fold miscompiles, `152b830` |
| 6 | `JS.number(x["length"])` loses its `ToNumber` | fixed — at the family's admission predicate |
| 7 | `optional_constructor_callback` fails to compile | fixed — `5fc2aac` |
| 8 | local-phi region duplicates a side effect | fixed — `d07a529`, found by 0.3b |
| 9 | at `optimization_level = 15`, a `\|\|`/`&&` chain loses one side-effect call | **open** — found by the probe's config matrix; pre-existing; needs its surrounding program, minimisation in progress |

Items 1 and 2 are language/fleet-rule decisions, not bug fixes, and neither blocks a later phase.
Item 9 is a real miscompile in the search and is the next thing to fix.

---

## Phase 1 — complete

| Item | State | Evidence |
|---|---|---|
| every kind retains its grammar operands | **landed** | arity table + pin test — `0570fea`, `0081beb` |
| one owner for the child list | **landed** | three `Option<Box<Self>>` fields became projections — `e7fa02f` |
| `code` derived by the printer, not authored | **landed** | `render` owns every kind but `Atom`/`Raw` — `b9ebd20`, `e7fa02f`, `697c735` |
| render options are a printer parameter | **landed** | `JsRenderOptions`; 57 hand-threaded copies removed — `697c735` |
| twin witness + negative control | **landed** | `LILSCRIPT_TWIN=1`; corruption caught 53 of 72 — `b31c5d6` |

**Gate:** BEHAVIOUR + NEUTRAL, met with room to spare — 44 of 44 artifacts came out byte-identical
across cnlil, posthoglil and markedlil, all three clean under `LILSCRIPT_TWIN=1`, compile time
+0.3% / +0.4% / +0.006%. Identity is reported because it was achieved, not because it was required.

Three latent defects fell out of doing it: `NullNormalized`, the `Member`/`Index` tag collision, and
an operandless `>>>0` `Binary` node — each invisible while `code` was authoritative.

---

## Phase 2 — half done

| Item | State | Evidence |
|---|---|---|
| 2a `JsBlock` alias, 63 signatures | **landed** | pure rename — `404ec93` |
| 2b real type, escapes named | **landed** | no `DerefMut`; `truncate`/`pop`/`remove`/`insert_str`/`replace_range` are named methods — `292803b` |
| **2b escapes removed** | **landed — 26 → 0** | every edit of already-emitted text is gone; bodies and headers are values, runs are pending lists, terminators are flags |
| **2b escape methods deleted** | **landed** | `remove`, `insert_str`, `replace_range` are gone from `JsBlock`; `truncate`/`pop` remain only behind the two flag readers |
| 2b loop-keyword census as counters | **landed** | was a full rescan twice per loop — `292803b`, bounded in `00206f1` |
| **statements and module as a tree** | **started, 10 kinds** | bindings, `return`/`throw`, loop control, `import`/`export`; the emitter still writes the rest as text |
| block termination is a fact, not a text read | **landed** | 8 `ends_with(';')` sites → a maintained flag, witnessed — `afacdc1` |

`00206f1` is worth reading before the next perf change: the first counter implementation was
byte-identical and **23% slower**, and only a stopwatch could have caught it.

### The number phase 2 is driving down

Phase 1 worked because the gap was written down as a table and closed one row at a time. Phase 2's
equivalent is the count of places that assemble a statement out of text fragments instead of building
a node. It is static and `grep`-able, so it cannot drift:

    grep -c 'out\.push_str('  src/codegen_ir_js.rs     # fragment appends
    grep -c 'out\.push_statement(' src/codegen_ir_js.rs # statement nodes
    grep -c 'out\.\(truncate\|pop\|remove\|insert_str\|replace_range\)(' src/codegen_ir_js.rs

| | at 2b | now |
|---|---:|---:|
| fragment appends (`push_str`) | 324 | **0 — `JsBlock` has no text-appending API** |
| statement nodes (`push_statement`) | 0 | **70** |
| escapes into emitted text | 26 | **0** |

`JsStatement` has ten kinds — `Declaration`, `DeclarationGroup`, `Binding`, `Return`, `Throw`,
`Break`, `Continue`, `Import`, `Export`, `If` — and each holds its **children**, not their text.
`JsStatementOptions` mirrors `JsRenderOptions`: a printer setting the search varies belongs to the
printer, never to the node. Phase 3 can delete `code` when
the fragment column reaches the handful that genuinely emit sub-statement syntax.

Each family moved so far has found a disagreement the text was hiding: two `return` sites differed on
stripping an outer grouping, and two `export` emitters differed on the trailing `;`. Neither was a
bug, and neither would have been found by reading — they only show up when one node has to render
both.

---

## Phases 3–8 — not started

| Phase | Blocking on | What is already known |
|---|---|---|
| 3 tree authoritative, delete `code` | **started**: `JsBlock` is a statement list with the text as its cache, witnessed at every block boundary; **100% of statement bytes arrive as nodes on the probe and on all 72 cases in the shipped, none and forced-search lanes** (`stmt_raw_sum` 0 everywhere). The module's up-front `let` list is one `Declarators` node built by its five helpers instead of a `started` flag threaded through text pushes; the cluster IIFEs are `Binding`/`Expression` nodes over nested blocks; tuple copies, mutation spellings and `throw Error()` are nodes. What remains is static: sites in paths no case reaches (exports/imports of multi-chunk output, identity classes, the entry state machine), converted by reading in the next batch | 22 folds (G1, G2) become unreachable |
| 4 deliver the facts | 3 | annotations, `NodeId` provenance |
| 5 naming post-layout | 2–4 | the largest single lever (katexlil identifier stream, +2,113) |
| 6 fold groups | 3 | **census taken**: 53 of 128 folds never fire; worth ~3% of CPU |
| 7 candidate derivation and budgets | — | **premise measured**: L11 beats the shipped L13 on 3 of 4 ports, on both bytes and time |
| 8 retire the text layer | 3–7 | 47,118 lines |

---

## Measurement facts that should steer the remaining work

Taken this session, on the pool, and they contradict two things the plan assumed:

- **Cost model.** One posthoglil build, CPU across threads against 31.2 s wall: `codec` **87.7 s**,
  `emit` **60.1 s** over 1,246 emissions, `peephole` 8.3 s, `analyze` 8.3 s, `lex` 3.1 s,
  `optimize` 1.3 s. The candidate search's own scoring dominates the text layer by an order of
  magnitude — so phase 7 is where compile time is, and phases 3 and 6 are correctness work whose
  speed benefit is second-order.
- **`take_trailing_expression_statements` is not a superlinearity worth fixing.** [009](009-phases.md)
  called it one; measured, it is 5.1 ms of 31.2 s — **0.016%**. Retracted there.
- **Effort is not monotone downward either.** Level 11 is smaller *and* faster than the shipped level
  13 on cnlil, posthoglil and mobxlil; markedlil prefers 13. mobxlil pays 28% of its compile time for
  64 bytes it does not get.

---

## The loop

Owner directive: iterate on **one small library**, and keep the big ports for occasional background
verification that never blocks editing.

    # inner loop -- 4.6 s, both optimization lanes, diffed against a golden
    cd ~/probelil && LILSCRIPT_COMPILER=../lilscript/target/release/lilscript \
        node scripts/build.mjs --compile

    # cheap gates
    cargo test --release --lib                  # 1,706 tests
    node finer/tools/workers.mjs check          # 144 case-lanes on the pool, ~11 s

    # occasional, in the background, never blocking
    node finer/tools/workers.mjs build --ports markedlil,zodlil --compiler <path> --dist-dir <dir>

`~/probelil` is not a port of anything. It is a small library whose source deliberately exercises
every language feature, built at `preset = "none"` *and* at the shipped config and diffed against
`expected.out`, so a wrong program at either level fails in seconds instead of in a 130-second port
build. cnlil, the smallest real port, takes 76 s here; the probe takes **4.6**.

Writing it found six things the language does not have — no hex literals, no `do`/`while`, no ternary
`?:` (the compiler *emits* them; you cannot write one), `error` is reserved, catch bindings are typed
and must be `auto` or `JsValue`, struct literals are positional while record literals are named — and
one trap worth more than all of them: copied from cnlil, its config carried `strip_console = true`, so
every `print` was stripped and the artifact ran clean while asserting **nothing**. That is the third
convincingly-empty pass this session has produced from an inherited or discovered config.

    # and the sharper one: 18 configurations, one answer, 43 s
    cd ~/probelil && node scripts/configs.mjs

`scripts/configs.mjs` builds the probe under 18 configurations — levels 0/5/9/15, `candidate_search`
off/always, three cost models, three priorities, the peephole disabled, both function spellings, beam
1 and 32, and the phi-region option that carried live-8 — and demands they all print the same 32
lines. A feature-dense program has exactly one correct output, so **any configuration that disagrees
has found a wrong program**, which is precisely how live-8 announced itself. The artifacts range 2,522 B
to 3,309 B, so those configurations really are emitting different programs.

The probe also carries a regression block for every defect that has shipped: live-8's side-effecting
selection (asserting *two* probe calls, the only possible answer), `this` escaping its object through
an arrow, bug 7's generic-with-a-func-typed-parameter, the two folds that deleted live bindings, and
`charCodeAt` out of range where `NaN|0` must be 0.

Do **not** build the 27-port fleet.

### Byte moves under the Declarators node (2026-09-05)

**live-10 (fixed here):** a name declared twice in one `let` run joined the next declarator onto a bare assignment, declaring nothing; on the list the group closes and a later `let` opens a new one. Never observed firing.

Two of the 72 cases changed bytes, shipped lane, both smaller: `closure_factory_variant`
1238 → 600 and `optional_constructor_callback` 267 → 259. Cause, verified with the search off
and the peephole off: the text classifier `is_single_binding_statement` refused to group any
`let x=function(){…}` whose body contained a `;`, so twelve closure bindings went out as
twelve statements (1406 B); the structural classifier groups them into one declarator list
(1362 B). The leading keyword then flips `let`→`var` through `top_level_declaration_variants`
(compiler.rs), which scores both spellings of the leading top-level declaration on purpose —
now for the whole group at once. With the search on, the smaller base lets a different
candidate win. Behaviour: probe both lanes, 17/18 configs (level15 = live-9), twin 0/144,
1706 unit tests, pool 144/144.

**Port verification of `b6fdb96` (Expression + Loop nodes), pool, background:** markedlil
`marked.esm.js` 9,431 and zodlil `zod.core.js` 32,603 Brotli-11 — identical to the phase-0
HEAD row above. Two node kinds, zero bytes moved on both ports.

**Port verification of `523ca17` (Declarators), pool, background:** markedlil `marked.esm.js`
9,431 (identical bytes); zodlil `zod.core.js` 32,609 (+6 Brotli-11, raw −140). The let-grouping
of function-valued bindings reaches zodlil; +6 is inside the rename noise floor but it is real
and on the ledger: the end state has to win it back with the rest.

**Port verification of `6b8062f` (for-in/for-of, parallel copies, closure wrapper), pool,
background:** markedlil 9,431 and zodlil 32,609 — identical bytes to `523ca17` on both.

**Batch 5 (stores as nodes):** one case moves one byte, `33_algorithms` in the
no-optimization lane, 353 → 354. With the search off both binaries emit the same 374 B, so the
emission is unchanged; with it on, five more candidates are scored and a 353/354 tie flipped.
Scoring noise in a lane the `none` preset still searches; the shipped lane is byte-identical
across all 72 cases.

**Port verification of `dae03f2` (stores as nodes), pool, background:** markedlil 9,431 and
zodlil 32,609 — identical bytes to `6b8062f` on both.

**Port verification of `2d7a01a` (the `Function` node), pool, background:** markedlil 9,431
identical; **zodlil 32,627 (+18 Brotli-11, raw +9)**. Cause found: `push_return_conditional`
still pushed `return c?a:b;` as text, so the structural concise-arrow check (`[Return v]`) no
longer saw what the `{return X;}` text check had matched — one arrow in zodlil lost its concise
spelling (`{return }` is the 9 raw bytes). Not reproducible on the 72 cases in any lane
(shipped, none, arrow, level-15 production). Fixed in the next commit by making that return a
node; the zodlil number is expected back at 32,609.

**Why the tree matters for the codec objectives (owner's question, 2026-09-05):** every variant
the search scores today is a text rewrite (`top_level_declaration_variants` flips `let`/`var`
with `strip_prefix`; `compact_return_expression` is a list of `starts_with` guards; the let
grouping refused any value containing a `;` — worth 638 B on one case once decided on the
node). On values, variants can be structural (grouping, statement shapes, reordering of
effect-free runs, `if`/`?:`/`&&` forms), scoring can re-render subtrees instead of the whole
artifact, and names become values — where the katexlil gap lives. Headroom for the later
phases, measured against the ±100 noise floor like everything else.

**Port verification of `19bc243` (global store, closure paths), pool, background:** markedlil
9,431 and zodlil 32,627 — identical bytes to `2d7a01a` (the +18 stands until the fix below
lands on the pool).

**Port verification of `229b728` (Try node, conditional return as a node), pool, background:**
markedlil 9,431 identical; **zodlil 32,609 — the +18 from `2d7a01a` is recovered**, as
predicted by the `{return }` diagnosis.

**Port verification of `f3e9165` (merge scratch), pool, background:** markedlil 9,431 and
zodlil 32,609 — identical bytes to `229b728`.

**Batch 10 (state machine on nodes) — how it was verified.** No case and no probe config
reaches the state machine; it is a scored variant the registry proposes only when
`structural-control-flow-variants` / `switch-lowering-variants` are enabled. With those forced
(`migration/tools/candidate-diff.sh`), both binaries emit 152 state-machine candidates over
four loop cases and the candidate sets are identical except one losing variant: under the
search's `elide_block_terminal_semicolons=false` option the text path always wrote
`if(c){s=1}else{s=2}` and the `If` node spells it per the option (`{s=1;}`). The elided twin
wins every time; outputs are byte-identical in all four lanes. Two facts for later: the
branch condition there carries a redundant pair of parentheses (`if((c<=1))`) in both old and
new — a byte to win once the state machine is worth scoring — and the state machine only ever
renders as a candidate in these ports.

**Port verification of `9254eeb` (state machine on nodes), pool, background:** markedlil 9,431
and zodlil 32,609 — identical bytes to `f3e9165`.

**Batch 11 (the dynamic remainder).** Aggregate raw-site census over the 72 cases in three
lanes: 12,198 + 63,508 + 19,562 raw bytes before → 0 + 0 + 0 after. Candidate sets identical
(1,274 each, 152 state-machine renders) under `candidate-diff.sh`. One compile in the
forced-search lane fails on both binaries (`type_guards`: the startup-cost guard rejects every
candidate under that synthetic config) — not a regression, a limit of the lane.

**Port verification of `7012629` (module let list, cluster IIFEs, dynamic remainder), pool,
background:** markedlil 9,431 and zodlil 32,609 — identical bytes to `9254eeb`.

**Batch 12 (the static remainder, by reading):** the export list and the chunk imports use the
existing `Export`/`Import` nodes (an empty import list stays exact as `Raw`, since the node's
empty shape is the side-effect import); identity classes are `JsStatement::Class { head,
members }` with `ClassField` members and `Function` methods; the object-literal scratches are
`String`s (`push_object_literal_key` takes one); the entry state machine is an `Expression`
over a nested body; the `;` the module needs before an export list is an explicit `Empty`
statement. Zero byte diffs in three lanes, candidate sets identical, pool 144/144.

**Port verification of `5622552` (module-level and class nodes), pool, background:** markedlil
9,431 and zodlil 32,609 — identical bytes to `7012629`.

**Batch 13 — the emitter side of phase 3 is done.** The last block-typed pushes (a second entry
state machine, `var name;`, `s??(e);`, `t=t||v;`, `t=m?i:t;`, the structured fusion batch) are
nodes; the `BracedFunction` frame and `close_statement_block` are deleted because no path wants a
dispatcher to own braces any more. Static census: 0 `push_str` into a block. What still turns
text into blocks: `compact_top_level_expression_statements` (a byte-scanning fold over a loop
body, three call sites), `rotate_guarded_decrement`'s `JsBlock::from(rest)`, `truncate` (list
mirroring), and three `Raw` constructions (the local-update fallback, the empty import, the merge
tail fallback). What still reads block text to decide: ~25 `Deref<str>` uses and the classifier
family (`is_braceless_statement`, `is_comma_eligible_statement`, `compact_branch_expression`,
`compact_ternary_arm`, `compact_return_expression`, `compact_sequence_expression`,
`merge_conditional_assignments`, `conditional_assignment_expression`, `negated_self_or_assign`,
`optional_method_reassign`, `concise_arrow_body` is already structural). Those are phase 3's
remaining work, and they are where the G1/G2 folds live.

**Port verification of `d0667b5` (emitter side done), pool, background:** markedlil 9,431 and
zodlil 32,609 — identical bytes to `5622552`.

**Batch 14 — the classifiers read the list.** Every text classifier that decides a shape from
emitted bytes (`is_braceless_statement`, `is_comma_eligible_statement`, the `compact_*` family,
`merge_conditional_assignments`, `conditional_assignment_expression`, `negated_self_or_assign`,
`optional_method_reassign`, `parse_assignment_guard_return` and their helpers) got a structural
twin over `JsBlock.statements`, and every call site computed both under `LILSCRIPT_TWIN=1` with a
panic on disagreement — 288 case-lanes plus the probe in four lanes. The witness found three
things: (1) the text sequence fold refuses a run of one, and a statement whose terminator was
elided — matched; (2) the non-declared merged assignment I had emitted as an `Expression` in
batch 9 is an assignment, and the twin wanted the `Binding` — fixed at the producer;
(3) **live-11**: `parse_single_assignment` accepted `1==v58%3?..;` as an assignment to `1`
(a digit is an identifier byte), and `conditional_assignment_expression` then spelled the else
arm as `1` `=` `=v58%3?..` — right by accident. The structural twin refuses it; the text side
now requires an identifier start. Found by the twin on `function_subsumption`.

**Port verification of `7e5975e` (classifier twins), pool, background:** markedlil 9,431 and
zodlil 32,609 — identical bytes to `d0667b5`.

**Batch 15 — the call sites read the list.** Every classifier call site uses the structural
version; `twin_check` is gone. Zero byte diffs in three lanes, candidate sets identical
(2,011 each over six cases), probe both lanes, 17/18, 1706 tests, pool 144/144. The text
classifiers survive only as the `Raw` fallback inside their twins, which is the next thing to go.

| live-11 | `parse_single_assignment` accepted `1==x?..;` as an assignment to `1` (a digit is an identifier byte); the else arm survived by re-spelling `1` `=` `=x..` | **fixed** on the text side and impossible on the list; found by the classifier twin |
| live-12 | `is_braceless_statement` called an `if` body of `[try{..}catch{..}, statement]` braceless — no top-level `;` after the braces — so the statement was emitted **outside the `if`**. The shipped compiler prints 21 for `tests/cases/live12_braced_try_in_if.lil`; the correct value is 14. Reached from zod's own source | **fixed** (`statement_is_braceless` on the list); zodlil +35 Brotli is the correct program's price |

**Port verification of `3bb2322` (call sites on the list), pool, background:** markedlil 9,431
identical; **zodlil 32,644 (+35 Brotli-11, raw +1)** — one site: the shipped text classifier
spelled an `if` body of `[try{..}catch{..}, nk=g.value]` braceless and put `nk=g.value` outside
the `if` — **live-12**, a wrong program in every compiler on `main`. The list version keeps the
braces. `tests/cases/live12_braced_try_in_if.lil` and probe line 42 reproduce it (21 on the
text-era compiler, 14 correct).

**Batches 16–18 — `Raw` is gone.** The local-update helper returns an expression; the empty
import branch was unreachable; the merge tail is declarators; the loop-body sequence fold and
the guarded-decrement rotation rebuild their block from the list (`replace_with`,
`retain_from`); `take_trailing_expression_statements` pops trailing expression statements off
the list instead of scanning bytes backwards; `JsStatement::Raw`, `JsBlock::push_str`/`push`/
`Write`/`From` and every text classifier with its tests are deleted; the classifiers have
structural tests. Twin sweep 0 failures over the probe and the cases in four lanes; byte diffs
vs `3bb2322`: none; candidate sets identical; 1710 unit tests; pool 146/146 with the new case.

**Port verification of `f6d515a` (`Raw` gone, live-12), pool, background:** markedlil 9,431
and zodlil 32,644 — identical bytes to `3bb2322`.

**Batch 19 — the text is deleted.** `JsBlock` is `{statements, counters, inherited, tail,
flags}`; `render()` concatenates the statements when a boundary asks; `into_string` is
`render`; `is_empty` is the list's; the loop-keyword counters take a child's delta on
`push_block` and keep a five-byte tail for a needle completed across a join;
`drop_trailing_semicolon` marks the last node, `drop_trailing_bare_return` and the trailing
fold pop it; `truncate`, `pop`, `edited`, the `Deref<str>`/`Display`/`From` impls and the witness
are gone. The four readers that wanted text — the layout scorer's n-gram profile and lengths,
the fusion loop's comma run, the loop's `for_update_clause` — render on demand or read the list.
One real bug caught by `string_code_units`: the tail kept characters where the join buffer holds
bytes. Zero byte diffs vs `f6d515a` across 74 cases in three lanes; candidate sets identical
(3,064 each); probe both lanes; 1710 tests; pool 146/146. **Compile time on the probe: 19.8 s →
16.7 s wall, emit CPU 104 s → 83 s** — the per-push text copy and the witness were that much.

**Fold evidence for the G1/G2 deletion protocol (2026-09-05, on `29b8ed7`).** Per-fold active
counts over the probe (four lanes) and the 74 cases (three lanes), plus markedlil on the pool
(`LILSCRIPT_FOLD_REPORT=all` is forwarded by `workers.mjs build`; zodlil's production config runs
one emission and **no peephole**, so its artifact is pure emitter output and it contributes no
fold counts). 81 of 129 reported folds never fire on the corpus. Of the G1/G2 candidates,
idle everywhere: `fold_or_assignment_parens`, `fold_statement_negated_ors`,
`fold_empty_comma_operators`, `fold_empty_ternary_then_comma`,
`drop_pure_regex_expression_statements`, `fold_same_binding_strict_equality`,
`fold_fresh_empty_array_pushes`, `fold_fresh_empty_object_assign`, `fold_or_empty_object_assign`;
never even invoked: `collapse_double_async`, `repair_async_functions`, `fold_expression_bodies`,
`strip_parenthesized_range`. Still active (corpus / markedlil): `fold_redundant_loop_body_braces`
2,542 / 512, `elide_separating_keyword_spaces` 1,790 / 256, `fold_single_statement_control_braces`
1,601 / 266, `elide_asi_safe_semicolons` 1,477 / 281, `fold_negated_equalities` 422 / 295,
`fold_single_return_arrow_bodies` 328 / 170, `fold_negated_conditional_arms` 33 / 32,
`fold_redundant_and_parens` 0 / 256. On the *final* probe artifact, skipping any of the first
seven changes nothing — they fire on candidates the search does not pick — and skipping
`fold_single_statement_control_braces` makes the probe **85 B smaller** (4,048 → 3,963: the
search then keeps `f++` where it had `f=f+1`), a fold/search interaction for phase 7.

**G1, `elide_separating_keyword_spaces` — the emitter side.** `keyword_separator` decides the
space after `return`/`throw`/`await` from the next token (nothing before a string, template,
regex, `[`, `(`, `{`, `!`, `~`, `+`, `-`; a space otherwise, including `.`), the fold's own rule.
With the peephole off, the raw emission of the probe and all 74 cases has **zero** keyword-space
residue. The fold's activity fell 1,790 → 149 on the corpus, and every remaining activation is on
text an earlier fold wrote (the trace pairs are all `return` + `!`/`(`/`[`/quote) — so this fold
is now a repair of other folds and is deleted with them, not before. Bytes: probe 4,048 → 4,055
raw (1,508 → 1,509 Brotli), four cases move by ≤3 Brotli, markedlil +25, **zodlil −72** (its
production config runs no peephole, so every `return (` and `throw "` there was paying for the
space). The markedlil move is the layout and rename stages scoring text before the peephole —
noise-level, and the reason phase 7 scores final text only. Five unit tests that asserted the
old spacing say the new one.

**G1, `fold_single_statement_control_braces` — the emitter side (if/else).** `JsBranch::
compact_before_else` spells a then-branch braceless when its one statement cannot capture the
`else` that follows (an `if` without `else`, at any depth of braceless bodies, would); the
else-branch is `compact`. Both `if/else` emission sites (structured and state machine) use it.
Raw-emission residue of braced single-statement `if`/`else` bodies: 13 → 1 over the probe and
the 74 cases; zero byte diffs in three lanes (the fold had been producing exactly this); 1710
tests. The loop-body brace fold's residue is inter-fold (bodies the comma folds join first) and
`fold_single_return_arrow_bodies` has no emitter residue at all.

**Finding: the brace spelling is a codec decision, not printer hygiene.** With the braceless
`if/else` default: zodlil (no peephole, one emission) 32,572 → **32,619 (+47 Brotli, raw −38)** —
its 73 `}else{if(` chains compressed better than the mixed `;else{` form; **markedlil 9,456 →
9,342 (−114)** — the layout and rename stages, which score text before the peephole, now see the
shape the peephole would have produced and choose better; the probe and the 74 cases with the
peephole off are a wash (7,960 vs 7,955, two wins each, 70 ties). Net −67 over the two ports.
So `fold_single_statement_control_braces` — and by the same token
`fold_redundant_loop_body_braces` — are codec-dependent shaping decisions (G9's kind), to be a
knob first (`braceless_control_bodies`, default true = this emission) and a scored variant where
the search runs; zodlil's config, which runs no search, can set the knob. On markedlil the fold
still runs after emission, so its final bytes depend on the default only through the
pre-peephole scoring — which is the phase 7 argument for scoring final text.

**G1, `fold_negated_equalities` — the emitter side.** The presence test `x===void 0` /
`x!==void 0` is a node (`JsExpressionRoot::UndefinedTest`) so `negated()` flips the operator
instead of wrapping the test in `!(..)`. Raw-emission residue of negated comparisons: 2 → 0 over
the probe and the 74 cases; zero byte diffs in three lanes; 1710 tests. The fold's remaining
activity is on text other folds write.

**`braceless_control_bodies` is a knob and a scored variant.** `CompressionDecision::
BracelessControlBodies` (`"braceless-control-bodies"`, on in every priority), `IrJsOptions::
braceless_control_bodies` (default true), the registry family `braceless-control-bodies`
(`BeforeEntropy`, sequential, proposes the braced variant), and `JsBranch::body`/`before_else`
at both `if/else` emission sites. Default behaviour is byte-identical to `78aaf49` on the probe
and the 74 cases in the shipped lane; the new variant adds candidates (as it should). One
none-lane case picks the braced variant at Brotli 266 vs 265 — the search's admission is not
final-Brotli-exact, phase 7's problem. Port verification of `78aaf49` (UndefinedTest): markedlil
9,342 and zodlil 32,619, identical bytes to `e5c40c3`.

## Phase 4 — deliver the facts

**4a — the facts cross the boundary (landing).** `optimizer::IrFacts { effect_summaries,
finite_values, parameter_array_lengths }` is computed once per module the emitter sees
(`analyze_ir_facts`, beside `IntegerValueAnalysis` on the emission context and once per IR
variant on the candidate paths) and delivered to `IrJsEmitter::with_facts` as an `Arc`; the
escape lattice already crosses as `value_escapes` on the IR. No pass consumes it yet — phase 4
delivers, phase 6 consumes with its own measurement — so the gate is byte-identity: the probe and
the 74 cases in three lanes are identical to `1516ad0` and the candidate sets match. Delivery is
observable: `LILSCRIPT_TIMING` reports `facts_delivered` (82,181 facts over the probe's 381
emissions). First cut computed the facts inside the IR-variant emission loops and cost 3 s on
the probe; hoisted to one analysis per variant.
