# 012 — The second look: what 111 commits taught, and the plan from here

Parent: [index](index.md). Written 2026-09-05 at `acace54`, against a plan written 2026-09-04 at
`c9d0d3c` before any of it had started. The owner asked for the past to be densified
([progress](progress.md), [history](history.md)) and the plan re-thought against
[`finer/objective.md`](../finer/objective.md). This is the re-think. It does not edit 001–011;
where it overrides them it says so, and [009](009-phases.md) points here.

---

## 1. The test this migration has to pass

The objective is not a representation. It is: for every maintained port, a Brotli artifact smaller
than the best of Terser / Oxc / esbuild / SWC, at level 13, in a compile a developer tolerates,
with Closure ADVANCED's whole-program moves as table stakes and typed-source moves beyond them;
correctness by construction; a compiler change generic and fleet-verified (objective §1–§8). The
owner's gate for *this* work: behaviour never breaks, and when the migration is finished Brotli and
compile time are the same or better (2026-09-04, [001 D3](001-directives.md)).

Three things follow that the original plan under-weighted:

- **The fleet's losses are mostly ports, not the compiler.** Of eleven losses, eight are untyped
  transliterations mid-migration (katexlil's gap is "a distribution", the micromark family shares
  one core); the two clean-source losses are mobxlil +2,641 and jquerylil +731. The text layer is
  worth 189 bytes on markedlil. So this migration's *byte* claim is modest by construction, and its
  value has to be measured in wrong programs removed, compile time, and room the ports can use.
- **The compile-time complaint is real and specific**: markedlil pays 269 emissions at 248 ms each
  (66.7 s CPU of ~90 s) for one 34 KB artifact; jquerylil takes 807 s on the pool. Level 13 must
  stay the sweet spot (objective §3); today three of four ports are smaller *and* faster at 11.
- **Every wrong program on the ledger was found by an instrument this migration built**, none by
  the fleet. Fourteen so far, ten fixed, three open, one pre-existing on the owner's list; the sixth wrong-program fold surfaced on 2026-09-05.
  The instrument is the asset the objective's "beat, never merely tie" rests on, because a port that
  throws is not a win at any byte count (jquerylil, 061).

## 2. What the record established

Facts, each with its evidence in [progress](progress.md) or [history](history.md).

**F1 — the tree exists, inside the incumbent.** Phases 1–5 grew it by ratchet rather than beside
the string: `JsExpression` keeps every grammar operand and `code` is a derived cache; `JsBlock` is
a statement list rendered on demand; 19 statement kinds, no `Raw`, no text-appending API; every
name is a `Bind` with its spelling in a per-emission table; closures keep their trees; conditions
keep their trees; a `Respell` walk rebuilds only what changed; a scope tree and a sound renamer
sit on top; every node carries its IR origin and an 8-bit fact word. The arena `Shape` / `Spell` /
`Print` of [003](003-target-representation.md) — 16-byte nodes, children by id, no text anywhere —
**was not built, and nothing so far needed it.** The witnesses that 003 wanted from it exist on
the tree we have.

**F2 — correctness is the yield.** Fourteen live wrong programs (table in progress.md): three fired
in default configurations, two on `main` in shipped ports (live-12 reached from zod's source,
live-2 from jquery's), six were folds emitting valid JavaScript for a different program. The
finders: reading (5), `LILSCRIPT_SKIP_FOLDS` bisection (1), an agent's repro (1), the release gate
(1), the differential at classes (1), the classifier twin (2), the probe's 22-configuration matrix
(2), the port twin lane (1). The class [004](004-legality-by-construction.md) calls *still-possible* — a legal tree
that means something else — is the one that keeps shipping, and only instruments catch it.

**F3 — the bytes are where the plan said, and the two verification ports are already below the
pre-migration baseline.** markedlil 9,470 → 9,322, zodlil 32,489 → 32,415, with every correctness
cost inside. The gains came from printer-side retirements zodlil could see because it runs no
peephole (6.1–6.4: −155, −26, −23), from the emitter making shapes the folds used to repair
(keyword spaces −72, braceless bodies −114 on markedlil), and from naming — but only the form the
plan did not predict: **one idiom group pinned as the baseline of the whole search** is −1,168 net
on six ports, while the same group as a late re-spell wins nothing and `FrequencyDesc` loses four of
five. Names steer `CompressionSimilarity` layout and every text-scored decision after them
(056/059: frequency order is entropy coding; convergence pays for it globally).

**F4 — the scoring order is the pathology under phase 6.** The candidate stages score the
*pre-peephole* rendering; the folds normalise afterwards. So `elide_block_terminal_semicolons =
false`, `comma_expressions = false`, `mutation_spelling` variants never reached a final artifact —
their whole life was what the intermediate folds saw — and a fold at the end of the pipeline
(`;`→`,` at module level, ASI `;}`) could be retired neutrally while a mid-pass one (the inner join,
unit-counter spelling) could not: **a printer-side port is neutral exactly when the fold ran after
everything that could have consumed its input** (6.3). The same order is why the idiom axis wins as
a base and loses as a late candidate. And the origin census shows a second population: the
compiler's *string-surgery* candidates (`top_level_declaration_variants`, the function-leading
respelling, pooling) re-create the shapes the emitter no longer writes — 146 and 536 activations,
all on derived text. Both are one defect: **the artifact is scored before it is final, on text the
pipeline still rewrites.**

**F5 — compile time is the emission count.** Per emission the emitter is roughly linear now
(`PairGraph` took the probe from 16.7 s to 5.8 s wall; markedlil's 248 ms for 34 KB against the
probe's 43 ms for 4 KB is ~n^1.1). The multiplier is 269 emissions for 56 admitted axes of which
53 are printer-or-local and 3 structural ([003](003-target-representation.md) §Level 1: "roughly 16
realised bases would serve 270 emissions"). The peephole is 11.6 s of that compile, the codec 6.5 s
over 492 calls; on posthoglil, before `PairGraph`, the codec led at 87.7 s. Both profiles say the
same thing: **a spelling variant costs an emission plus a peephole today and should cost a print.**

**F6 — where the measured byte levers are, for the compiler.** In order of measured ceiling
([`finer/status.md`](../finer/status.md) leads): the single-use assignment collapse Terser's
`collapse_vars` + `unused` take from our artifacts (+280 / +296 micromark, +56 / +132 mobx, +136 /
+94 jquery; 243 of 244 micromark sites have nothing observable between), the idiom axis (above),
the function-scope wrapper for module state (048: perf, −209 net Brotli on 21 ports), dissolving
module-init singletons (048), the for-init `var` list 6.3 had to revert (waits on single-use), the
redundant number coercions (047: 64 sites on katexlil), the pooling benefit model (011), and the
starved families (46 of 47 at micromark's config). The first is G4/G5 on the tree with the fact
word phase 4 delivered — the migration's byte item, directly on the objective.

**F7 — what the plan got wrong.** Byte identity as the gate (owner revised it; it was the right
*evidence* and the wrong *bar*). "22 folds become unreachable at phase 3" (they fire on other folds'
output and on option variants; corrected in 009). "Phase 6 before 7" (F4 says the reverse for
every mid-pass fold). The 61-config sweep and the frozen 61-config baseline (the owner narrowed
verification to two ports plus the probe and the cases; that has been enough to find fourteen wrong
programs, and it is what the pool runs in 20 s). `take_trailing_expression_statements` as a
superlinearity (0.016%). Timing claims before `ed20326` (a ±17% code-alignment lottery on the
emitter's hottest loop, proved with an uncalled function). Phase 5's promise that `rename.rs`
deletes when an ordering matches it on the fleet (the mechanism landed; the fleet said per-port).

## 3. The vision from here: the print is the candidate

The original design already says it ([003](003-target-representation.md) §Level 1,
[006](006-candidate-derivation.md) §3, [planned-architecture](../docs/knowledge/compilation/planned-architecture.md)
§4–5). What changes is its place in the order and how much of it is already true.

> **One emission per IR variant. Every spelling, naming and layout decision is a print. The
> artifact is scored only when it is final.**

The pipeline it describes, against what exists:

| stage | what it is | state |
|---|---|---|
| IR variants | the optimizer-option clones (`inlining`, `scalar_replacement`, …) — the 3 structural axes | exists; the only tier that should re-emit |
| emission → tree | one `build_module` per IR variant; the tree is *canonical*: every branch kept, every run kept, no spelling chosen | exists as `JsBlock`/`JsStatement`/`JsExpression`; spellings are still chosen at emission for ~20 options (comma runs, braces, mutation, quotes, keyword, `void 0`, …) — **the work of 7a–7b** |
| names | a `BindTable` written by an ordering, `Respell` follows | exists (phase 5) |
| layout | `CompressionSimilarity` orders functions by an 8-gram profile of their printed text | exists; must run per print, after names, before scoring |
| print | `render` with `JsRenderOptions` / `JsStatementOptions` | exists for expressions and statements; the printer-only options must all live here |
| structural rewrites | G9/G10/G13 as `ShapeTransform { sites, apply(subset), polarities }` overlays the beam proposes | **not built**; needed before those groups open (unchanged) |
| validate, score, compare | oxc gate, pinned codec, incumbent retention, `plan_identity`, budget prefix | exists; **the score must move to the final text on every path** — the peephole, while it exists, runs *before* scoring, never after |

What the rule buys, clause by clause of the objective:

- **Compile time (§3):** a spelling variant is a print (linear, no naming, no out-of-SSA, no
  peephole). markedlil's 269 emissions become the IR-variant count. Level 13 buys more with the same
  CPU, and the ladder is re-derived rather than inherited ([006](006-candidate-derivation.md)).
- **Bytes (§1, §7):** the search stops ranking spellings a fold undoes; the axes it "samples away"
  today (the idiom group as a base, the second Cartesian seed) are affordable as prints; layout and
  names are scored where they act. The string-surgery candidates become tree overlays or die.
- **Correctness (§7, D1):** nothing rewrites text between scoring and shipping, so the class of
  6.x findings — a fold reacting to a shape the emitter never wrote, a fold blind to a back edge —
  shrinks with every printer decision that moves in. The oxc gate stays at the end.
- **Room (010):** `ShapeTransform`s over a tree that carries `PURE`, `NO_THROW`, `OWNED_SLOT`,
  binding identity and the effect summaries are where the single-use collapse and the literal
  fusion go — the leads in F6.

What it does **not** need: the arena rewrite. The tree's per-emission cost is linear enough; the
16-byte node, the overlay-by-id, the print memo keyed on reachable spellings are optimisations to
take when a measurement asks for them, and the rule "no decision from text; `code` is a cache" is
kept without them. This is the one substantive departure from 003, and it is a deferral, not a
rejection.

## 4. The sequence from here

Phase numbers keep their names so the ledger stays readable; the order changes.

### 7′ — one emission, many prints (next)

0. **Read first** (objective §10): the printer/option layer of each competitor for this class —
   Terser's `OutputStream` options and `best_of`, its `compress` pass order (`sequences`,
   `join_vars`, `if_return`), `scope.js`'s mangler; Oxc's `oxc_codegen` options and
   `minimize_statements`; Closure's `CodeGenerator` / `CodePrinter` and the peephole pass order.
   Recorded in `refs/competitor-techniques.md` before 7a lands.
1. **7a — score final text only.** Every candidate stage and terminal family scores the text the
   pipeline will ship; with the peephole still present it runs before the score. Gate: DECLARED —
   the ports may move (they should not lose: 6.2 showed `;` winners from unscored paths were tiny
   artifacts); the probe matrix, behaviour, twin, tests, pool. The `higher_effort_…` pins re-set.
2. **7b — the printer-only options print.** The 20 fields classified printer-only become render /
   statement options and a variant on one of them is a re-print of the same tree — byte-identical
   to today's emission with that option, proved per option on the cases and the probe. `emit_calls`
   is the metric; each option moved records its share.
3. **7c — the local options as overlays or re-emissions, by measurement.** Of the 46 bounded
   local rewrites, those the tree can carry as overlays (both branch spellings kept, runs kept)
   re-print; the rest keep re-emitting and are counted.
4. **7d — names and layout inside the print loop.** The ordering axis (`name_ordering`,
   `idiom_group`) becomes an ordinary spell axis scored after layout; the "baseline vs late"
   distinction of 5.5b disappears by construction. Then the fleet A/B of the idiom axis as a
   search decision, and 5.5's deletions if it matches the text pass.
5. **7e — budgets re-derived.** The ladder per port from the new cost model, with the L11 evidence
   as the first data point; level 13 re-earns its sweet spot or the default moves on a fleet measure.
6. **7f — the end-state measurement**: markedlil and zodlil bytes and `emit`/`codec`/wall on one
   worker against `54e1948`; then jquerylil, mobxlil, posthoglil, cnlil.

### 6′ — the fold groups, under 7a

Each group by the two-commit protocol of [007](007-fold-disposition.md), with the solo census
(`LILSCRIPT_ONLY_FOLDS`) as the residue instrument:

- **G1 remainder** — the inner comma join, the unit-counter spelling, keyword spaces and redundant
  parentheses on other folds' output: print policy, retirable once 7a stops anything scoring
  between print and fold.
- **G3, G7, G11** — the emitter stops writing `=void 0`, the `arguments` alias, the split
  allocation; each removes a named emitter decision.
- **G4, G5 — the single-use collapse on the tree** with `PURE` / `NO_THROW` / the effect
  summaries: the byte lever of F6 and the shape 6.3's `var`-list absorption waits on.
- **G6** from `INT32` on the node (1,554 lines become a field test).
- **G8** — the latch from `ControlShape::Loop { update }`; closes two shipped wrong-program folds by
  construction ([004](004-legality-by-construction.md) §3).
- **G9, G10, G13** as `ShapeTransform`s, the type built first.
- **G12** last, shrinking as ports type.

### The correctness track, continuous

- **live-9** now (a level-15 `||`/`&&` chain drops a side-effect call; the probe matrix's one red
  row) — minimise, fix at the family's admission if it is a scored axis.
- **live-1** and **live-2** are owner decisions (§6).
- Every compiled artifact runs under `timeout` (live-14 was a hang); the probe matrix, the twin and
  the pool check are the per-commit gate; the differential with a random seed runs in CI; a second
  probe covering classes, closures and the state machine is written (the first has no receiver
  parameter, which is how the `this` bind leak hid until a port).

### 7½ — the chain-head rule, and its limit (added 2026-09-05, ledger 7.9)

A text fold at chain position *k* can move to the emitter byte-neutrally when every fold before
*k* is idle on the text in question, because the emitter's output is what fold 1 sees. So the
porting order inside phase 6 is the chain order, earliest active fold first, and the census
(`fold-census.sh`) names it. The limit: the candidate ranking scores pre-peephole text, so a port
that changes the emission changes which candidate wins even when every candidate's final text is
unchanged (7.9: markedlil's winner moved to another spelling family, +22). Byte identity under the
search is therefore 7a's to give, not the port's; with the search off the rule holds exactly.
The terminal slot's first occupant is the leading `let`/`var`, re-decided on the final text.

### 7a, measured (added 2026-09-05, ledger 7.10)

Built as "peephole every emission before scoring, codec-verified", 7a wins on markedlil locally
(−44) and loses on the fleet (+653 over 19 ports, mobxlil +429). The search's exploration keys on
distinct text — the frontier dedup, `seen_code`, the entropy sources — and folded emissions collide,
so it explores less. So 7a's true form is not "fold earlier" but "explore by plan identity, score by
final text": the seeds and the dedup key on the plan (context, options, tree), the score on the
text that ships. That is 7e's re-derivation, and until it lands the knob stays off and the
chain-head rule is verified with the search off and with `LILSCRIPT_EMISSION_PEEPHOLE=1`.

### Batching (owner, 2026-09-06)

From here the work goes in batches: several ports, conversions or deletions per build, one
verification batch per batch (markedlil search-off identity, the three lanes with behaviour, the
tests, the pool), one ledger row per batch naming the pinned binary. Reading stays per item;
building and verifying do not. A batch that diverges is bisected inside itself with
`LILSCRIPT_ONLY_FOLDS` and the identity checks, not rebuilt per item.

### 8 — retire the text layer

Unchanged exit criteria ([009](009-phases.md)), reached group by group.

## 5. What this migration owes the fleet

So that the architecture is measured in the objective's unit:

| lever | measured value | where it lands |
|---|---:|---|
| single-use assignment collapse (Terser `collapse_vars` + `unused`) | +280/+296 micromark, +136/+94 jquery, +56/+132 mobx when ablated from Terser on our artifacts | G4/G5 on the tree, after 7a |
| idiom group as the search's base | −1,168 net on six ports, pinned | 7d as an axis; per-port pins meanwhile |
| module state in a function scope | −209 net on 21 ports; cnlil 1.15 → 1.00 runtime | an emission family (048 lead 0b) |
| dissolve module-init singletons | cnlil +283 → −367 by hand | scalar replacement past `LocalOnly` (048 lead 0c) |
| redundant number coercions | 64 sites on katexlil | emitter, with `INT32`/`ValueKind` |
| the for-init `var` list | +5/+8 on the cases when taken early | after G4/G5 |
| starved families | 46 of 47 at micromark's config | 7e |

The ports' own losses — the transliterations — stay the ports' work; the compiler's part there is
G12 shrinking as they type, and the fact that typed ports already win.

## 6. Decisions only the owner can make

1. **The arena representation** (003's `ShapeNode`): defer until a measurement asks, keep the
   rule. *Recommended: defer.*
2. **live-1, `extern` by spelling**: the fix is a language feature, `extern("Math.round") …` with a
   declared host binding, and the 105-name table deletes; a hard error today would break ports that
   rely on the table. *Recommended: the feature, behind the prelude, in its own folder.*
3. **live-2, the receiver**: making the `function` spelling of a lexical closure capture the
   lexical receiver so both spellings agree costs bytes where captures are needed; the alternative
   is refusing the arrow axis for receiver-using functions (061's admission item). *Recommended:
   admission refuses first (correct today), the capture lowering measured after.*
4. **zodlil ships its dev config** (level 8, the fold layer off) because a fold miscompiled; the
   real config is −2,736. The migration has fixed six fold miscompiles since; re-testing zodlil at
   its shipped config is a port decision with real bytes. *Recommended: re-test now.*
5. **The verification set**: two ports + probe + cases per step (current) and the clean ports at
   phase ends, or the 61-config sweep. *Recommended: current, plus jquerylil and mobxlil at 7f.*
6. **Level 13 as one number or per-port ladders** (F5, 7e). *Recommended: the default stays 13 and
   ports may pin what their curve earns, as objective §3 already allows.*
7. **G12**: keep the recogniser on the tree, or type the ports (katexlil's classes, mobxlil's
   tables). *Recommended: the ports, with the recogniser scoped and shrinking.*

## 7. What "done" means, revised

The migration is complete when:

1. no production path re-parses generated JavaScript to recover identity, and nothing rewrites text
   after the score;
2. every port is at or below its `54e1948` Brotli under its own objective at its shipped level;
3. `emit_calls` per compile equals the IR-variant count, and emit CPU and wall on one worker are
   below `54e1948`'s, for markedlil, zodlil, jquerylil, mobxlil, posthoglil, cnlil;
4. the probe matrix is 22 of 22 and no known wrong program is open; classes 1–3 of
   [004](004-legality-by-construction.md) are closed or owner-decided;
5. the instruments in *progress.md* run on every commit.

Point 4 is the one that matters; it is also the one that was true of no compiler on `main` when
this started.

## Found while batching (2026-09-06)

- **The text chain is adopted whole or not at all.** At the terminal the canonical pass is one
  chain run scored once against the emission (`apply_search_off_declaration_peephole`,
  `finalized_javascript_candidate_precedes`). On markedlil the chain's result was 10,022 Brotli
  against a 10,014 emission, so the emission shipped; skipping any one of five folds gave
  9,956–10,008. `fold_while_trailing_increments` alone was a 49-byte loss on the unjoined text
  too. The folds were never monotone under the codec; only the chain's *sum* was measured. Each
  port moves that sum, so a port can flip the whole chain off and read as a regression that is
  really the chain's. The instrument: `LILSCRIPT_PEEPHOLE_TRACE=1` now names the refusal at every
  terminal site (`[peephole-refused] …`), `LILSCRIPT_PEEPHOLE_DUMP=<path>` keeps the refused text.
- **A printing decision read from a thread-local at print time is a wrong program.** The join
  was gated on `StatementPolicy::current()` inside `JsBlock::render`; the pool's candidate
  re-prints run on rayon threads that never installed the policy, so a braceless two-statement
  body printed `for(…)a;b` — two case-lanes wrong, caught by the pool check, bisected with
  `LILSCRIPT_SKIP_PORTS`. Every shape is now a property of the tree (`JsBlock::comma_join`, the
  branch's `braceless`), decided on the emitting thread; the policy is read only at construction.
- **Closures are rendered when built**, before the module's shaping pass; they take the shapes
  in `render_closure_statement`, so the stored tree and the text agree.
- **Emitter ports carry a kill-switch**: `LILSCRIPT_SKIP_PORTS=comma_join,for_init,…` turns one
  off at run time, the emitter's `LILSCRIPT_SKIP_FOLDS`. A batch of ports is bisected on one
  binary, on the pool, in minutes.
- **The work-unit ledger charged idle attempts.** Deleting three late-cleanup passes that never
  rewrote anything moved katexlil +1,004 and posthog +69 (7.30): every (pass, candidate) attempt
  reserved a unit before the pass ran, so idle passes were spending units the later naming families
  then lacked. With the fourteen deleted folds merely *skipped* (`LILSCRIPT_SKIP_FOLDS`, ledger
  intact) katexlil read the same 64,878 as before. The ledger now charges a unit when a pass yields
  a proposal the codec scores (7.31); a fold's deletion is then byte-neutral by construction, which
  Phase 5.5 needs.
- **The search is not deterministic under parallelism, since 7′.** posthoglil at `54e1948` compiles
  to the same 5,559 bytes at 1, 4 and 4 threads; at `2bf678f` (7.24) it reads 5,600 / 5,626 / 5,600
  and every later binary varies by tens of bytes per run and per thread count. remarklil and
  markedlil happen to be stable. This is why pool numbers for the same binary differed (remarklil
  37,698 / 37,190) and why single-port deltas under ~50 bytes proved nothing all day. `git bisect
  run` between the two, on "posthoglil compiles byte-identically three times", names the commit;
  the suspects are the shared re-print cache (7.20) and any constructor-time policy read on a
  thread that never installed it. **Named (7.33): 7.20, the cache.** Fixed by re-printing every
  plan under a key from the canonical spelling's tree, emitted on demand; the pool's repeat runs
  and the fleet numbers are stable again from 7.33 on.
- **live-9, reproduced (7.33).** Level 15, search on, the probe: `f=j(o)||j(o+1)` where `j` counts
  its calls (`h++;return 0==(o&1)`) ships as `b=0==(a&1)||0==(a+1&1)` — the first call's increment
  is hoisted before the statement, the second call's is dropped, so the count is short by one
  (line 32: 21 for 31). With the search off the level-15 build is correct; with the search on the
  wrong text is a late-cleanup candidate that wins only when all four of
  `fold_single_use_function_expressions`, `fold_zero_argument_return_iife`,
  `fold_expression_self_assignments` and `fold_top_level_adjacent_expression_statements` run
  (`LILSCRIPT_SKIP_FOLDS` of any one gives 31). The late cleanup is text the tree is retiring; the
  fix is either the pass that inlines a body with a statement before its `return` into a
  short-circuit operand, or the deletion of that pass. `$SP/level15.toml` and `runner.js`
  (`globalThis.read` counting) reproduce it in one second on `probelil/src/probe.lil`.
- **A tree rewrite before scoring is a lottery ticket; the same rewrite at the terminal is a
  slot (7.34–7.36).** The single-use collapse, run on every emission, read +128 net on the fleet
  with six wins and six losses, the ten-port numbers byte-identical between two runs: not
  noise, plan choice. A rewrite that shrinks most emissions by a little changes which plan the
  search ranks first, and the plan it moves to prints its *other* shapes worse (remarklil's new
  winner lost the exclusive-closure inlining, the `void 0` drop and the regex spelling in one
  go). The text folds it replaces never had this problem because they run on the finalist and
  are codec-verified there. So the tree's off-for-cause shapes -- the collapse, the for-init
  hoist (+76), the negated arms (+36 on micromark) -- ship as **terminal shape challengers**
  (`terminal_shape_options`, 7.36): one re-emission of the finalist's plan per shape, admitted
  by `finalized_javascript_candidate_precedes` like the naming and string-pooling families.
  Cost: one emission and a declaration ladder each, from the terminal ledger. This is the
  general rule for the rest of phase 6: a shape that is a pure win on the cases goes on every
  emission; a shape that wins on some ports and loses on others goes to the terminal slot.
- **The re-print cache raced on its accounting (7.34).** Two siblings under one key arriving
  during the canonical emission both emitted it; the text was identical (7.33) but the count
  was thread-order, and `compiler_resource_counts_preserve_exact_selected_javascript` failed
  4/4 on b24 (49 against 48) after passing on b23 -- the collapse changed the timing, not the
  logic. One `Mutex<Option<tree>>` slot per key now: the first arrival emits while holding it,
  the rest wait and re-print. One attempt is counted per request whichever path serves it.
- **`void 0` was not a literal to the prune (7.35).** `expression_is_pure_literal` knew `null`,
  `undefined` and numbers; the emitter spells undefined as `void 0`. katexlil carried a
  5,924-byte statement of dead phi copies (`var ZW=void 0,_W=void 0,…`) into the terminal
  chain, where `fold_void_initializers_off_fresh_vars` and `remove_unused_standalone_vars`
  stripped it one token at a time. With the search on the bytes were already gone; with it off
  the port read −482 Brotli, −6,659 raw.
- **A terminal challenger must be a print of the finalist's tree, not a re-emission of its plan
  (7.37–7.38).** Re-emitting the finalist's plan with a shape flag and finishing it again lost
  71 to 101 bytes on markedlil every time -- with the shape doing nothing there (the collapse
  everywhere gives the identical 9,178). The finalist's text is not `emit(plan)`: it is the
  search's tree plus the rename family's spellings plus the late cleanup, and a fresh emission
  has none of that even when the ledger is extended to pay for a second finishing (7.38's first
  try: the ledger grew by the finishing's cost, the challenger still finished 71 worse). The
  challenger that compares like for like is the finalist's *frozen tree* (`frozen_tree`, the
  re-print cache under the plan's key) with the shape applied on the tree (`ModuleTree::collapse`
  over the module block and every closure body) and printed the way the plan prints it
  (`reprint_collapsed`, the rename pass first when the plan re-spells), then scored and finished
  through the same gate as the naming and pooling families. On markedlil that is one challenger
  offered and selected, 9,189 → 9,180. This is Phase 7′ arriving from the other side: shapes
  as prints of one tree, one codec probe each, and the finishing paid once.
- **The terminal ledgers were spent before the shape stage (7.37).** Two of them: the codec
  probes (exhausted on every port, `terminal_codec_probe_limit_reached`) and the plan slots
  (`candidate_limit` minus the finalists, then the naming and pooling families). The shape
  family now holds its own codec slice (`release_shape_reserve_once`) and is granted its own
  plan slots (`TerminalJavaScriptCandidateBudget::grant`); the second finishing, when the gate
  passes, extends the ledger by what the first cost (`extend`), so a win is finished on equal
  terms and a loss costs one emission's worth of probes.
- **Print against print (7.39).** The unshaped print of the finalist's tree is not its emission
  byte for byte (markedlil: 9,471 against 9,484 before finishing), so a shaped print judged
  against the emission measures the print's drift, not the shape. The gate's incumbent is now the
  unshaped print, one codec probe; a shaped print that beats it is finished and judged against the
  artifact. With that, markedlil 9,178 → 9,169 and the ten heaviest ports −22 for +2% wall. The
  drift itself is Phase 7′'s remaining debt: when the print *is* the artifact, there is nothing to
  drift from.
- **Concise closures have no tree (7.39).** `render_closure_body` renders `a=>a*7|0` as text and
  `closure_node` finds no registered rendering, so the value is a `Raw` node: the collapse's
  census sees its names as text (unsafe), the beta reduction sees no `Closure` callee, and the
  rename pass cannot look inside. Registering concise closures as `ConciseNode` trees (head
  pieces with binds, the body as an expression node) is the prerequisite for G4/G5's IIFE folds
  (`fold_identity_arrow_iife` 104, `fold_zero_argument_return_iife` 31,
  `fold_single_use_function_expressions` 61 on the cases) and would tighten every census.
- **The closure trees were one `render` away (7.40).** The deferred expression-closure path
  rendered its `Function` node to text directly; every concise closure was a `Raw` node to the
  tree. Routing it through `render_closure_statement` registers the tree, and the difference
  shows at once: the beta reduction fires on the four cases that had an immediate call, and the
  probe's level-15 configuration agrees again (live-9 no longer reproduces, since the closure
  the late cleanup mis-inlined is now a tree the print handles). The lesson for the remaining
  text-rendering sites (`grep 'JsStatement::Function {' | grep render`): a node rendered to text
  is a node the tree cannot see, and the census, the renamer and every shape are only as good
  as the tree's coverage.
- **The census now counts the finishing twice (7.40).** With the shape stage's second finishing,
  every text fold that fires on a case fires again on the finished challenger, so
  `fold-census.sh` reads roughly double (`fold_expression_self_assignments` 69 → 150). Residue
  is measured with `LILSCRIPT_TERMINAL_SHAPES=0`; the doubled numbers are not new work for the
  tree.
- **The fold report sees `session.run`, not direct calls (7.42).** Two folds the report called
  idle everywhere are called by name from the compiler's repair and canonical paths
  (`fold_redundant_null_undefined_or`, `fold_dead_identifier_copy_declarators`, lines 8822 and
  10999 of `compiler.rs`), and `fold_empty_comma_operators` is a helper of a class fold. A fold
  is deletable when it is idle *and* `grep` finds no caller outside its registration; the
  `delete-batch.py` dry run reports registrations, the build reports the rest.
- **With closures as trees the everywhere collapse turns (7.43).** 7.34's +128 over 20 ports
  becomes −286 over nine of the ten heaviest -- the census that counted closure names as unsafe
  text was refusing most of the wins -- and one port, katexlil, loses 736 with the search on
  while gaining search-off. That port's terminal ledger is exhausted on every run (`terminal
  work 256/274`), so which plan finishes first decides the artifact by hundreds of bytes: the
  7e re-derivation is now the blocker for flipping a measured-good default, not the shape.
- **The ledger decides katexlil (7.44).** Pinned at 384 probes the collapsed path is −68; at
  the scaled 256 it is +736; the uncollapsed path does not move. The artifact scaling
  (`gradual_artifact_work_limit`: full to 16 KB, a quarter at 64 KB, a twelfth at 256 KB) was
  measured for compile time, and on the largest port it leaves the finishing -- worth about 2 KB
  there -- fewer probes than a plan needs to be judged. 7e's shape: size the ledger by the
  finishing's marginal bytes per probe on the artifact (the counters exist: `rename_won_sum`,
  `cleanup_shaped_pushed_sum`), stop when a family's last N probes bought nothing, and let the
  everywhere collapse ship on that.
- **live-9 is the emitter's, and the collapse is its detector (7.45).** With the collapse on
  every emission the probe at level 15 ships `f=0==(b&1)||0==(b+1&1)` for
  `f=X(b)||(W=W+1|0,X(b+1|0))`; the block dumped before the collapse runs already holds the
  wrong text, every text fold skipped leaves it wrong, and the collapse skipped makes it right.
  So the miscompile is emission-time -- the plan where the pure helper is inlined into the
  short-circuit's right operand renders the phi region without its prefix statement -- and the
  collapse merely makes that plan the smallest. The fix is in the region rendering; until then
  the collapse stays off on the search's plans (it ships as the terminal print), and the fleet
  win it measured (−660 over 20 ports with the ledger at its base) is the prize for fixing it.
- **live-9 without the search (7.46).** `LILSCRIPT_EMISSION_DUMP` names the plan: every wrong
  emission belongs to one IR context, the variant with CSE off, specialisation off and the
  aggressive inline limits, and that variant compiles wrong search-off with the collapse off --
  `migration/tools/live9.toml`. So the everywhere collapse was a detector twice over: it made a
  latent miscompile the winner, and the dump made the winner nameable. The lesson for the
  search: a plan that miscompiles is not a plan that loses on bytes; the standards parser
  admission catches syntax, nothing catches semantics, and the probe harness is the only net.
  Two things follow: an IR printer (there is none) so optimizer losses can be seen before the
  emitter, and the probe's 22 configurations run on every IR variant the search admits, not
  only on the shipped plan.
- **live-9's mechanism (7.47).** The region renderer carries an output-less effect as a comma
  prefix on the *next* value's cache entry. Values that consumers inline by value -- constants,
  anything in `inlined_values` -- never go through the cache, so an effect parked on one is
  written and never read. It took a plan with the helper inlined and CSE off (so the arm held a
  raw `Const` right after the store) to expose it; with a call in the arm the effect lived inside
  the call and nothing was parked. The rule now: an effect waits for a value the consumers read
  through the cache. The general lesson is the one from 7.40 in another form: a value the tree
  serves by text or by value is invisible to whatever hangs state on the tree's entries.
- **The ladder's passes and the tree's shapes are not the same set (7.50).** Cutting the eleven
  cleanup passes with a tree twin read +629 on the ten heaviest ports, jquerylil +496; the five
  control shapes restored brought jquerylil to −9 and one port needed one more:
  `BooleanConditionalValues`, and in its early slot -- the tree's `boolean_arms` sees the
  construction-time `true`/`false`, the text fold sees the printed `!0`/`!1` after the
  compact-literal print, so its work only exists on the finished text. Two facts to carry: a tree
  shape at construction and a codec-verified rewrite of the finished text are different
  opportunities even when they spell the same rule, and a pass's position in the ladder is part
  of its identity (the same pass at the end of the ladder was worth nothing). The measurement
  loop for the next tranches is now cheap: `LILSCRIPT_CLEANUP_LADDER=Name,..` on the pool, one
  port, one pass, a minute each.
- **Every late rewrite that can be adopted gets the standards parser (7.51).** The structural
  challenger chain (the fixed pass list the late cleanup applies to each beam entry, up to four
  variants) admitted its result through `validate_inner` only, the same gap 7.38 closed for the
  shaped families and the shape prints; it has the parser now, byte-identical on the three
  heavy ports. `LILSCRIPT_CANONICAL_CHAIN=Name,..` keeps only the named passes in that chain
  for a per-pass A/B without a rebuild; with `LILSCRIPT_CLEANUP_LADDER` and `LILSCRIPT_SKIP_FOLDS`
  every text stage is now measurable one rule at a time from the environment.
- **One rule at a time, one port at a time (7.50–7.51).** The tranche method that works: pick
  the port that lost most when a group went, run it with each rule removed alone from the
  environment (`LILSCRIPT_CLEANUP_LADDER`, `LILSCRIPT_CANONICAL_CHAIN`, `LILSCRIPT_SKIP_FOLDS`),
  keep what the port still pays for, delete the rest, confirm on the ten heaviest. A minute per
  rule on the pool, no rebuild, and the answer is never a group's sum. Three tranches today by
  this method: five ladder passes, three chain passes, and the emission chain's folds next.

- **The emitter already owned the control shapes (found at 7.55).** `shape_block`
  (phase 6, G2/G5) is the tree's port of the return tails, early exits, continue
  tails and braces, run at emission under the plan's policy. The print ladder's
  `return_tails`, `exit_guards` and `rebrace` rungs call it again on the finished
  tree, after the collapse and merge changed what the branches hold; only the
  guard-suffix and branch forms (`if(c)return a;E;return b`, `if(c){E;return a}return b`)
  were new. The ladder's text passes for these fire, then, on shapes the emission
  refused under its policy or that later stages created -- the census
  (`LILSCRIPT_LADDER_REPORT=1`) says which.

### 7′, concretely (added 2026-09-06 after 7.53)

Three text stages still stand between the selected tree and the artifact, and everything the
remaining folds earn is earned on their output:

1. **The late cleanup ladder** (13 codec-verified passes) and **the structural chain** (6 passes,
   one variant dimension): text in, text out, each proposal one codec probe.
2. **`converge_local_names`** (`js_peephole/rename.rs`): per-scope renaming on the finished text,
   from a binding resolution it rebuilds by parsing; `rename_won_sum` 423 on markedlil, 1,149 on
   katexlil.
3. **`apply_selected_canonical_peephole`**: the emission chain (75 folds) once more on the winner.

The shape of the work: the finalist keeps its frozen tree (`frozen_tree`, 7.38); each ladder and
chain pass becomes a `TreeShapes` entry (the six with twins already are; the other thirteen are
new shapes: guard-return suffixes, negated equalities, null-normalised nullable tests,
or-assignment parens, arguments-length countdown, canonical leaf syntax, same-binding strict
equality, expression return branches, common conditional arms, sequence-assignment first use,
single-use function expressions); the ladder becomes a *print* ladder -- a beam of trees, one
reshape and one reprint per proposal, the same codec gate; the rename convergence runs on the
tree's spelling table (`Renamer` already knows binds and scopes, the text pass rebuilds them by
parsing); and the artifact is `reprint(best tree)`. Then the emission chain runs on nothing it
can improve, and its 75 folds go in bulk. First measurement to take: which residue folds fire
search-off on a heavy port at all (jquerylil with `LILSCRIPT_FOLD_REPORT=all`, shapes off) --
those that do not are the finishing's own creations.
