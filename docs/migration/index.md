# Migration plan: one compiler

Revision 2026-09-27, for version 1 of the design. **This is the only plan.** It does three things:
1. It takes the codebase from two compilers in one binary to the single compiler described in [future-architecture.md](../future-architecture.md) (done in M1).
2. It carries that compiler to version 1 of the design: the language contract [language.md](../language.md) and the compiler [future-architecture.md](../future-architecture.md).
3. It carries both to winning every maintained library under every objective, in both the open and the closed world, at runtime parity, within the compile-time budget.

Task ids are frozen (architecture §22): a merged task keeps its id and says "merged into"; a split task keeps its id with a letter suffix.

History and records:
- Milestones 001–013 (receipts, batch ledger, measurements, the Closure ADVANCED inventory 013-T7) are in [record-2026-09.md](record-2026-09.md).
- Owner briefs are in `finer/intent/`; today's is [2026-09-27](../../finer/intent/2026-09-27.md).
- The eleven area reports behind the 2026-09-23 page are in `~/lilscript-work/out/arch/`. The seven design reviews, the verified loss diagnosis and the katex runtime bisection behind version 1 are in `~/lilscript-work/out/design/review-*.md` and `~/lilscript-work/out/diagnosis/`.

---

## Where we are

### Version 1 of the design (2026-09-27)

The owner asked whether the migration is being done properly and asked for one final design, architected rather than iterated ([2026-09-27](../../finer/intent/2026-09-27.md)). The architecture answers the question in its §1.1: **partly no**, because mechanisms were deleted before their replacements existed, choices were built before rules, gates reported instead of blocking, and the language was left unfinished while the compiler recovered facts. Seven independent reviews of the design and the code, a verified diagnosis of the entries that still lose, a runtime bisection of katexlil, and three adversarial critiques of the first draft produced version 1. This revision of the plan re-scopes and re-orders every remaining phase to converge on it. Every landed record below is unchanged.

**What the reviews measured** (binary `head-d1d48c4c` unless stated):

| Finding | Evidence | Source |
|---|---|---|
| The compile-time gate (rule 3) fails | Level 13 at the ports' shipped configs, against pre-M1: zodlil 4.0×, posthoglil 3.9×, katexlil 3.7×, micromarklil 3.4×, markedlil 1.5×. Outside katexlil's one data-table choice, the extra time bought −88 to +7 Brotli. katexlil's package build went 9.6 → 54.9 s, against KaTeX's own build at 14.4 s | review-compile-time §2.5; measurements-2026-09-27.md |
| Exact Brotli-11 is the compile | 55–74% of wall time at level 13 on every reference port; raw bytes, the ranking the search uses, orders only 5 of 18 real pairs 200–1,000 bytes apart; Brotli q5 orders all 99 pairs ≥ 200 bytes apart at 1/60 the cost | review-compile-time §2.2, §4 |
| Effort above 8 buys almost nothing, non-monotonically | The search winner is byte-identical at levels 8, 13 and 15 on five of six ports; on jquerylil it is 25,466 at level 8 and 25,421 at 13. katexlil's level 13 is 98 bytes larger than its level 8 (two interleaved terminal budgets); level 0 ships 20–28% larger because naming is gated at 8 | review-compile-time §2.3–2.4; `compile-time-data/jquery-L{8,13}-production.explain.json` |
| The per-objective procedure misses its own optimum | katexlil's gzip-objective build is 188 Brotli smaller than its Brotli-objective build (posthoglil −28) | review-objectives §1.1 |
| The runtime gap came mostly with M1, and the adapter part is fixable | Steady-state Node ratio to upstream KaTeX: 1.102 for release-d; pre-M1 1.041; M1 added about 6 points. The recorded "20%" was an outlier of one recording run. Five shared receiver-adapter thunks serve all 141 methods, 9.6–10.0% of JS self time, one GENERIC call site. The adapter change alone (independent re-measurement): katexlil Brotli −62, gzip +2, raw +2,379; Node −0.041 [−0.058, −0.029]; Chromium −0.019 to −0.034; micromarklil −283 Brotli; mobxlil +130 Brotli unless constructors get formals; parity 30/30, Jest 1,230/1,230 | `~/lilscript-work/out/diagnosis/katex-perf-2026-09-27.md`, "Verification"; diagnosis addendum |
| The program rule layer is empty in production | Only liveness (with M7.2) runs on the Program IR; 54 calls to 39 passes run on the JS tree with their own effect and initialization models. `comparison/apps`: 945 against Closure ADVANCED's 834 Brotli (old route 571); `comparison/algorithms`: 3,250 against 2,756 (old route 2,305); all 18 cases lose | review-closure-advanced §2 |
| Local completeness trails every competitor | `comparison/cases`: 37 of 54 canonical cases and 380 of 570 catalog variants lose on Brotli to the best of Terser, Oxc and esbuild; `if (false)` bodies survive | review-competitors §1.5 |
| The language forces the ceremony the diagnosis measures | About 1,040 of the ~1,800 verified recoverable Brotli bytes on the five losing entries; 0 of 11 M10 items started | review-language §2.3; review-code-reality §5.1 |
| Facts landed without consumers | M6.1–M6.5 are computed on the program; the tree still re-derives effects and initialization (rule 2 was waived in `ff27361f`) | review-code-reality A6 |
| Bars and consumers | SWC is the strongest general-purpose bar on 4 of 5 measured boundaries (katex 62,399; remark-parselil's gzip win becomes a tie). A consumer importing one zod export pays 28,006 Brotli from zodlil against 11,749 from upstream (Rolldown) | review-competitors §1.1, §1.4 |
| Native is a subset | 236 of 361 corpus cases masked in C; no maintained library compiles natively ("native exported ABI"); the C output runs 3.3× slower than Node running our JavaScript for the same program | review-performance-native §3.4–3.5 |

**What version 1 changed against the 2026-09-23 page (architecture).**
- **The answer and stability.** §1.1 answers the owner's question; §22 fixes laws, IR, pipeline, procedure and contract behind owner-approved amendments and freezes ids.
- **Laws.** L3 now decides rule against choice by measurement (a removal with a codec-stable sign over the calibration corpus); L7 governs choices only; L20 (replace, then delete) and L21 (gates block) are new; L14–L19 are kept.
- **Objective.** Lexicographic: correct, not slower, smallest, within budget. Runtime parity is defined (architecture §16.4) as the owner ruled; it is not an open question.
- **One decision procedure** (architecture §9.6), stated once: a level-independent move list walked as a prefix; the level sets only the prefix, an exact-judgement budget and a work budget; proxies only prune; monotone across levels by construction; several objectives walk in lockstep with checkpoint offers; a gzip shadow for Brotli builds; the structural beam becomes one move until M9.1's rest deletes it. Program-level moves re-run the rules on what they change.
- **Rules.** One lexicographic measure, fixed structural order, never truncated by effort; transitional rules until replacements land; field-store, unit-merge, class-strictness, construction-order and regex-placement legality written down; `exact` folds only.
- **Annotations.** Binding facts and evaluation behaviour join the tree columns, each with a transfer function and debug verification, before `quiet.rs` goes.
- **Delivery.** Consumer annotations only where `discardable()` holds and only in the bundler-facing file (M3.8a/b); the IIFE default for application scripts with a global-facing refusal; a delta for the M3.3 session.
- **Native.** Shifts stay defined; `volatile` goes only with in-source FP guards; QuickJS's `libregexp`; trial-deletion cycle collection; named portable cores as M11's exit.
- **Language.** [language.md](../language.md) is the v1 target contract, tagged per clause; R2's spelling-observing operations are refused, R4 keeps in-place places with copy-on-store, R6's reflected set is a whole-program closure, R8 uses ABI values at reflected positions, R12 keeps JavaScript's `==` and adds `unknown`, R5 adds intersections, R9 adds specialization over const data.
- **Compile time.** Frozen baselines; targets per entry and per package build; levels 1–4 against Terser's time; B9's exchange rate and ceilings; CPU-time pairs per batch.
- **Runtime.** Hot-site alternatives ordered by runtime class before bytes; a runtime ledger with owners; per-batch timing gates in a quiet window.
- **Benchmarks.** BC5 maps language guarantees; BC10 judges shipped artifacts; the per-cell threshold (Y6); a corrected held-out set with a failure procedure; calibration never on ports; a closure ledger per losing cell (architecture §18.4).
- **Owner decisions.** Sixteen open questions became seven yes/no questions (Y1–Y7), each amending an earlier owner choice; the rest are decided with their reversal cost (architecture §21).

**What version 1 changed in the plan.**
- Rules 2, 3, 4, 5, 7 and 8 are sharpened; rules 9 (order), 10 (not overfitting) and 11 (stability) are new. Every gate blocks; phase gates are ratchets over ledgers; "no losing cell" is M12.3's exit only.
- **New tasks:** M2.10–M2.15 (benchmark contract, held-out set, perf runner and runtime ledger, generic ratchet, frozen baselines, calibration corpus), M3.8a/b, M5.3a/b, M5.7, M6.4a/b, M6.8, M7.5a, M7.8a, M7.10, M9.11, M9.12, M10.12–M10.19, M11.10–M11.12, M12.6.
- **Merged:** M3.6 and M9.4 into M3.5; M3.9's incremental formation into M5.7; M8.1 into M5.2 (annotations) and batch A1 (import identity).
- **Batch A** splits into A1 (formation fixes that need no tree analysis) and A2 (after M2.5, M4.6 and M5.2).
- **Order.** A floor slice of exact program rules and language slice 1 come right after A1; the budget model follows the floor slice; the core is split into batches by the ruling each needs; the critical path waits on no ruling (see "Effort and critical path").

### Now (2026-09-24, M1 closed)

**There is one compiler.** Branch `one-compiler`, merged into `finer/059-idiom-directed-naming`.

**What M1 removed and changed.**
- The old route is deleted: about 157K lines, including its 1,555 tests. The test-only tree experiment, the fixed two-file resource cut and interaction pairs are also deleted: about 18.7K lines.
- `--backend` and `[compiler] backend` are gone.
- Modules are named by role: `check`, `program`, `js`, `build`; `compile_source` and `compile_path` are the entry points.
- The LSP, playground and lint compile through the one compiler.
- 61 retired configuration keys warn ("no effect") or refuse, from one table applied before parsing.
- Old-pipeline docs moved to `docs/knowledge/history/`, and `docs/current-status.md` is rewritten.

**Correctness and coverage.**
- Four wrong programs were fixed, each with regression coverage: an async body inlined as its return value; field defaults created before constructor arguments; `==` on a `JsValue` lowered to `===`; and a raw-objective forwarding into an assignment target.
- 288 regression cases were harvested from the old route's tests.
- Two runners with expected-failure ledgers (`docs/testing.md`) replace the old census and port scripts.

**Evidence (binary `m1-merged`, then the M1.7 rename, which changed no artifact).**
- **Unit tests:** 1,462 pass; the binaries' tests pass.
- **Case runner:** 361 cases × 18 lanes, with no failure outside the ledger. Against the pre-M1 binary, no lane's size total grew; the production Brotli module lane went from 31,056 to 30,833.
- **Port suites:** all green (markedlil, zodlil, katexlil, jquerylil, posthoglil, motionlil, micromarklil). A pre-deletion fleet run found no compiler-caused failure in 26 ports; five known failures are ledgered with owners.

**Carried into later phases.**
- The M1.9 coverage gaps: struct values passed to an `extern`, generic struct transport, and the adapter-name spec question.
- Four production bugs found by porting the experiment's tests, now ignored tests with reasons:
  - `??=` on a place;
  - generic methods;
  - an explicit `JS.undefined()` argument still takes the default;
  - a detached `charCodeAt`.

  They are owned by M4 and M10.

### M5.4, M9.2 and M9.3's first family (branch `m5-terminal`, 2026-09-24)

**What landed.**
- **The terminal challenger stage (M5.4).** After the search picks each objective's winner, `program/search_terminal.rs` offers it a declared, ordered schedule of challengers (`js::Challenger`). Each one is formed from the winner's candidate, rendered with the winner's naming style, source names and literal mode, admitted by the same verifier and policy admission, and scored by the objective's exact codec. It is kept only when the complete artifact shrinks. The budget is the number of challengers the codec judges; it comes from the effort ladder (`terminal_challengers` in the policy receipt: 0 through level 7, then 3, 4, 5, 7 at 13, 9, and all from 15 or with `candidate_search = "always"`). The build report and `--explain` list every challenger with its outcome and byte delta. Formation reuses the candidate's demand plan and a formed head (everything before the families) for every challenger.
- **The objective as a judge (M9.2).** `OutputTactics.raw_structure` is gone. `OutputTactics.families` (`js::OutputFamilies`) holds block inlining, flat blocks, the statement spellings, string pooling, loop heads and logical statements. The objective only seeds them, and the stage offers the other alternatives: the raw families under gzip and Brotli, the codec seed under raw, the naming plan's raw spelling both ways, and each objective's whole other seed. Print decisions stay on the tree.
- **The first M9.3 family.** `compress_statements` is split. Same-exit merge, trailing-statement dedup and exit to `break` run under every objective. Conditional values, exit points, loop fusion, conditional returns and logical branches are five spelling groups, each a challenger.

**Evidence (binary `d2188075`, then a change to refusal handling only).**
- **Unit tests:** 1,474 pass. Two deep-tree tests overflow the default 2 MiB debug test stack on this host, on the base commit too; they pass with `RUST_MIN_STACK=16777216` and in release.
- **Case runner:** no failure outside the ledger, and formation-only lanes are byte-identical. Production lane totals fell: Brotli −498 and −706 (script, module), gzip −481 and −861, raw −95 and −186. Two case-lanes grew by 4 gzip bytes (`loop_carried_early_return`, exit to `break`).
- **Reference ports, main entries, against `release-m1`:**
  - Brotli objective: every port smaller, −517 in total. markedlil −18, zodlil −42, katexlil −75, jquerylil −44, posthoglil −54, motionlil −255, micromarklil −29.
  - Raw objective: none larger, −1,280 in total. motionlil −1,030 (block inlining off), posthoglil −161 (raw spelling off).
  - Gzip objective: every port smaller, −755 in total.
- **Port suites** (`scripts/ports.mjs`, with main's `--patches none` for the five ports whose repositories carry their rewrite): green at the shipped objective and under `--objective brotli` and `--objective raw`, the same as `release-m1`. The one exception, identical with `release-m1`, is katexlil's browser-performance test: its site build calls `git ls-files` in a copy without `.git`.
- **Delivered `dist/` files, per objective, against `release-m1`:** every file the compiler wrote shrank or held. Brotli: −1,650 over the seven ports. Raw: −6,026. Files the ports derive from ours moved either way: a banner prepended (markedlil's `marked.esm.js`, +2), esbuild's `.cjs`, `.min.js` and bundles (motionlil's `index.cjs` +133 while its `full.js` fell 201). zodlil's esbuild `index.cjs` grew 52 Brotli while `zod.core.js` fell 42, so zodlil's `dist/` total is +10 Brotli.
- **Compile time (level 13, three alternating pairs, main entries):** markedlil 0.80→1.14 s, zodlil 0.80→1.96 s, katexlil 3.84→8.74 s, jquerylil 30.6→32.2 s, posthoglil 0.12→0.39 s, motionlil 38.8→44.0 s and micromarklil 0.63→2.04 s. The cost is exact codec scores. It is open against rule 3's phase-end gate; the bounded codec pool (architecture §9) and estimator-ranked finalists (M9.4) are where it is recovered.

**Open.**
- Monotone selection holds within a search and within the stage (tests in `search_terminal_tests.rs`), and every level is at most the search-off result. Between two search-enabled levels whose search winners differ, it is not guaranteed. Beam width, retained capacity and the effort-gated tactics shape the trajectory. Replaying the lower level's schedule as the higher one's first phase closes this, with M9.10's recalibrated ladder.
- `pool_strings` and `pack_string_arrays` still ignore the `string-pooling` and `string-array-packing` permissions, and packing's startup risk, as before this batch.

### M9.1's first slice, M9.8 and the L2 spellings (branch `m9-data`, 2026-09-27)

**What landed.**
- **The choice kernel (M9.1, first slice; `js/choices.rs`).** A `ChoiceKey` names one site of one family by a stable identity (for data, the root binding a table initializes, never an expression id formation renumbers); an `AltId` names an alternative; a `ChoiceMap` is an immutable, sorted, shared assignment on the artifact's output assignment (`OutputTactics.choices`, next to the families), so every decision is data on the tree. Formation records each site it found (`Module.choice_sites`): its alternatives with the estimator's raw saving, its seed and what it applied.
- **Choices in the terminal stage.** Before the family challengers, the stage surveys the search winner's tree once and offers every site's other alternatives, largest estimated stake first, each formed, admitted and scored like a challenger and kept only on a strict whole-artifact win. When two or more sites apply an encoding, the first move puts every site back to its literal at once: decoders share text a codec matches across tables, so one site at a time cannot leave a state where several pay together (the families' `other-objective-seed` is the same whole move). The choices have their own budget, `terminal_choices` in the receipt (the challengers' ladder, counted apart so a table never displaces a family challenger calibrated on its own schedule). `--explain` lists every choice trial and each delivered site with its seed.
- **Data tables (M9.8; `js/tables.rs`).** Any root `let` whose value is a tree of constant data is a site: the literal; the front-coded string table (now counting UTF-16 units, so an astral prefix decodes exactly); **columns** (a schema derived from the table: collections become key, count and item columns; arrays of one length and objects of one key sequence are transposed; array-index keys are delta-coded; a number column whose values are all `n/10^p` is spelled as those integers; strings are joined; values of mixed kinds keep their literals in a column) with a decoder specialized to the schema; and a **value dictionary** over the columns. `MIN_ENTRIES`/`MAX_SHARE` are gone: the seed is the largest positive raw estimate, else the literal, and every objective can keep any alternative. Exactness is the legality: the same insertion order (index keys enumerate first either way), `Object.is` values, prototypes and plain data properties; no `__proto__` or duplicate key, no non-finite number, no observed literal; module root and pristine builtins only. Every encoding is decoded in Rust with the decoder's semantics and compared with the literal before it is offered.
- **L2 spellings.** A string key that is a canonical integer of at most 2^53 − 1 prints as a number in object literals, member accesses (`o[32]`, and `o[-1]` where a sign is legal) and a dynamic import's members (Closure's `isSimpleNumber` set). The lexer reads `.5`, `.25e-2`, `1e3`, `15e-5` (floats) and `0x1F` (an int under the signed 32-bit rule).
- Prior art (rule 7): Closure `closure-compiler@0da58e1 CodeGenerator.java:1596-1622` and `AliasStrings.java`; Terser `terser@8fa44c8 lib/output.js:2245`, `lib/compress/index.js:3580`, `lib/compress/common.js:170` (`best_of_expression`, a local printed-size judge); esbuild `esbuild@f6058f8 internal/js_ast/js_ast_helpers.go:2684`, `js_parser.go:14605,14993`; Oxc `oxc@591966d oxc_minifier/src/peephole/substitute_alternate_syntax.rs:1389`, `convert_to_dotted_properties.rs:17`; old route `d362338f:src/decision_registry.rs` (whole-program flags, no sites). **No competitor re-encodes constant data**, and none has a codec to judge an encoding.

**Evidence (binary `data-7`, `6d307b5f…`, against `merge-i`).**
- **Unit tests:** 1,519 pass (lib), the binaries' tests pass. New: `js/tables_tests.rs` forms six table shapes under every alternative and compares Node's observations with the literal's own run; `build_terminal_tests` delivers `tests/cases/data_tables` under all three objectives; the choice schedule and the numeric-key spelling have their own tests.
- **Case runner:** no failure outside the ledger. The one compared artifact that changed is the numeric-key case (−9 Brotli per production lane); every lane total fell or held. The new `data_tables` case (Node's own run as its oracle) delivers a different assignment under each objective, against `merge-i`: Brotli 2,243 → 2,108, gzip 3,142 → 2,682, raw 12,607 → 7,975.
- **Port suites** (13 ports, `--patches none`, plus motionlil from its release commit `0eeca0c`): failing sets identical to `merge-i` in every port. motionlil's current head (`69cd2cf`) fails to build on both binaries (a duplicate type declaration, M4.1); the four family site tests that compare committed receipts fail on both.
- **Sizes, `dist/` against `merge-i`:** markedlil, zodlil, posthoglil, motionlil, unifiedlil, mdast-util-to-hastlil and remark-rehypelil byte-identical. katexlil's compiled `katex.raw.js` +26 Brotli (stitched `katex.esm.js` +11), jquerylil `jquery.raw.js` +3, micromarklil `micromark.closed.js` −21 and `micromark.raw.js` −6, react-markdownlil `react-markdown.raw.js` +29 (`closed.js` −46), from-markdown and remark-parse ±25. Over all 13 ports' JavaScript files: +387 Brotli of 1,843,355 (+0.02%), inside the fleet noise law (appendix B). Decomposed on katexlil: the numeric-key spelling alone lands +26 (same search recipe, a different family path); on jquerylil it moves the search to another recipe whose winner is 2 bytes smaller and whose terminal path ends 3 larger.
- **katexlil with its font metrics in LilScript** (patch `~/lilscript-work/portwork/katex-data/katexlil-font-metrics-in-lilscript.patch`, no stitching; scratch build): `katex.esm.js` 60,281 Brotli, 72,187 gzip, 261,313 raw, against the shipped 62,704 / 75,914 / 274,680: −2,423 Brotli. Against the bars: Terser 63,044 (−2,763, −4.4%: a win under D4) and the CDN's `katex.min.js` 62,686 (−2,405, −3.8%: a win, where it was a tie). The table seeds its dictionary (raw estimate −19,855) and the codec keeps columns of ×1e5 integers with delta keys (−930 more); every shipped JavaScript file of `dist/` falls, −3,077 Brotli over 25. `npm test`: 21/21 node:test (browser parity 30/30) and the official Jest suite 1,230/1,230.
- **Compile time (level 13, three alternating pairs, main entries, medians):** markedlil 1.15→1.20 s, zodlil 2.07→2.01 s, katexlil 10.4→15.6 s, jquerylil 34.3→30.8 s, posthoglil 0.37→0.37 s, motionlil 16.4→16.4 s, micromarklil 2.04→2.27 s; katexlil with its data 12.1→19.0 s. The cost is the choice trials' exact codec scores (katexlil offers 13 alternatives on six sites and its configuration asks for every one).

**Open.**
- The estimator is raw bytes under every objective. A small site's seed moves the search like any tree rewrite on every emission (the terminal challenger law), so ports without a large table move by tens of bytes either way. A per-objective estimator (M9.4) seeds codec objectives better.
- Joint moves beyond "every site canonical" are not explored; decoders are declared at the root's start in the first module, as the string tables were (code splitting places them per chunk in M3.3); decoders become LilScript prelude code in M8.5.
- The numeric-key spelling is a rule, as the owner asked, not a codec choice.
- M9.1's rest: the product, record and string families and function layout still run their own enumerations.
- katexlil's patch keeps `src/fontMetricsData.js` for `scripts/audit-parity.mjs` and `scripts/attribute-core.mjs`, which still read it.

### M4.1 nominal identity (branch `m4-nominal`, 2026-09-27)

**What landed.**
- **Identities.** `NominalId` names every nominal kind: struct, class (with extern classes and objects) and enum, tagged by kind. `Type::Class`, `Type::ClassInstance` and `Type::Enum` carry a `NominalType { identity, name }` that compares by identity; the name is display data. The checker's class and enum registries are vectors indexed by id.
- **Per-module scopes.** Each module has one type scope, `type_bindings` (name → id), filled by its declarations and its type imports, aliases included. Two modules' private classes, extern classes or enums of one name are two identities. One scope still refuses a second declaration. A mismatch names both declarations ("distinct class declarations"). `language-v0.1.md`'s Modules section states the rule.
- **One checker entry.** `check::analyze` checks a single source as a module graph of one module, through `analyze_modules_in`. The phases hand typed products to each other: graph → declarations (ids, type imports, published classes) → schemas (enums, structs, classes, hierarchies, observed classes) → signatures (functions, typed bindings, value interfaces, `import()`) → bodies, in initialization order. An `auto` export is published after its module's bodies and is refused only when another module imports it.
- **The Program IR.**
  - `ClassDefinition` holds `identity`, `module`, `base: Option<NominalId>`, `observed`, `value`, `published` and `prototype`. `Program::class(id)` looks classes up by identity, and `EnumDefinition` has an `identity`.
  - Class field accesses are `Place::ClassField { receiver, field: FieldRef { nominal, slot } }`: the declaring class and its flattened slot. Instances are `AllocationKind::Instance { class, keys }`.
  - Formation, native, effects, demand, verify and lint read these. Formation prints the field's name, which is display and ABI data.
  - `src/program/nominal_identity_tests.rs` fails if a name-keyed class or enum lookup returns. It checks 22 spellings, ignoring whitespace and comments. Run against the base commit, it finds 40 such lookups.
- **`export constructor`.** Some classes stay JavaScript classes (the checker fact `ClassInfo::observed`, which generalizes host-derived classes): a published class, its internal bases, and every class that extends an observed class. For these:
  - the class's value binding holds its constructor unit (`UnitData::constructor_of`);
  - `new` is `ConstructClass`, and `super(...)` is `SuperConstruct`;
  - a class without `init` gets JavaScript's implicit constructor;
  - a published chain gets prototype methods that call the static bodies.

  Published constructors and prototype methods are interface escapes in the call graph. Their parameters are not typed-defined, so they keep their defaults. Natively, these classes allocate and run their constructor units. One name may export both a class's type and its constructor. A generic kept class is refused.
- **Prior art** (rule 7):
  - Closure `0da58e1`: `JSTypeRegistry.java:218,1133-1136,1508-1526` keys named types by (scope root, name), so each module body is its own row. `EqualityChecker.java:229-245` compares resolved nominal types by reference.
  - esbuild `f6058f8`: `internal/ast/ast.go:374-388` defines `Ref{SourceIndex, InnerIndex}`. `internal/renamer/renamer.go:580-628` names the second `Node` only when printing.
  - Oxc `591966d`: `crates/oxc_semantic/src/scoping.rs:20,281-298` has per-scope name → `SymbolId` maps over flat arrays.
  - Rolldown `5c676e5`: `crates/rolldown_common/src/types/symbol_ref.rs:8-13` defines `SymbolRef{owner, symbol}`.
  - Old-route prior art: `d362338f:src/lower.rs:413-470`, the constructor-export rules.

**Evidence (binary `~/lilscript-work/bin/nominal-2`).**
- **Unit tests:** 1,510 lib tests pass (9 ignored, as on the base) and 21 binary tests pass. `cargo fmt --check` and `check-doc-links` pass.
- **Case runner:** 382 cases × 18 lanes, with no failure outside the ledger.
  - The ledger drops from 10 entries to 9. The M4.1 entry's 6 cases now pass: 42 case-lanes went from refused to pass.
  - Six new cases in `tests/cases/nominal` cover same-named private classes and enums in three modules (JS and C), an extern class shadowed by a private class, class fields across modules through an imported and a renamed class, a published chain across modules (JS and C), and the `export constructor` ES boundary (a module probe).
  - Against the merge binary `merge-i`, one artifact per JavaScript lane changed, `irjs-an_internal_class_extending_a_host_class_is_a_real_subclass`. Its constructor's binding is now the class's own (`Problem`, previously a synthetic `init`). In production that is −1 Brotli and −1 gzip in both the script and module lanes. The unmangled formation-only lanes spell the name, so they grow +6 raw and +1 Brotli. C lanes are byte-identical.
- **Reference ports** (`scripts/ports.mjs --patches none`): all seven are green, and every delivered file is byte-identical to `merge-i`:

  | Port | Files | Brotli |
  |---|---|---|
  | markedlil | 7 | 65,061 |
  | zodlil | 7 | 61,698 |
  | katexlil | 26 | 596,424 |
  | jquerylil | 5 | 127,208 |
  | posthoglil | 6 | 33,786 |
  | micromarklil | 9 | 187,217 |
  | motionlil (built from `portwork/motionlil-reint`) | 629 | 237,216 |

  - motionlil `main` needs `finer/port-migrations/motionlil.patch`. With the patch, its 22 files are also byte-identical (280,587 Brotli). `main` alone fails on both binaries: on `merge-i` with the duplicate-type refusal, and after it with an unimported `Math`.
  - katexlil's browser-performance guard failed once on `merge-i` while two runs shared the host. It passed when `merge-i` was rerun alone.
- **Port renames reverted** (patches in `~/lilscript-work/portwork/nominal/`; no port repository was changed):
  - The "16 port class renames" are 16 class renames in one port, motionlil, made by the 013 batch-4 patch. Reverting all 16 on `main` with the patch builds green (9/9 suites), and the 22 files are byte-identical. `merge-i` refuses the reverted source with duplicate type declaration `GroupAnimationWithThen`.
  - On the integrated rewrite (`motionlil-reint`), 7 renames remain revertible and are reverted: 629/629 files identical, 22/22 suites green. `JSAnimationFull` keeps its name, because its file imports the extern `JSAnimation`.
  - micromarklil renamed the extern views `Point`/`Token` to `TokenizerPoint`/`TokenizerToken` and `AttentionPoint`/`AttentionToken`. The revert builds green (1,963 tests) and its 9 files are identical. `merge-i` refuses it with duplicate type declaration `Point`.
  - mdast-util-from-markdownlil, remark-parselil and react-markdownlil carry copies of the same micromark sources, with the same renamed views; the same patch applies to each copy. unifiedlil's `UnifiedPresetView` and `UnifiedFileView` are new names, not renames.
  - katexlil and zodlil renamed nothing.

**Open.**
- M4.2: the `$js` type parameter; interned types without source lifetimes. `NominalType` carries a `&'src str` display name, so `Type` grew to at most 56 bytes.
- M4.3: checker facts transported.
- M4.4: node ids. Type scopes are still name maps per module, and spans still key checker facts.
- M4.5: contracts at check time.
- M4.6: the operation catalog.
- Gaps left by this batch:
  - kept generic classes;
  - D2 adapters for a published constructor's struct parameters;
  - importing a name that exports both a type and a constructor binds only the type;
  - field facts (M6.7), property renaming (M9.6) and layout choice (M9.7) now have `FieldRef` and `Instance { class }` to key on.

### The tools, first part: M2.13 (branch `m2-tools`, 2026-09-27)

**What landed.**
- **The generic corpus ratchet** (`scripts/ratchet.mjs`; [testing.md](../testing.md#the-generic-corpus-ratchet)). `comparison/cases` (54 canonical, 570 catalog), `comparison/apps` and `comparison/algorithms` compile in three objective lanes, run against their oracles (the reference programs' stdout; the algorithms' ordered host accesses too) and are measured with the canonical codec. Each artifact is compared in its own metric with three bars fixed in `tests/ratchet/bars.json`: the smallest valid competitor (Terser with and without its safe property lane, Oxc, esbuild; the algorithms' bundler lanes), Closure ADVANCED (apps and algorithms), and the frozen old route. A run fails when a loss count or any item's loss grows, a passing lane fails, a loss or failure is not covered by an owned entry of `tests/ratchet/ledger.json`, or the bars changed under `tests/ratchet/baseline.json`. Improvements are reported, and `--update-baseline` tightens the baseline and refuses while anything blocks. The gate needs only Node, the compiler and the codec (bars, old-route configurations and reference host traces are committed); it runs in about 40 s at `--jobs 3` and in CI's gate job. The competitor recipes moved to `comparison/cases/recipes.mjs`, shared by the paired-case harness and the ratchet.
- **Harness conformance (BC12).** The six case configurations carried six retired keys (`identical_function_folding`, `function_subsumption`, `pool_numeric_literals`, `stable_local_names`, `local_name_reserve`, `mangle.exports`); they are removed, and every configuration's policy fingerprint is unchanged (the old route's bars keep the originals, `tests/ratchet/old-route/`). Five harvested regression configurations that held only `mangle.exports` or `mangle.extern_fields` are deleted. The 38 failing catalog variants: the 27 that called name-keyed host helpers the old route gave bodies to (`mathMax`, `objectHasOwn`, `isFunctionValue`, `isWindowValue`, `windowSelf`, `defineConfigurable`, `defineIterator`, `typeOf`), and the canonical `host/math-max`, are rewritten to declared host bindings (`extern class` views of `Math`, `Object` and `Reflect`; `extern JsValue globalThis`) or to the predicate the reference program spells, with the JavaScript oracles unchanged; the other eleven are ledgered: ten on record spread (M10.8), one on `??=` on a place (M10.9). The review's "33 on record spread" counted case-lanes.
- **NO3** (`src/no_library_knowledge_tests.rs`, `cargo test --lib`): 30 mentions of ports and upstream libraries in non-test `src/`, in 22 (file, library) entries of `tests/no3-allowlist.json`, each with its reason and owner (M3.5, M7.5a, M8.2 A1, M8.7, M9.3, M9.8, M9.10). A new mention fails; a removed one must lower its entry, so the ledger only shrinks. The review's "about 35" used a wider word list; this one counts English words (`marked`, `motion`, `remark`, `unified`) only in their library spellings. Thresholds without a policy or estimator source are not checked lexically yet (NO3's second half stays with M8.7).
- **NO4**: the idiom debt ledger `tests/idiom-debt.json` with its schema `tests/idiom-debt.schema.json` and a validator in the runner tests. It starts empty.
- **Hygiene.** `rayon` is removed (declared, never used; 46 lockfile lines); the `Cargo.toml` comment no longer claims an admission parse that did not exist; `docs/current-status.md` describes version 1.
- **Prior art** (rule 7): Oxc `591966d` `tasks/minsize/src/lib.rs:49,144-146` keeps minified sizes against esbuild in a checked-in snapshot, reviewed by diff, not blocking; the paired-case and algorithm harnesses (`comparison/cases/run.mjs`, `comparison/algorithms/run.mjs`) gate every case absolutely and so failed on every run (495 of 624 cases), which is why they reported instead of blocking. The ratchet keeps their recipes and oracles and blocks on growth.

**Evidence.** Baseline `head-d1d48c4c` (SHA-256 `47048e41…`); the branch's own build reproduces every item's sizes. Brotli, ours against each bar where both exist:

| Corpus | Passing | vs smallest competitor | vs Closure ADVANCED | vs old route |
|---|---:|---|---|---|
| `comparison/cases` | 612 of 624 | 49,223 / 43,541, 439 lose | — | 49,126 / 33,656 over 611, 603 lose |
| `comparison/apps` | 7 | 945 / 870, 5 lose | 945 / 834, 7 lose | 945 / 558, 7 lose |
| `comparison/algorithms` | 11 | 3,250 / 3,039, 9 lose | 3,250 / 2,703, 11 lose | 3,250 / 2,305, 11 lose |

Loss counts (raw / gzip / Brotli): cases against the competitor 399 / 400 / 439 and against the old route 610 / 600 / 603; apps against the competitor 1 / 3 / 5, Closure 5 / 6 / 7 and the old route 7 / 7 / 7; algorithms against the competitor 5 / 8 / 9 and against Closure and the old route 11 in every metric. On the unchanged catalog the harness reproduces the review's standing exactly (canonical 9/6/37/2, 4,058 against 3,721; catalog 59/93/380/38, 42,743 against 37,689). The old route's apps total is 558, not the review's 571: the bars compile the corpus's own configurations (level 15, `always`); its algorithms total, 2,305, matches. The algorithms' Closure bar is the smaller of the harness's two Closure lanes (single file and module graph), 2,703 against the review's 2,756. The old route itself fails three lanes: one wrong output (`cases/catalog/number/edge-number-flow-n1nn0p75n4n3`, Brotli) and two refused by its own standards-parser admission; those lanes have no old-route bar.

### Where we started (2026-09-23)

**Two compilers share one binary.** `[compiler] backend` or `--backend` selects between them.

**The old route: `compiler.rs` → `lower.rs` → `ir.rs` → `optimizer.rs` → `codegen_ir_js.rs` → `js_peephole/`, plus `codegen_native.rs`.**
- It is about 149.5K lines with tests, 42% of `src/`, and holds 1,555 of the 3,092 unit tests.
- It re-emits the whole program 267–381 times per build and reasons over its own printed text.
- Its typed interprocedural optimizations are still ahead on small closed programs:

  | Measure (binary b80) | Old route | Remaining compiler |
  |---|---|---|
  | 72 census cases, Brotli, `strip_console=false` | 5,866 | 8,576 |
  | `comparison/cases`, Brotli wins/ties/losses against the smallest competitor | 53/1/0 | 9/6/38 (+1 refusal, 1 wrong output) |
  | `comparison/algorithms`, Brotli total | 2,305 | 3,345 |

- Its native C works under GCC 13 but not under Clang ≥ 16 (an undeclared `strdup`). It supports native `Record`/JSON and a user-facing C extern ABI, which the remaining compiler refuses.

**The remaining compiler: `compiler_service` → `semantic_program` → `structured_js`, plus `semantic_program/native*`.**
- It is 3–12× faster at equal effort with the search off (katexlil 1.0 s against 4.3 s), and about 100× cheaper per candidate.
- It has zero census miscompiles, and its native C is clean under ASan, UBSan and LSan.
- On the 13 goal boundaries (the six reference ports and the react-markdown family), patched scratch builds beat the strongest pinned bar on 11:
  - katexlil, markedlil, posthoglil and jquerylil beat theirs on both objectives. katexlil's Brotli margin, −374, is inside D4's strict-win threshold of 630.
  - The react-markdown family beats all seven bars (react-markdownlil browser: 27,248 against 31,082).
  - These numbers come from scratch builds with `finer/port-migrations/*.patch` applied. micromark's shipped file can be an esbuild reprint.
- **Open losses:**
  - motionlil: +8,612 Brotli. Its 49,644 is esbuild plus Terser over our output, not a compiler-written file.
  - zodlil's package: +16,396 Brotli as compiled, or +12,211 after esbuild's whitespace minifier.
  - zodlil's core: about 1.5–1.7K above the upstream slice.
- Its bytes come mostly from about 40 hand-ordered rewrites of the finished JavaScript tree. Typed facts barely reach them: every user call is an unknown effect, class field identity is erased at elaboration, and native gets none of these optimizations.
- **The migration's test harvest found real miscompiles here** (M1.9).

**Routing and ports.**
- The default was flipped to the remaining compiler on 2026-09-23 (`d362338f`). These still run the old route regardless:
  - the LSP, the playground and lint (on the old IR);
  - `--write-lock` effect summaries and `--print-dependencies`' module discovery;
  - every `lib.rs` compile function.
- No port pins a route. Committed port `dist/` files were last rebuilt 2026-08-29 to 09-04, all on the old route, and the Pages sites show them.

**Configuration.**
- About 55 configuration keys are consumed only by the old route and about 12 more are translated but read by nothing. Only three of them warn.
- Some port configs will be refused by an honest resolver: mobxlil's `priority = "realistic-performance-first"`, and cnlil's `public_aggregate_abi = "positional"`.

**Status.** CI has not been green since 2026-08-25. 0 of 14 old milestones are verified.

---

## Rules for every phase

1. **One compiler.** Nothing new may depend on the old route. Its code is prior art: read it, never link or port it line by line.
   - The frozen reference binary is `~/lilscript-work/bin/reference-2026-09-23/lilscript`: binary b80, SHA-256 `df8595…`. It was built at 21:00 on 2026-09-23 from the pre-flip working tree of `d362338f`, so its default route is the old one. It is used only to measure "the first bar".
   - The pre-migration binary of the one compiler is `~/lilscript-work/bin/pre-m1/lilscript`, built from `0c17237e`. Its compile times are frozen once (M2.14), because later sources will not compile on it.
2. **By design.** Every change is generic and owned, and a fact is computed once, by its owner.
   - A fact reaches its consumers through its publication channel (program tables, or tree annotations after M5.2). **The re-derivation it replaces is deleted in the same batch its consumers switch.** This rule is not waived again: a fact counts as landed only when its channel reaches every target layer that decides with it and its re-derivations are deleted (architecture L16). Until then the phase table says "computed, not consumed".
   - **Replace, then delete** (L20). Nothing is deleted before its replacement lands and passes the gates; a replaced pass runs as a marked transitional rule (M5.3a) with its deleting task.
   - Formation never emits a shape a later pass undoes.
   - No thresholds tuned on ports: a choice goes to the codec, a structural bound comes from versioned schedule data calibrated on the calibration corpus.
3. **Gates. Every gate blocks** (L21): a failing gate stops the merge. A known loss is carried only by an expected-failure row with an owner task, written before the batch; no gate has an "unless the ledger row says why" clause.

   | Gate | Rule |
   |---|---|
   | Correctness | Unit tests, the case runner and the reference port suites. Byte identity is evidence, not a gate |
   | Size, per batch | No Brotli degradation at batch end on the case runner's corpora and the reference ports beyond the seeded re-mangling band (NO6) |
   | Size, per phase | A ratchet over the scoreboard frozen at the phase's start (M2.8, re-frozen under M2.10's bars): no cell that is a win or a tie becomes a loss, and no ledgered losing cell grows beyond its noise band. Every losing cell is a ledger row with an owner task (architecture §18.4). "No losing cell" is M12.3's exit, not an earlier phase's |
   | Generic corpus | From M2.13: `comparison/cases` (54 canonical, 570 catalog), `comparison/apps` and `comparison/algorithms` run per change as a ratchet: the loss count against the smallest competitor, and against Closure ADVANCED on apps and algorithms, never rises; each new rule names the cases it wins |
   | Exact rules | A new rule passes L3: it removes operations, and its Brotli, gzip and raw deltas are non-positive on every calibration-corpus program under seeded re-mangling, with the sign distribution in its registry entry. Its summed case-corpus effect is at most zero within the seeded band; per-port effects are judged under the noise law. A rule that loses beyond a port's band becomes a choice with prior "apply" in the next schedule version |
   | Codec-dependent shapes | Ship as choice families under the budget model (M3.5). No new codec-judged family lands before M3.5 |
   | Compile time | Per change and batch (architecture §13.7): (a) added default-level WU buys bytes at or above B9's exchange rate and no level exceeds its ceiling (from M3.5); (b) moves judged at the default level, reported beside (a); (c) process CPU time on the case runner and the seven main entries against the pinned previous binary, alternating pairs, median of at least three, fails above +15% (from now). Per phase: the targets of architecture §13.2 due at that phase, against the frozen baseline (M2.14). Before each of M5, M6 and M7: a projected WU table for katexlil and jquerylil of the facts and rules it adds |
   | Runtime | Per change: the structural counters (architecture §16.5). Per batch, from M2.12: the reference perf lanes in a quiet window, this batch's artifact against the previous binary's; worse by more than the controls' spread δ fails. Per phase: a ratchet over the runtime ledger (architecture §16.6): no new miss, no ledgered ratio worse beyond its interval. M12's exit: no miss |
   | Native | Per phase, from M11: native at least as fast as the JS output under Node on the native perf corpus median and on each named portable core |

4. **Pass rule (D4).**
   - At or below the bar is the floor ("tie"). A **win** is at least the cell's threshold below it: max(1% of the bar, the cell's seeded noise band) once the owner answers Y6; until then `max(100 bytes, 1%)`.
   - Each cell's bar is built by the benchmark contract (architecture §18, M2.10): generated from the declared surface, the minimum over a pinned recipe grid (SWC and the Rolldown bundler included), matched in assumptions (including the language guarantees a port relies on) and reflection, and passing the same oracle.
   - Verdicts are on shipped artifacts per codec (BC10). Both the open world (the developer-facing API preserved, mangling fairness contract) and the closed world must pass, and so must the consumer lanes of library-world ports, from M3.3c's exit.
5. **Batches.** 4–8 changes per build, one verification pass per batch and one ledger row per batch. Builds and tests run on this host only. Each ledger row pre-registers its predicted per-port deltas (NO5), its work-unit cost, and its expected runtime effect.
6. **Evidence.** A result counts only when the delivered file is compiler-written, with no post-minifier. The receipt pins the binary, source, patch and dependency identities.
7. **Prior art.** Each batch's ledger row cites the competitor source read (`repo@commit file:line`, from `~/competitors`) and the old-route prior art read (owner, 2026-09-01). A new exact rule cites the competitor rule it generalizes; one with no counterpart must win on two unrelated ports or cases, or on the held-out set (NO8).
8. **Verification ladder.** The fleet runs only at phase end: the owner's "don't build it all" (09-04) holds per change, and "no accepted losses" (09-22) holds as the ratchet per phase and absolutely at M12.

   | When | What runs |
   |---|---|
   | Per change | Unit tests, the case runner, probelil, the generic corpus ratchet, the micro size and perf gates, the structural perf counters, CPU-time pairs |
   | Per batch | The reference port suites (katexlil, markedlil, zodlil, jquerylil, posthoglil, motionlil, micromarklil and its family); the reference perf lanes in a quiet window after the suites (no concurrent compiles, load recorded, a run above the load threshold repeated), blocking; work units per port |
   | Per phase | The fleet, the held-out set, the benchmark contract's cells, the runtime ledger ratchet, wall-clock compile time per entry and per package build |

   A change worth under about 400 fleet bytes is judged on micro gates, seeded re-mangling or by the terminal slot, never by a fleet A/B.
9. **Order.** Language before recognizers: a fact the language can state is not recovered by a new analysis or pattern (L14). Rules before choices: the canonical base comes from exact rules before a choice family varies it (L15). The budget model (M3.5) precedes every new codec-judged family. Replace, then delete (L20). The critical path never waits on an owner ruling: work that needs one is split so the rest proceeds.
10. **Not overfitting.** The held-out libraries (M2.11) are never diagnosed while held out, and a held-out loss follows NO1a (architecture §18.3). Priors, margins, the tariff and the exchange rate are calibrated on the case corpus and the calibration corpus, never on ports (NO9). No port or upstream identifier, and no threshold without a policy or estimator source, may appear in `src/` beyond the NO3 allowlist ledger, which M8.7 empties. A port rewrite needed only because the compiler mishandles the idiomatic form is recorded as idiom debt, with the idiomatic form as a regression case (NO4).
11. **Stability.** Ids are frozen. The plan changes by landing records and by re-ordering "Next action" with a ledger note; design changes go through an architecture amendment (§22).

---

## Phases

| Phase | Name | Depends on | State |
|---|---|---|---|
| M0 | Record and freeze | — | done 2026-09-23 |
| M1 | One compiler: the old route leaves the product | M0 | done 2026-09-24 |
| M2 | Verification ladder, baseline, interim release and the benchmark contract | M1.3 (runs alongside M1) | active: M2.1, M2.2 and M2.6 done; M2.8 and M2.9 done for the 13 goal ports (motionlil released 2026-09-27 on its newer origin; its delivered files are still esbuild + Terser, M12.2); katexlil re-released 2026-09-27 with its font metrics in LilScript (60,281 Brotli); M2.13 landed 2026-09-27; M2.10–M2.12, M2.14 and M2.15 new |
| M3 | Honest configuration, one public API, delivery contract, the budget model | M1; M3.3c needs M5.2, M6.2 and M9.1; M3.8b needs M5.2 and M3.3c | active: M3.3a/b in flight (parallel session) |
| M4 | Checker identities and checker-owned facts | M1 | active: M4.1 landed; M4.2 in language slice 1; M4.6 in the core; M4.3 and M4.4 before M5.2 |
| M5 | The machinery: edit kernel, annotations, scheduler, monotone selection | M1, M4.1; M5.1 after M2.5; M5.2 after M4.3 and M4.4; M5.3a after M5.2 and M2.5; M5.3b interleaves with M7, M9.7 and M10.4/M10.7; M5.7 after M5.3a | active: M5.4 landed (monotone across levels: closed by M3.5's §9.6 procedure) |
| M6 | The fact spine | M6.4a after M5.1; M6.4b after R1 and R11 (M10.12, M10.9); M6.6–M6.8 after M5.2; M6.7 after M10.14's checker half | active: M6.1–M6.3 and M6.5 **computed, not consumed** (rule 2) until M5.2 deletes their re-derivations |
| M7 | Program rules: the floor | the floor slice (M7.3, M7.5a, M7.8a) after M5.1 and M6.4a; the rest after M5.2/M5.3a and M6 | active: M7.2 landed |
| M8 | Canonical formation and the pure printer | A1: nothing; A2: M2.5, M4.6, M5.2; the rest of M8.2 by replacement (M7.3, M9.7, M10.4/M10.7); M8.3 after M5.2, its `\|0`/`++` half after M6.4b | next: batch A1 |
| M9 | One choice system: choices, naming, layouts and data | M3.5; M5.2; M8.3 for spelling families; M7.5 for inline-or-share; M10.3 before M9.8's rest | active: M9.1's first slice, M9.2, M9.3's first family and M9.8 landed; no new family before M3.5 |
| M10 | The final language (R1–R18) | slice 1: M4.2; the core: nothing (M10.12 after Y1); the rest after the core | waiting: slice 1 and the core are steps 7 and 8 |
| M11 | Native: the whole language, directly executable | M11.2 in batch A1; M11.5 after M7; M11.11 after M4.5 | waiting |
| M12 | Qualification and publication | continuous; M2.10 (bars); closes last | waiting; M12.2 for motionlil in flight with M3.3 |

### M0 Record and freeze: done 2026-09-23

- Batches 29–32 and the default flip committed (`d362338f`).
- The owner briefs recorded verbatim.
- The architecture written; `docs/compiler-design.md` became a pointer to it.
- The plan rewritten; the history moved to `record-2026-09.md`.
- The reference binary frozen with its SHA-256; the pre-M1 binary preserved.
- Migration work happens on branch `one-compiler`, in worktrees under `~/lilscript-work/wt/`, so parallel sessions on the main tree are not disturbed.

### M1 One compiler: the old route leaves the product

**Goal.** No route switch, no old-route code, no old-route tests. `--backend`, `[compiler] backend` and `CompilerBackend` are gone, and every tool compiles through the one compiler.

| Task | Content |
|---|---|
| M1.1 Move shared pieces | Four pieces move and one import is re-pointed:<ul><li>The manifest moves to the JS delivery code; each file is measured once with the canonical codec.</li><li>Diagnostics move to `diagnostics.rs`.</li><li>The codec helpers move to `compression.rs`.</li><li>`reference_parameter_span` moves into the checker, so the interpreter no longer calls `lower.rs`.</li><li>`typed_array.rs` imports `primitive::Intrinsic`.</li></ul>Tactic defaults move into the `TacticId` specs. There is one contract builder, and `public_function_spelling` is deleted: it is always `None` on the one compiler. Output is byte-identical |
| M1.2 Tools | <ul><li>The LSP uses a check-only session.</li><li>The playground compiles with `strip_console = false` and gets a smoke test on its printed output.</li><li>Lint's AST rules move onto the module graph and checker the build uses; its IR rules move onto the Program IR. The IR rules now see the program *before* program rules, and move after M7 when those land.</li><li>`performance/aggregate-escape` returns with the escape fact (M6.6).</li><li>`LintRuleContext` exposes the program (a breaking change, accepted).</li><li>`--write-lock` stops writing effect summaries.</li></ul> |
| M1.3 CLI | <ul><li>`--backend` and its dispatch are deleted; a hidden flag fails with "there is one compiler". `--profile-template` is deleted.</li><li>`--explain human` is readable.</li><li>One target-to-request function serves both `--print-policy` and the build.</li><li>`--target all` writes every chunk.</li><li>Native uses one strict driver.</li><li>`--print-dependencies` moves onto the one compiler's discovery, host modules included.</li><li>Lilpack and Vite keep `--mode`, `--delegate-bundling` and `-o`.</li><li>The repository tools that pass `--backend` (`finer/tools/semantic-census.mjs`, `semantic-port-tests.mjs`, `semantic-service-baseline.mjs`) stop doing so.</li></ul> |
| M1.4 Configuration | <ul><li>`[compiler] backend` is refused.</li><li>One retired-key table is applied before strict deserialization. Each old-route-only or inert key gets a printed "no effect in this compiler" warning or a refusal.</li><li>`priority ≠ size-first`, `[policy.constraints]`, `public_aggregate_abi = "positional"` and `for_of_specialize_family` are refused at load with actionable messages.</li><li>The old option derivations, the second contract builder, the legacy ABI manifest and contract fields with no reader are deleted.</li><li>`schema.md` is regenerated and `configuration.md` updated.</li></ul> |
| M1.5 Tests | <ul><li>**Harvested:** 288 regression cases from the old route's executing tests (`tests/cases/regressions/`, with per-prefix manifests: irjs 202, compiler 65, opt 21). Each carries expected stdout derived from the test's own assertions, never from a compiler.</li><li>**Re-pointed:** the cross-route tests (`structured_js/tests.rs`, `semantic/narrowing_input_tests.rs`, `semantic.rs`, `compilation_facts_tests.rs`, `parser_admission_tests.rs:205`) go to the one compiler or the interpreter; the interpreter's `for_of_family` test is deleted.</li><li>**Mined:** `migration/target-tree` for live-bug cases, then retired.</li><li>**Kept:** keyword-spacing cases, ported to the `structured_js` printer.</li><li>**Rewritten:** `comparison/cases/.../host/math-max`, with a declared host binding.</li></ul> |
| M1.6 Delete | <ul><li>The old route: `compiler.rs` (after M1.1), `lower.rs`, `ir.rs`, `optimizer.rs`, `value_analysis.rs`, `compress_passes.rs`, `codegen_ir_js.rs`, `codegen_js.rs`, `codegen_native.rs`, `js_peephole/`, `decision_registry.rs`, `artifact_memo.rs`, `profile.rs`, `for_of_family.rs`, `compiler_rest_capture_tests.rs`.</li><li>`js_externs.rs`, whose platform-name data moves to a neutral data module.</li><li>The module linker and the AST's linker fields, with `analyze_program`'s linked-program path edited.</li><li>Package effect summaries, `timing.rs`'s old buckets and the pass-ablation benchmarks.</li><li>The test-only annotated-tree experiment in `structured_js`, including `selection::select`.</li><li>The fixed two-file resource cut.</li><li>`search_pairs.rs`, off by default, and the `LILSCRIPT_DEBUG_HELPERS` read in the search loop.</li><li>`examples/structured-slice.rs`, and the checker's dead `EscapeState`.</li></ul>**Kept:** `interpreter.rs`, `primitive.rs`, `typed_array.rs`, `literal.rs`, `js_string.rs`, `js_regex.rs`, `js_syntax_target.rs`, `compression.rs`, `stable_hash.rs`, `scalar_transfer.rs` |
| M1.7 Names and docs | <ul><li>**Renamed by role:** `semantic.rs` and `semantic/` → `check/`; `semantic_program/` → `program/`; `javascript*.rs` and `structured_js/` → `js/`; `native*.rs` → `native/`; `compiler_service` → `build`.</li><li>The rename lands as one announced commit when no session holds uncommitted changes to the renamed files.</li><li>**Docs:** every doc a grep finds describing the old pipeline is rewritten or retired. That includes `language-v0.1.md`, `configuration.md`, `current-status.md`, `optimization-coverage.md`, `modules-and-delivery.md`, `web-platform.md`, `differential-testing.md`, `knowledge/compilation/*`, `knowledge/verification/config-matrix.md`, `finer/objective.md` and `finer/status.md`.</li><li>`language-v0.1.md` states which clauses wait for M10: `inline for`, `@pool`, and string semantics (UTF-16 code units).</li></ul> |
| M1.8 Verify and gate the deletion | Before M1.6, **every maintained port (27) builds and runs its suites on the pre-M1 one-compiler route**. Each failure becomes an owned entry in the expected-failure ledger (M2.6). After M1.6, the same corpus is rerun on the new binary, and byte identity is compared against the pre-M1 binary: census × 3 lanes, `comparison/cases` × 3 codecs, probelil and the 13 goal boundaries |
| M1.9 Correctness debts found by the harvest | Fixed as bugs (D3), with the harvested cases as their reproductions:<ul><li>`async int f(){return 1;}` is folded to `1`, breaking `.then`;</li><li>a field default (`new Map`) is evaluated before a constructor argument (D3.4 order);</li><li>`JsValue == 0` is lowered to `===`, while the language spec says dynamic equality coerces. The fix follows the spec until M10.9 changes it by owner ruling;</li><li>struct values passed to an `extern` are refused;</li><li>a wrapper's inferred name disagrees with the spec, in one of two sibling cases;</li><li>under the raw objective, a module whose inlined callback assigns a root binding fails inside the compiler with "assignment requires a reference" (`compiler-drops_unread_scheduler_store_and_host_value`, found by the case runner).</li></ul>The first three are fixed (`93967ef0`, with a service-level regression test) |

**What M1 does not restore.** The old route had these; each is owned.

| Capability | State after M1 | Owner |
|---|---|---|
| The `pure` contract check (a diagnostic) | Absent (already absent on the default route) | M6.3 |
| Removal of discarded pure calls | Absent | M7.2 |
| Native `Record<T>`/JSON, and the user-facing C extern ABI | Refused; the `scripts/verify.sh` extern-ABI step and the differential's native `Record` lane become ledgered expected failures | M11.3, M11.4 |
| Native stack and region storage | Absent | M11.5 |
| Name-keyed host helpers: extern names given built-in JS bodies, such as jQuery's `isWindowValue`, lil-solidjs's `DOM_RECONCILE`, `objectHasOwn` and `mathMax` | **Dropped by design.** Ports declare host modules or use `JS.*` and the catalog (M10.2); jquerylil, motionlil, monacolil and lil-solidjs are checked in M1.8 | — |
| Same-named private classes in two modules; `export constructor` | Landed in M4.1 (2026-09-27): per-module nominal scopes; published classes stay JavaScript classes | M4.1 |
| Unrolling of `inline for`; `@pool` | Ignored | M10.11 |
| Record spread construction | Refused | M10 decision (M10.8) |
| `public_aggregate_abi = "positional"`, `function_scope`, `idiom_directed_naming`, `[mangle] properties`, profile-guided optimization | Refused or no effect (warned) | Positional: refused (D2). Module wrapper: M3.1 `format`. Naming: M9.5. Typed property renaming: M9.6 |
| preserve-modules chunks and lazy `import()` chunks | Already broken on the one compiler (reproduced: `chunks: []`) | M3.3 |
| Source maps | Not supported (the parallel `codex/source-maps` branches are built on the old route) | M8.6 |

**Exit, checked by script.**
- A grep over `src/`, the CLI help, configuration and current docs finds no route-selecting `backend`, and no "legacy route" or "semantic route" outside history files.
- Every binary (`lilscript`, `-lsp`, `-lint`, `-fmt`, `-codec`, `-differential`, `-playground`, `lilpack`) builds and uses the one compiler.
- `cargo test` passes on all targets.
- M1.8's corpus is byte-identical to the pre-M1 binary, or every difference is explained.
- The expected-failure ledger lists every red gate with its owner.

### M2 Verification ladder, baseline and interim release

| Task | Content |
|---|---|
| M2.1 Green CI | <ul><li>Fix `cargo fmt` and `examples/semantic-integrated.rs`.</li><li>One Linux job under about 15 minutes, with explicit steps: fmt, `cargo test --lib`, the case runner's production lanes, the codec contract, and the micro gates as a *reported* step until M7.</li><li>Publishing steps (web catalog, VS Code packaging, Playwright, Closure) leave the gating path.</li><li>probelil is vendored into the repository, or run from a pinned copy.</li><li>Competitor artifacts for the micro gates are cached and committed.</li><li>Steps owned by later phases (extern ABI, differential native lane) stay out until their owners land</li></ul> |
| M2.2 Case runner | <ul><li>One runner over `tests/cases`, the harvested regressions and the D3 clause cases, moved out of `d3_clause_tests.rs` so they run on shipped output.</li><li>Lanes: `{formation-only, production} × {brotli, gzip, raw} × {script, module, C}`, with per-target feature masks from day one.</li><li>The harvest's case conventions: `.host.js` preludes, `.module-probe.mjs`, multi-module folders and merged `.toml` keys.</li><li>Family-veto lanes are added after M3.2.</li><li>It replaces the development-mode census and the knob configs in `tests/config/`</li></ul> |
| M2.3 Oracles | Interpreter-generated `.out` where the interpreter covers the program. Coverage is measured by running it, not estimated. Blessing is refused when the interpreter disagrees. `print` is never stripped (decision owned here; M10.11 adds the `debug` class). **Open:** the default `strip_console = true` still deletes `print` (`src/config.rs:1044`), so a program with no configuration compiles to an empty file; batch A1 makes `print` a program effect and splits the key into `strip_debug` and `strip_console_calls` |
| M2.4 Interpreter extension | The reference interpreter gains structs, classes, enums, generics, Map/Set and a declared host model, feature by feature. Each feature lands before the M6/M7/M9 work that optimizes it. It stays independent of formation |
| M2.5 Admission parse | Every delivered JavaScript file is re-parsed by Oxc inside admission; its structural digest must match the printed tree (A5) |
| M2.6 Port runner and ledger | <ul><li>One versioned runner that records failing-test sets, diffs them against the **expected-failure ledger** (every entry has an owner task) and pins the compiler by digest.</li><li>It replaces `portgate.mjs`, `semantic-port-tests.mjs` and the unversioned `~/lilscript-work/tools` scripts.</li><li>It is checked into the repository</li></ul> |
| M2.7 Differential | The generator becomes type-directed with per-target masks. Its hand-pinned prologue shapes move to `tests/cases`. It enters the gating job only after this |
| M2.8 Baseline on one binary | <ul><li>Every maintained port's rewrite is committed to its own repository (M12.1 brought forward). The in-flight motion and zod rewrites in `~/lilscript-work/portwork/` land there too.</li><li>The fleet is built and its suites run on the post-M1 binary.</li><li>The scoreboard is frozen as the baseline that phase gates compare against, open and closed world</li></ul> |
| M2.9 Interim release | The owner's 2026-09-23 release request. Every port is rebuilt by the pinned post-M1 binary with no post-minifier; its Pages site is updated with sizes and compile times (`site/results.json`, `npm run check:site`); one report compares against the last release and against Terser, Oxc/Rolldown and esbuild. Ports that lose are published as losses, not hidden |
| M2.10 The benchmark contract | One repository tool, `scripts/bars.mjs`, with a pinned manifest and lockfile, builds every bar under the architecture's §18 (BC1–BC14): entries generated from the declared surface; Terser, **SWC**, Oxc minify, **the Rolldown bundler**, esbuild and upstream's minified file at pinned versions matching `~/competitors`; a recipe grid with the minimum over passing lanes; assumption-matched (BC5, including the language guarantees a port relies on) and reflection-matched (BC6) lanes; banner-free scoring; both sides through the same oracle (BC8); consumer lanes for library-world ports (BC9); closed-world cells (BC11); verdicts on shipped artifacts per codec (BC10) with robustness rows at Brotli q5 and a second gzip encoder; Terser's wall time on every port's input (for the level 1–4 target); one receipt per release that `site/results.json` renders. The per-port `measure-site` and `minify-lanes` scripts consume its receipt. Re-verdict on landing: remark-parselil gzip, unifiedlil, posthoglil main, katexlil's code lane, and every row of the closure ledger (architecture §18.4) |
| M2.11 Held-out libraries | The split of architecture §18.3 NO1: the development set is the 13 goal boundaries plus every port that has been the subject of a hypothesis, a diagnosis finding or a migration patch, or shares the development families' code (cnlil, mobxlil, playcanvaslil, rehype-katexlil, hast-util-to-htmllil, rehype-stringifylil, remark-gfmlil and the rest of the unified ecosystem). The held-out set is monacolil and solidlil, with their prior exposure recorded (scoreboards and the idiom census), plus **at least three ports written blind** before M3's phase end (a fresh session, upstream's source only, libraries outside the development ecosystems), then two or three per quarter. NO1a's procedure for a held-out loss is a ledger row type. Their cells are published at every phase end and gate it (rule 10) |
| M2.12 Perf runner, runtime ledger, static counters and micro perf gates | `scripts/perf.mjs`: motionlil's protocol (paired, alternating, a fresh process or page per lane, bootstrap CI), with the parity definition of architecture §16.4: at least three no-op-perturbed builds per artifact (`.pA`/`.pB`/`.vC`), at least five fresh processes each, at least 150 rounds after at least 30 warm-up, steady and warm-up windows, the controls' spread δ, a host-load threshold and the quiet-window protocol; pinned Node and Chromium gate, one non-V8 engine reported. **The runtime ledger** (architecture §16.6) frozen with measured ratios and intervals. **The warm-up study** for katexlil (bytecode size per hot function, deopts in rounds 0–40, tier-up), reported with an owner for what it finds. Structural counters in `--explain` now (shared adapters per method, `arguments` materializations, trampolines, startup statements); allocation and polymorphism counters when M6.6 and M6.8 land. A micro perf gate per law P1–P9, and from them the versioned runtime-class calibration table (architecture §16.3). Replaces katexlil's 50% guard rail and the site's single 30-round sample |
| M2.13 Generic corpus ratchet | **Landed 2026-09-27** (branch `m2-tools`; record above). `comparison/cases` (canonical and catalog), `comparison/apps` and `comparison/algorithms` run per change against the smallest competitor, Closure ADVANCED and the frozen old route; the loss counts are a ratchet (rule 3). The case configurations move to the current schema with no "no effect" warnings; the 38 refused catalog cases are rewritten or ledgered with owners (BC12); the NO3 grep test with its allowlist ledger (about 35 port-name mentions in non-test `src/` today); the idiom-debt ledger (NO4). Doc hygiene in the same task: `docs/current-status.md` (it still says "M1 is active") and the stale `Cargo.toml` claims (Oxc "reserved for … final rewrite admission" at `:31-35`, where no admission parse exists; `rayon` declared at `:14` and unused) |
| M2.14 Frozen baselines | The pre-M1 binary's compile times on the 2026-09-27 sources and configurations, frozen once per reference port: each main entry at level 13 and the full `npm run build`, median of five alternating runs, host load and credit state in the receipt; one number per port, with its size-normalized form (seconds per 100 KB delivered). Upstream's own build times and Terser's times beside them. Architecture §13.2's provisional values are replaced by these |
| M2.15 Calibration corpus | Medium-size generic programs (10–300 KB) outside the development and held-out sets: unported npm packages compiled through `JsValue` shims, and synthetic concatenations of case programs, each with an oracle. Priors, the proxy margin M, the tariff and the exchange rate are calibrated on it and the case corpus, leave-one-out (NO9); the proxy's miss rate is validated on its no-prune lane. Lands before M3.5 |

**Exit.**
- CI is green on the one compiler.
- The case runner, the micro gates and the port runner run each batch.
- The baseline scoreboard exists and the interim release is published.
- The benchmark contract builds every goal cell's bar; the held-out set is named and its first blind ports exist; the perf runner, the runtime ledger, the generic ratchet, the frozen baselines and the calibration corpus run on the ladder of rule 8.

### M3 Honest configuration, one public API, delivery contract, the budget model

| Task | Content |
|---|---|
| M3.1 Schema v3 | The axes of architecture §14: contract (`[target.javascript]` with independent `execution`, `world`, `format` and `checks`, where `format` replaces `function_scope`), objective, effort, resources, execution, performance, generated families. A translator maps every old key to a new key, a warning or a refusal. **First slice with M3.5** (objective, effort, resources, performance, `checks`); `format` with M3.3b |
| M3.2 Family registry | Every program rule, JS rule and choice registers `{id, mandatory or optional, legality, risk, runtime class, bytes per WU}`; a rule's entry carries its L3 sign distribution. `TargetCompaction` splits into its real families. The five tactics without a producer are removed. The receipt lists the families that actually ran. With M3.5 |
| M3.3 Delivery contract | The judged design (`~/lilscript-work/out/design/m3.3/design.md`), adopted with architecture §10.6's overrides and delta: M3.3a (multi-entry ESM, preserve-modules, lazy chunks, the plan verifier, manifest v3), M3.3b (formats), M3.3c (facts and choices, after M5.2, M6.2 and M9.1; **its exit gates the consumer lanes, BC9**), M3.3d (lazy effects and cycles), M3.3e (ports, with M12.2). Deploy cost uses the objective's codec only. `verify-bundles.mjs` is split into contract assertions and plan assertions, and its fixtures stop depending on `strip_console` |
| M3.4 Public API and shared formation | `build::{check, build, with_session}` with a typed `BuildReceipt` and `Delivered { files, manifest, sizes }`. The multi-objective CLI (`--objective raw,gzip,brotli`) gives one winner per objective, from **one formation per candidate judged under each objective**. With M3.5, before its joint moves |
| M3.5 The budget model (absorbs M3.6, M9.4, M5.6's accounting and the structural part of M9.10) | Architecture §9.6 and §13, in one milestone that lands before any new codec-judged family:<ul><li>**Work units.** A versioned tariff per work kind and codec setting (B1), charged by input size in every loop; WU per phase and per move in the receipt; `--print-policy` prints the planned budget; `[resources] work` is a deterministic truncation and `deadline` abort-only; B9's exchange rate and per-level ceilings. `build::search_request`'s caps and `LILSCRIPT_SEMANTIC_WORK` go. M5.6's per-worker ledgers and scratch limits land here.</li><li>**The procedure of §9.6.** Terminal challengers and choice alternatives merge into one level-independent list; levels set p(L), e(L), W(L) and F only; the two budgets of `src/config.rs:1339-1366` and the seven ladders of `:1183-1366` go. The structural beam runs at its level-13 schedule, ranks by proxy, and is one move at a fixed late position until M9.1's rest deletes it. Lockstep objectives with checkpoint offers; the gzip shadow.</li><li>**Three judges.** Tier-0 estimate under the name plan orders (the old M9.4); a proxy (Brotli at min(q, 5); gzip and raw exact) prunes beyond M; only the exact codec keeps; the exact budget bounds exact work. A periodic no-prune lane measures misses. Calibration on M2.15's corpus.</li><li>**Codec settings as objective configuration:** `[objective.brotli]` quality, window, mode and `[objective.gzip]` level, window, defaults unchanged, fingerprinted.</li><li>**Deterministic parallelism:** speculative parallel greedy with in-order commit; `-j N` (default from the schedule, never fingerprinted); `--codec-jobs` removed.</li><li>**Level 0** runs every rule and the naming seed, and no codec (estimated sizes).</li><li>**Per-port budget keys** (`candidate_search`, beam widths, byte and probe budgets, `[policy.search]`) warn for one release, then refuse after Y7; the port configs are rewritten to contract, objective, effort, performance floor and permissions.</li><li>The WU, moves-judged and CPU-time gates in `scripts/cases.mjs` and `scripts/ports.mjs`.</li></ul>**Exit.**<ul><li>*Monotone:* size(L+1) ≤ size(L) for L in 0–15 on the case corpus and the reference ports, by construction and by a test at every tier boundary that compares the recorded incumbent at each lower level's stopping point with a build at that level.</li><li>*Size:* level 15 at or below today's shipped bytes on every reference port; level 13 within the port's seeded re-mangling band of its shipped bytes, or the difference ledgered as a known cost with its owner (M9.1's rest or M7).</li><li>*Time:* level 13 at or below the frozen baseline (M2.14) at `-j 4` on every reference port and at most 1.3× it single-threaded; katexlil's package build at or below KaTeX's own build (14.4 s) and every other reference port's package build at or below its frozen pre-M1 package time; levels 1–4 within 2× Terser's time; the tariff's median error under 10%.</li><li>A thread-count change never changes bytes.</li></ul> |
| M3.6 Codec pool | Merged into M3.5 (deterministic parallelism) |
| M3.7 Environment variables | Only diagnostic variables remain |
| M3.8a Consumer-shakeable delivery, first half (architecture §10.6, override 1) | With M3.3b's formats, for library-world ports, in the bundler-facing export condition only: `/*#__PURE__*/` on top-level calls in `Definition` root rows (which require `discardable()`), `sideEffects: false` per file only when every root row is `Definition`, exports bound by declaration-initialized bindings where the initialization fact proves no read in the temporal dead zone |
| M3.8b Consumer-shakeable delivery, second half | After M5.2 and with M3.3c: `/*#__NO_SIDE_EFFECTS__*/` on functions `discardable()` for every admitted argument that call no parameter; the choice between annotations and export granularity; gated by the consumer lanes (BC9) at M3.3c's exit. Today a consumer importing one zod export pays 28,006 Brotli from zodlil against 11,749 from upstream |
| M3.9 Caches and the decision lock | A codec memo keyed by (byte digest, codec settings) across candidates, objectives and chunks, persisted on disk; a content-addressed build cache (`--cache DIR\|off`); the opt-in decision lock `lilscript.choices.lock` (`--write-choices`, `--choices`) with memoized verdicts, a fingerprinted input replayed as move 0; a per-module elaboration cache for level 0 and the language server. Every cache is output-transparent except the declared lock (B7). With M3.5. *Incremental formation and fact reuse: merged into M5.7* |

**Exit.**
- No accepted key is silently inert.
- `--print-policy` equals the build's request, including the planned work budget.
- The bundle contract cases pass.
- A thread-count change never changes output bytes.
- M3.5's exit holds.
- The consumer lanes are M3.3c's exit, not M3's.

### M4 Checker identities and checker-owned facts

These are prerequisites for field identity, shapes and every fact the checker already proves.

| Task | Content |
|---|---|
| M4.1 Nominal identity | <ul><li>`NominalId` for classes, enums and extern classes, with per-module scopes; the 16 port class renames are reverted.</li><li>One module-graph checker entry with explicit phase products; `export constructor` in module mode.</li><li>Class fields become `FieldRef{nominal, slot}` places in the IR, and allocations carry their nominal.</li><li>`ClassDefinition`, native and formation look classes up by id, not by name</li></ul>**Landed 2026-09-27** (branch `m4-nominal`; see "M4.1 nominal identity" above). The rename reverts are patches in `~/lilscript-work/portwork/nominal/`, to land with each port's next release |
| M4.2 The dynamic type (with R12, M10.2) | `Type::Dynamic` replaces `TypeParameter("$js")` at every site (`src/check.rs:9515`, `src/primitive.rs:654-656`, `src/program/effects.rs:361`, `src/program/native_plan.rs:476`); `unknown`; type parameters by id; interned types without source lifetimes (`NominalType`'s `&'src str` goes). It lands together with R12's ordinary syntax on the dynamic type (M10.2) in language slice 1, as its own batch because it touches the whole checker |
| M4.3 Checker facts transported | <ul><li>`ResolvedOperator` recorded by the checker; elaboration stops re-deriving `IntBinary` from result types.</li><li>`assigned` split into `reassigned` and `observable_before_initialization`, plus a per-occurrence `ReadInitialization`, which seeds M6.5; the binding facts (written anywhere, creation moment) the tree's binding column needs.</li><li>Parameter defaults on declarations, not in function types.</li><li>Declaration attributes (`pure`, `debug`); ambient `this`/`arguments` as checker-resolved bindings</li></ul>Before M5.2 |
| M4.4 Node ids | Ids on identifiers, declarations and statements; the span-keyed fact maps are deleted; tree-level `ChoiceKey` ordinals become source identities. Before M5.2 |
| M4.5 Contracts and capabilities at check time | Frame (D3.9), boundary (D2) and delivery (DL1–DL10) refusals, and **target capabilities** (every JavaScript-only type or operation under a native target), are diagnosed in the check phase with source spans, against the resolved contract. Native's `Unsupported { span: 0..0 }` refusals (`src/program/native_plan.rs:1046,1073`) leave the backend (M11.11) |
| M4.6 Operation catalog (in the core; no ruling) | `BuiltinCall` and `Intrinsic` merge into one declarative catalog: signature, defaults, effect class (including `debug`), fold with its exactness class (`exact` or `host-precision`), JS and C spelling, target capability, per-condition bindings (R17). A `GlobalId` on host nodes replaces `Expr::Host(String)` name tests and `STANDARD_GLOBALS` (`src/js/inline.rs:20-55`). The checker diagnoses non-portable use against the requested targets. The "never rename" host surface, and the property names the allocator reserves, are derived from `extern` declarations and the catalog. It carries effect-free known constructions (`new RegExp(valid literal)`, diagnosis C9) and `Object.hasOwn` for `hasOwnProperty.call` (1.2% of katexlil's self time) |

**Exit.**
- Tests show that two modules' private `class Node` compile, `export constructor` works, and no `"$js"` string test remains.
- A grep finds no name-keyed class lookup.
- The checker's diagnostics carry spans for every refusal, native capability refusals included.
- No name-keyed host-global test remains on the JS tree.

### M5 The machinery

What every later phase needs: edits, the carriers that take facts to the tree, the scheduler, and selection that cannot regress.

| Task | Content |
|---|---|
| M5.1 Program edit kernel | Structural `EditBatch` on the existing transaction (insert, remove, splice, clone, delete unit, merge units, change signature, retype allocation); `UseIndex` updates incrementally, checked against a full rebuild. DCE, demand's liveness applied as an edit, is its first production rule, and native gets it: the native plan then plans only demanded code, so an unused refused function no longer blocks it. `drop_unreferenced_functions` (`src/js/mod.rs:1536`) is deleted. In the floor slice, after M2.5 |
| M5.2 Tree annotations and journal (absorbs M8.1) | <ul><li>Annotation columns on the JS tree (value domain, binding facts, evaluation behaviour, `FieldRef`, `AllocSite`, `UnitId` and function facts, callee, effects summary, initialization order, `GlobalId`, observation, spelling, frequency), renumbered by the arena itself, each with a declared transfer for every target-rule edit (a join where statements merge, top for created nodes) and a debug-build verification against recomputation after each rule set. M3.3's root rows ride the effects and initialization columns.</li><li>`literal_alternatives`, `binding_classes` and `defined_parameters` move onto nodes; the hand remaps (`src/program/javascript.rs:797-1259`) and the `protected` protocol go.</li><li>Typed mutation helpers journal every edit; a debug build checks the journal against the actual difference.</li><li>**Deleted in the same batches, once their columns verify (rules 2 and L20):** `src/js/quiet.rs`; `inline.rs` `inert` and `runs_no_user_code`; `js/mod.rs` `inert_value`, `initialized_at` and `settled_reads`; `initializers.rs` `same_inert`; `javascript.rs:207-224`; the `debugLog` name tests; `root_constants.rs`'s own proof; native's initialization proof (`src/program/native_plan.rs:1102-1217`). M6.1–M6.5 are then landed</li></ul> |
| M5.3a Scheduler | One journal-driven scheduler for program rules and JS target rules: one lexicographic measure (units, allocations, operations, nodes) asserted per edit in debug builds; a fixed structural order over SCCs; always to the fixed point, never truncated by effort; a round ceiling that aborts; the verifier after each rule set in debug builds. It hosts the classified rules and, for every pass whose replacement has not landed, a **marked transitional rule** in its current relative order with its deleting task (architecture §8.2's table). There is no byte-identical mode. The hand-written chain (`src/program/javascript.rs:744-986`, `:1073-1211`), its magic limit 6 and its hand rounds are deleted as a chain. Needs M2.5 and M5.2 |
| M5.3b Transitional rules deleted | Each transitional rule is deleted in the batch its replacement lands: `drop_*default*` with M7.3; the removing half of the inliners with M7.5a, the rest with M9.1's rest; `eliminate_aliases`/`forward_root_constants` with M7.4; `inline_initializers`, `drop_redundant_init_stores`, `fold_object_stores` with M9.7 and M7.7; `scalarize_member_objects` with M7.9; `self_method_calls` and `array_receiver_calls` with M10.4/M10.7 and M6.4b. Interleaves with M7 |
| M5.4 Monotone selection and the terminal slot | Incumbents never worsen. A terminal challenger stage offers choice assignments on the final artifact under the requested codec. The mechanism lands before any new family. **Landed** (batch T). Monotone across levels is closed by M3.5's §9.6 procedure, in which the lower level's result is the incumbent where that level stops (the replay is structural) |
| M5.5 Dataflow and views | The call graph with SCCs (generalized from `CallableInputs`); one region-structured dataflow solver; the cell-SSA view. Before M7.4 and M6.4b |
| M5.6 Resource accounting | Work units per phase and per rule through M3.5's tariff; exact byte accounting only for retained candidate and artifact storage; per-worker ledgers and scratch limits for speculation. The allocation-exact ledgers inside analyses (about 4,400 ledger lines in `src/program`, 1,074 in `src/js`; `AllocationBudget<'a>` across 398 sites in 101 files) are retired. **Lands inside M3.5** |
| M5.7 Incremental tail (absorbs M3.9's incremental formation) | Re-form only the units a move touches and re-run JS target rules on them through M5.3a's dirty-unit scheduling; reuse `Arc`-shared units and revision-keyed facts across candidates (demand re-runs about 340 ms per katexlil candidate today). **Exit:** level 13 at or below the frozen baseline single-threaded on every reference port |

**Exit.**
- The hand-written chain is gone; the scheduler runs classified rules, and every remaining transitional rule is listed with its deleting task.
- The terminal slot runs on every build.
- The edit kernel carries DCE for both targets.
- A grep finds none of the re-derivations M5.2 lists.
- M5.7's single-threaded target holds.

### M6 The fact spine

Every fact is taken through the same four steps in one batch: **compute → publish → switch consumers → delete the re-derivation.** A fact counts as landed only after the fourth step (rule 2). M6.1–M6.3 and M6.5 have done the first step and the program-side consumers; M5.2 does the rest for them.

Facts rest on the language's guarantees (architecture §7). The exact and finite-set tier of the value lattice needs no language rule and lands in the floor slice (M6.4a); the int32-range tier waits for R1 and R11 (M6.4b).

| Task | Fact | Deleted when its consumers switch | First reproductions |
|---|---|---|---|
| M6.1 Call graph and function facts | Complete call sets, value calls resolved, address-taken, name and length observability | The call-only scans in `inline.rs`; the name-observability predicates in `javascript.rs:1265-1537` | — |
| M6.2 Effects | Per-operation and per-unit summaries, seeded by declared `pure` and trusted `pure extern`; `discardable()` is the one removal and annotation test. Termination needs a proof (D3.6; Y4) | `facts.rs`'s unknown calls, `demand.rs`'s private model, `helper_family`'s composition, `quiet.rs`, `inline.rs:inert`/`runs_no_user_code`, `mod.rs:inert_value` | `scratch-program/ip/pure.lil`; motionlil's `warning`/`invariant` (the `debug` class, not purity) |
| M6.3 The `pure` contract | A check-phase diagnostic from the effect engine | — | The old route's diagnostic, verbatim |
| M6.4a Values, exact tier (floor slice) | Exact and finite-set values over literals, `const` and `define`, per value, formal and result, joined over complete call sets; folds only `exact` catalog operations | `simplify::known` where it covers constants | `comparison/cases` `control/dead-branch`; `functions/nested-local` |
| M6.4b Values, range tier (after M10.12 and M10.9) | int32 ranges and primitive classes; the array class of parameters and results over complete call sets (diagnosis C15). Held per value, formal, result and `(nominal, slot)`. Declared types are the lattice's start under R1; the effect obligations of `src/program/effects.rs:17-27` go | `javascript_int32.rs`'s proofs, `NumberFacts` sources, the `binding_classes` derivation, `scalar_transfer.rs`, `raw_domains.rs` | `scratch-target/a.lil` (`9+40\|0`); probe `f1`; a D2/D3.3 case with an ill-typed JS caller (development-check lane) |
| M6.5 Initialization order | Settled root bindings; "not invoked before root statement S", seeded by M4.3 | `quiet.rs`'s order and factory shapes, `root_constants.rs`'s own proof, the duplication in `demand.rs:initialized` | zodlil's 41 enum constants; `scratch-target/b.lil` |
| M6.6 Escape and uniqueness | Per allocation site: local, typed or host. A compare with `null` is not an escape. Per struct store: whether the source dies there (a move; law P2) | `scalar_objects.rs`'s syntactic test; lint's `aggregate-escape` returns | snippet `s17`; probe `f1`'s per-iteration array; the `native/pt.lil` cursor loop (4.0× slower than mutable JavaScript today) |
| M6.7 Field facts (after M10.14's checker half) | Per `(nominal, slot)`: read, written-value join, host-reachable. Reflective and exported-shape status is the checker's reflected set (R6), not an analysis | Name-keyed field logic | probes `p2`, `p5`; motionlil's dead fields |
| M6.8 Frequency | A static per-operation class, cold, warm or hot, from the region tree and the call graph (law P8); published as a tree column (M5.2) for hot-site ordering and vetoes | — (new) | katexlil's `toMarkup` methods hot, its `defineSymbol` tables cold |

- Lint: `performance/aggregate-escape` (removed in M1.2) returns with M6.6. Lint's IR rules see the program before program rules and move after M7 when those land.
- The reference interpreter gains each feature (M2.4) before the fact that optimizes it.

**Exit.**
- An owner table shows one owner per fact, and every fact is consumed by both targets where they decide with it.
- A grep finds none of the deleted functions.
- Every reproduction is committed to `tests/cases/regressions`.

### M7 Program rules: the floor

These pass L3, run in the scheduler's fixed structural order, are target-neutral, and native gets every one. A transformation whose value depends on printed length, or whose codec sign is unstable, is not a rule; it becomes a choice (M9). Together they cover the architecture's floor inventory (§8.4): every exact transformation Terser, Oxc, esbuild and SWC ship that is legal under our facts. Each deletes its JS-tree twin in the batch it lands (M5.3b), and each is gated per change on the generic corpus ratchet (M2.13). The search inventory's always-winning opportunities (scalar, product, inline, function) become these rules, which also removes most of jquerylil's 161 structural proposals and their exact scores.

| Task | Content |
|---|---|
| M7.1 Removal | Dead values, units and cells; unused `let`s whose initializer is discardable; dead stores; common subexpressions and algebraic identities, where they pass L3 |
| M7.2 Discarded effect-free calls | Needs M6.2 and the termination proof. **Landed** |
| M7.3 Parameters and returns (floor slice) | `Dropped` and `Constant` transports on complete call sets; unused results. Negative cases: effectful arguments stay in order; `arguments`, rest, exported and host-visible `length`. Deletes `drop_typed_default_checks`, `drop_default_arguments` and `native_default_lengths` in the same batch |
| M7.4 Root constants, defines, flow-sensitive forwarding | Settled constants of a few tokens (schedule data) are substituted by rule; build-time `define`s; reaching-definition forwarding of constants (katexlil's 41 `x=E;return x` sites). Non-constant single-use forwarding and copy coalescing are a coupled choice with prior "apply" (M9.12) until their sign distribution passes L3 |
| M7.5a Removal-only inlining (floor slice) | A rule when every call site is replaced and the unit is deleted with no operation growth; arrow IIFEs whose parameters are read once (diagnosis C16); parameter copies forwarded. Deletes the removing half of the JS tree inliners (M5.3b); the limit 6 (`src/program/javascript.rs:749-751,791`) goes with them |
| M7.5 Inlining, the rest | Known-closure calls and constructor chains. Deleted: `helper_family`, `forwarding_builtin`, `undefined_call`, the semantic parts of the JS tree inliners, and the IIFEs from `place_single_calls`, when the inline-or-share choice (M9) holds the duplicating case. Runtime adapters are not inlined here: formation never emits a shared adapter for a private lambda (M8.2 A1, law P1) |
| M7.6 Namespaces and emulated methods | Constant namespaces collapse, and single-definition methods of compiler-owned objects are devirtualized, on allocation identity and initialization order |
| M7.7 Fields | Dead fields, constant fields, overwritten stores (diagnosis C4b: `a.n=0;a.n=b` in every published class with fields), with the legality of architecture §8.3 (no observation or throw between the stores; key order unchanged or the nominal neither reflected nor published; no accessor on the base chain); proving cases include a published and a host-derived class |
| M7.8a Literal and branch folding (floor slice) | Constant and branch folding on M6.4a, before any spelling is chosen (`if (false)` bodies die here); dead code after folding with M5.1 |
| M7.8 Folding, the rest | SCCP; interprocedural evaluation of small pure functions with constant arguments over `exact` operations; path-sensitive constants; known-method folds from the catalog (`exact` only); string-literal sums; array and object store collection (diagnosis C17); `\|0` only where a result can leave int32 without a range proof (after M6.4b) |
| M7.9 Scalar replacement and store-copy elision (before M9) | Of classes, control-flow aggregates and loop-carried structs, on escape; a struct store that is a move copies nothing (P2), on both targets. Deleted: the tuple rebuild per field write (`src/program/javascript_structs.rs:1-3`) |
| M7.10 Identical units | Compiler-generated units equal modulo names merge by rule only under P1's static test (architecture §8.3): no call to a parameter or capture, no receiver-nominal growth. Receiver and D2 adapters never merge by rule. After M8.5 for prelude units (batch A1 already emits one decoder per schema at formation) |

**Floor slice interim exit.** `comparison/apps` and `comparison/algorithms` at or below the frozen old route (558 and 2,305 Brotli, the ratchet's bars; the review measured the apps at 571 under their default configuration), or each remaining losing case named with the M7 task that owns it.

**Exit.**
- `comparison/cases`: no case loses to the reference binary's old route, and none loses to the smallest competitor without a ledgered owner.
- `comparison/apps` and `comparison/algorithms`: no case loses to Closure ADVANCED, and the totals are at or below the old route.
- Every M6 reproduction is fixed.
- The JS-tree twins listed in architecture §8.2 are deleted.

### M8 Canonical formation and the pure printer

| Task | Content |
|---|---|
| M8.1 Formation writes annotations | *Merged into M5.2* (the annotation columns) and batch A1 (import identity `(source, imported)`; a default import prints `import x from`, diagnosis C18; the import spelling pinned "for katex's build", `src/program/javascript.rs:2705-2707`, goes) |
| M8.2 Canonical forms | **Batch A1 (next; needs no tree analysis; 8 changes):**<ul><li>**A method is its own function** (law P1): `JS.methodN`/`methodRest` of a private lambda (or a private function referenced only by the adapter) is emitted as a function whose receiver reads `this`; a rest list read only at constant indices with `length` unobserved becomes named formals (katex-perf B2), otherwise it reads `arguments`. Guards: the receiver and rest parameters are never assigned or captured by a nested non-arrow function or a nested class's methods (the prototype's `receiver_reads` misses `Expr::Class` methods and must refuse there); the lambda has no own `this`/`arguments`; arity is kept; a case pins the gained `.name`. Constructors take the formals form or keep a private, unshared function each (the unconditional constructor form costs mobxlil +130 Brotli). Shared adapter factories (`src/program/javascript_host.rs:43,538-555`) are no longer emitted for private lambdas; `dissolve_receiver_adapters` is deleted. Prototype: `~/lilscript-work/diag/katex-perf/receiver-methods.patch`. **Pre-registered:** katexlil −62 Brotli (the adapter change alone), Node steady −0.04; micromarklil −283 Brotli and −311 gzip; zodlil within noise; mobxlil at or below zero.</li><li>**Observed classes print with their bodies** (diagnosis C1) within architecture §10.2's legality (class-body strictness under `execution = "script"`; a constructor omitted only for a parameterless empty base or an exact forwarding derived constructor); the dead receiver alias is dropped. Then posthoglil's error-tracking port change to `export constructor` (−360 with both halves).</li><li>**Operands are not spilled** (C3): object and array literal operands keep argument order without temporaries; a constant computed key never carries a sequence (mhchem −250).</li><li>**One decoder per schema** at formation, keyed by schema digest (`src/js/tables.rs:1471-1474`), routed through M3.3a's `RootRow` helpers.</li><li>**Imports** (C18), from M8.1.</li><li>**`print` never stripped** (M2.3's open item): `strip_console` splits into `strip_debug` and `strip_console_calls`, both off by default for libraries.</li><li>**Naming's seed at every level** (M9.5): out of the level-8 gate (`src/compilation_policy.rs:240`).</li><li>**Native arithmetic** (M11.2) with its guards.</li></ul>**Batch A2 (after M2.5, M4.6 and M5.2):** `let x; x = E` fusion and dead `let` with a discardable initializer as JS target rules on the binding and evaluation columns, with the TDZ and loop-capture legality (C4a); C9 through the catalog; `globalThis.<ECMAScript builtin>` as the builtin under R10 (C19); `fold_logical_assignments`/`fold_logical_returns` replaced by canonical formation of the constructs they recover. The overwritten-default drop is not a formation recognizer: R3 removes implicit defaults and M7.7 removes overwritten stores.<br>**The rest, each in its replacement's batch (L20):** constructions by the layout choice (M9.7: `inline_initializers`, `drop_redundant_init_stores`); defaults by the transport (M7.3: `drop_default_arguments`, `native_default_lengths`, `drop_typed_default_checks`); receivers (M10.4, M10.7 and M6.4b: `self_method_calls` except for user-written `JS.call`, `array_receiver_calls`) |
| M8.3 Pure printer (before any further spelling family) | Structure rewrites leave the printer (`return c?a:b` consuming the next statement, logical statements, loop heads: `src/js/print.rs:523-560,1744-1795`) and become spelling attributes set per site. The `raw_spelling` bundle (`src/js/naming.rs:20-28`) is dissolved into its members, each its own family; the printer stops consulting the naming plan. After M5.2. *Second half, after M6.4b:* `\|0` and `++` come from facts |
| M8.4 Host modules | A typed Oxc visitor produces host units; the ESTree JSON walk is deleted |
| M8.5 Runtime helpers | The table decoders, reference helpers and D2 adapters are written as LilScript prelude code (`src/prelude/`), compiled through the pipeline, merged by M7.10 under P1 and demand-pruned. Share-or-specialize per schema is a choice (M9) |
| M8.6 Source maps | Re-founded on tree origins as a delivery-plan feature. The old-route source-map branches are prior art |
| M8.7 Port-shaped rules | Every rule in the inventory below is either given a generic legality condition, turned into a choice, or deleted. Every contract assumption a port sets carries a recorded reason. The NO3 allowlist ledger ends empty |

**Exit.**
- No pass recovers a shape formation emitted.
- No side table is remapped by hand.
- The printer holds no semantics.
- The port-shaped inventory is empty.
- No shared adapter serves two private methods (the P1 structural counter is zero on the reference ports).

### M9 One choice system: choices, naming, layouts and data

Every family is a `ChoiceFamily` of the one kernel (architecture §9.3): sites keyed by program identity, hierarchical per structural class, with a codec-free legality, a per-objective prior, a tier-0 estimate under the name plan, a coupling class and a runtime class per site. The procedure of §9.6 walks them as one move list. No family lands before M3.5, and each declares its bytes per work unit.

| Task | Content |
|---|---|
| M9.1 Choice interface | `Choice` + `ChoiceMap`. **First slice landed (`m9-data`):** `ChoiceKey`/`AltId`/`ChoiceMap` on the output assignment, sites recorded on the tree, per-site challengers and a joint canonical move in the terminal stage under their own `terminal_choices` budget; data encoding is its first family. **The rest:** every optional representation becomes a family with prior, estimate, coupling and runtime class; program-level families key by program identity; coupling classes generate joint moves; the inline-or-share choice holds inlining's duplicating case. **Deleted:** the structural recipe search (beam, diversity clock, render batches in `src/program/search.rs`; `search_selection.rs`, `search_opportunities.rs`, `search_entries.rs`), `implementations.rs`, the five bespoke family analyses (`helper_family`, `product_family`, `record_family`, `string_family`, `function_layout`: 8,393 lines) once their codec-dependent opportunities are families and the rest are M7 rules, and `OutputFamilies`/`Challenger` once their members are per-site families |
| M9.2 Objective as a judge | `raw_structure` and `raw_spelling` become per-family seeds, and every family is available to every objective. **Landed** for the terminal families (batch T). **Open:** gzip gets its own prior row; today it reuses Brotli's (`src/js/families.rs:116-119`) |
| M9.3 Spelling families | 013-T7.2's subset (measured −462 Brotli over six ports); `loop_head_declarations` and `logical_statements` become choices. **First family landed** (batch T) as module-wide booleans; after M8.3 each spelling is a per-site family with structural classes (conditional returns per site class, diagnosis C14; optional chaining and logical assignment, C13) |
| M9.4 Estimator and finalists | *Merged into M3.5* (the three judges and the exact budget) |
| M9.5 Naming | One allocator with the locality algorithm of architecture §9.5. Its seed runs at every level (batch A1). Frequency seeds and frequency-sorted alphabets are whole-artifact joint moves. Root names are allocated over printed bindings only (C11). Closure's scheme measured +86 to +1,171; root order by use count stays a joint move (refuted as a default, diagnosis §5). The −353 walk-order headroom is re-measured on current outputs first |
| M9.6 Property names (after M2.10's closed-world cells and M10.14's checker half) | Rename and ambiguate private `FieldRef`s outside the reflected set (R6) (markedlil −252 measured), never using a name reserved by extern declarations or the catalog; reporting the open and closed lanes against the competitors' property-mangling lanes (BC11) |
| M9.7 Layouts | Per nominal or allocation: scalars, positional, named or class; struct parameters as fields; `JS.assume` view or decode. Legality includes the shape law P3. Deletes `inline_initializers`, `drop_redundant_init_stores` and `fold_object_stores` (M5.3b) |
| M9.8 Data | String tables as a choice with an estimator (no 64 / 0.85 thresholds); numeric and columnar tables; pooling at naming time. **Landed (`m9-data`):** literal, front-coded, schema columns (transposition, delta keys, scaled integers, joined strings) and value dictionaries, each verified exact in Rust before it is offered, seeded by the raw estimator and judged by the codec; with katexlil's metrics in LilScript, −2,423 Brotli (a win over Terser and `katex.min.js`). **The rest, after M10.3:** the per-objective estimate (M3.5; katex's three small decoded tables are −218 Brotli as literals); decoders as prelude units (M8.5, M7.10); a lazy-decode alternative and the startup law P6, with the legality fact "the decoded binding is read by computed keys only"; legality by declaration (R9) replacing the `let`-shape recognizer; pooling at naming time |
| M9.9 Function folding | Identical, permuted and one-constant user functions, as a choice; outlining of repeated regions as a choice. Legality includes the feedback law P1 |
| M9.10 Calibration | Per-objective priors per (family, objective, structural site class), the proxy margin and error bands, the tariff and the exchange rate, calibrated on the case corpus and M2.15's calibration corpus, leave-one-out, never on ports (NO9), and versioned with the schedule. The structural monotonicity of levels is M3.5's; this task makes each level spend its budget where bytes are |
| M9.11 Order and locality | A gzip lever: the order of hoistable declarations and data within an initialization class, and pooling of repeats more than 32 KiB apart (katexlil loses 4.5% with a 32 KiB window). Judged per objective |
| M9.12 The diagnosis's codec-judged items | Batch 3 of the 2026-09-27 diagnosis as families, after M3.5 and M8.3: element loops as `for…of` or index (C5, within the runtime floor; after R14), regex placement (C10, only without `g`/`y` flags or `lastIndex` uses, and with identity unobserved), export binding shape (C12), conditional returns per site class with a lookup-table alternative (C14), coercion text as one coupled class (C8, where R1 and R11 have not already removed it), non-constant forwarding and copy coalescing as a coupled class (C4a's rest) |

**Exit.**
- No objective-conditioned rule remains.
- One kernel, one move list; the structural search, the five family analyses, `OutputFamilies` and `Challenger` are deleted.
- Every family declares its runtime class, and admission orders and vetoes by it on hot sites.

### M10 The final language (R1–R18)

The rules are [language.md](../language.md) (version 1); the architecture's §12 gives the principle. Items are ranked by bytes × runtime and ordered by rule 9: **language slice 1** (M4.2 + M10.2, then M10.4, M10.7, M10.16) needs no ruling and lands right after the floor slice; **the core** (M4.6, M10.13, M10.9, M10.15, M10.11's `debug` class, M10.14's checker half, then M10.12 after Y1) lands before M6.4b and M6.7. Every rule that refuses code the ports contain lands in two batches (language.md §14): a warning with a fix-it applied to every reference port as patches in `~/lilscript-work/portwork/` with suites green, then the refusal. Each item lands with at least one port using it, no Brotli loss, and its typed form at least as fast as the `JsValue` spelling (micro perf gate, P9). ⚖ marks an item that waits for an owner yes (architecture §21.1).

| Task | Rule | Content | Evidence |
|---|---|---|---|
| M10.2 The dynamic type (slice 1, with M4.2) | R12 | Ordinary JavaScript syntax on `JsValue` with JavaScript's meaning (member, call, `new`, `\|\|`, `&&`, `??`, `?.`, `?.()`, `typeof`, `instanceof`, `in`, `delete`, spread, `for…of`, `for…in`, `==` loose, `===` strict); `unknown`; `as`, `as?` and explicit conversions; the 63 `JS.*` builtins collapse into it and the catalog; a mechanical rewrite script per port that keeps meaning. This is the diagnosis's batch 2 (C6 `instanceof`, C7 rest, spread and dynamic `for…of`, mhchem's lambda-IIFE logic) | C6 107, C7 98, mhchem −75 ± 20 Brotli |
| M10.4 Receivers and variadics (slice 1) | R7 | `fn(this: T, …)`, methods in literals and shapes, `T... rest`, spread arguments. `JS.methodN`, `JS.methodRest`, `JS.staticRest` and `extern JsValue this/arguments` retire (their formation half landed in batch A1); `self_method_calls` is deleted | katex −240; 3,513 adapter sites fleet-wide; rest −24/−48 on posthog error-tracking |
| M10.7 Identity tests (slice 1) | R13 | `is`/`as?` on identity-kept classes, extern classes, variants and tagged shapes, lowering to `instanceof` or a tag compare | C6; katexlil's `isPrototypeOf` (1.1% of self time); motion's 11 identity-cast externs |
| M10.16 Iteration (slice 1) | R14 | `for…of` lowering becomes a choice within the runtime floor; typed arrays stay index loops; `for (k, v of map)` and `for (x of set)` | C5 −62 (sound subset); error-tracking loops −49 |
| M10.13 Definite assignment (core; two batches) | R3 | Locals without initializers, flow-checked; fields assigned by `init` or the construction literal; no implicit defaults; `this` unreadable before every field is assigned; non-reassignable function declarations and `const` bindings. Batch 1: warning and fix-it with port patches; batch 2: refusal | 8,609 placeholder-initialized declarations (upper bound); `a.n=0;a.n=b` in every published class with fields |
| M10.9 Absence and integers (core; two batches) | R2, R11 | `T?` nullish, `T??` = `T?`, spelling-observing operations refused on `T?` without narrowing, defaults for omitted or absent arguments, reflected nominals' absent fields as missing keys; lengths are `int`; index and code-unit reads carry a precondition, `a.get(i)` is the checked read; `charCodeAt` keeps its JavaScript meaning (47 uses in the reference ports, fix-it to `\| 0` or `codeUnitAt`); float `%`. The development-check lane runs every port suite before the precondition becomes production semantics | `a.get(b)??null??-1`; 11,592 `undefined` ceremonies; index loads `??""` and `\|0` |
| M10.15 Typed intrinsics (core) | R10 | Typed operations mean ECMAScript's originals; `assume_pristine_builtins` retires for typed code; the regex-literal gate goes | react-markdownlil's `pristine = false` cost 2,287 |
| M10.11 Effects and pins | R15, R9 | **Core:** the `debug` effect class (with M8.2 A1's key split); `pure` asserting termination after Y4 ⚖. **Later:** `inline for`, `@pool` and `@choose` as pins the choice system honors; `define` build constants | `print(1);` compiles to an empty file under the default policy today |
| M10.14 Reflection | R6 | **Core, checker half (no ruling):** the reflected set as a whole-program closure after instantiation (generic, transitive, throw and rejection and host callbacks as crossings, host-derived classes); explicit `as JsValue`; the allocator's reserved host names. **After Y3 ⚖:** non-constructible function values; `keep_*_function_names` and `assume_unconstructed_callbacks` retire | posthog `.name` about 9 B, contrib files about 15 B; katex −240 |
| M10.12 Trusted crossings ⚖ Y1 | R1 | Host values enter only at declared crossings, trusted, checked in development builds. Loads never normalize (`src/program/javascript.rs:98-131`); `prove_int32_cells` and the effect obligations go; `typed_arguments` retires. If Y1 is no: crossings coerce once at entry instead | probes p2, p13–p15 (`review-language-probes/`); 222 `\|0` against Closure's 110 on `comparison/algorithms` |
| M10.17 Host catalog and generics | R17, R18 | The platform catalog on M4.6 (`document`, `RegExp`, `console` …) with effect classes and per-target, per-condition bindings (a `browser` condition decoding entities through the DOM); `globalThis[...]` access retires; generics carry no runtime type information | small-contrib −98 (host globals); the micromark family's browser cells (about 9K each) |
| M10.1 Shapes | R5 | `data`/`accessor` fields, optional fields, construction literal, spread of the declared key set, intersections `A & B`, nesting at boundaries. micromark's 11 views migrate first. `assume_pure_property_reads`, `preserve_properties` and `internal_properties` retire for declared values | −6,359 Brotli on the markdown stack as a global flag |
| M10.8 Record spread and records | R5 | Spread with CreateDataProperty semantics over declared keys; `Record<T>` stays the null-prototype dictionary | `comparison/cases/collections/record-json`; 33 refused catalog cases |
| M10.3 Const data (before M9.8's rest) | R9 | Deep-immutable `const` data; exported const objects; bounded `const` evaluation of `exact` operations (katex's `unicodeSymbols` computed at compile time); M9.8's legality becomes a declaration check; `const` units content-addressed for the codec memo | katex −2,529, micromark −1,054; `unicodeSymbols` 927–982 Brotli and about 7 ms of self time per import on a loaded host |
| M10.19 Specialization over const data | R9 | A function called with const data is specialized on it at compile time, under M10.3's bound and exactness: a validator compiled per static schema | zodlil's 5.5× runtime gap (upstream's `new Function`, `zod/v4/core/schemas.js:970-987`) |
| M10.5 Sealed virtuals and sum types | R8 | A static call, tag switch or prototype method per call site; sum types with payloads | motion ≈ −700 |
| M10.6 ABI-valued enums | R8 | `enum T: string`, explicit values, `ordinal`/`from`, flag sets; ABI values at every reflected, printed, stringified or `JsValue` position; representation chosen per objective only at unreflected positions | micromark's 104 string types; zod's 41 int kinds |
| M10.18 Value structs | R4 | Places update in place; copy on store only when the source stays live (M6.6, M7.9: law P2); `with` expressions. `ref` removed after Y2 ⚖ | `ref` used 0 times; p4 205 → 170 and p5 267 → 140 Brotli; the cursor loop 4.0× slower today |
| M10.10 Sealed modules | R16 | `object` singletons deleted in favour of module namespaces and const records (0 uses); internal exports are visibility; the exports of every declared delivery entry are ABI | — |

**Exit.**
- Each rule has a port using it, and every clause of [language.md](../language.md) is in force, so v0.1 is retired.
- The census of `JsValue` and `JS.*` per reference port, reported at every batch, falls from the 2026-09-23 counts, with no Brotli loss.
- Every typed form is at least as fast as its `JsValue` spelling on its micro perf gate.

### M11 Native: the whole language, directly executable

The native-complete definition (architecture §11.1): every checked construct lowers to C, or is a JavaScript-only capability the checker reports with a span. Native forms from the optimized program and must run at least as fast as the JavaScript output under Node.

| Task | Content |
|---|---|
| M11.1 Toolchain owner | One owner for compiler discovery, flags, strictness and sanitizer profiles, used by the CLI (`-O3` today, `src/main.rs:672-690`), the case runner (`-O2`) and the tests (Clang 18 from an environment path). Native diagnostics carry spans and render as text |
| M11.2 Plain arithmetic (**in batch A1**) | Remove the per-operation `volatile` (`src/program/native_runtime.rs:188-192`; 3.8× on float loops) only together with in-source guards that hold under any user's flags (`#pragma STDC FP_CONTRACT OFF`; `#pragma GCC optimize("fp-contract=off")` for GCC, which ignores the standard pragma; `#pragma clang fp contract(off)` for Clang), the required flags documented in the generated header, and one JS==C float case compiled with GCC's and Clang's default flags. `int * int` as an int64 multiply plus a range check (the plain wrap on proven ranges comes with M6.4b). Left shifts stay on `uint32_t` (`ls_shl`, `:230-233`; a signed left shift can be undefined in C11); signed right shifts may be plain under a static assertion that `(-1 >> 1) == -1` |
| M11.3 Externs per target | `extern` binds a JS host name or a C link name; the header is generated. The `verify.sh` extern-ABI gate passes again |
| M11.4 Portable records | `Record<T>`, `Object.keys/values/assign` and JSON natively; the differential's native `Record` lane is unmasked |
| M11.5 Native forms from the optimized program (follows M7 directly) | Native is formed after the program rules, not from the unedited source (`src/build.rs:609-611`). It consumes liveness, initialization order (no `ls_gN` guards, `src/program/native.rs:401-418`), escape and uniqueness (stack storage, refcount elision, store-copy elision), effects (throw-free calls skip status checks) and specialization (monomorphized hot generics, borrowed closures) |
| M11.6 Exceptions, suspension, regex, strings | Exceptions by status propagation over `Try` regions; generators and `async` as region state machines with a runtime microtask queue (D3.8); QuickJS's `libregexp` (MIT) vendored at a pinned commit, differentially tested against V8; reference-counted strings (ABI v2) |
| M11.7 Runtime and symbols | Runtime helpers as C files with declared dependencies and a standalone `-Werror`/sanitizer build; one native symbol allocator |
| M11.8 Objective and the library ABI | A native objective, speed by default (size and balanced as alternatives), measured on a native perf corpus (`comparison/algorithms`, compute-bound cases, ports' portable cores); a C library ABI (exports plus a generated header), without which no library is directly executable. Before M11's exit |
| M11.9 Profiles | Cross triples and wasm32-wasi as toolchain profiles; the case runner runs one cross triple |
| M11.10 Host API | A typed native host catalog on M4.6: argv, env, stdin/stdout/stderr, files, clock, exit code (`int main(void)` takes no input today, `src/program/native.rs:367`) |
| M11.11 Native-complete definition | Every construct of the typed language has a native lowering or a checker capability refusal with a span (M4.5); the C lanes' 236 masked cases are each lowered or reclassified as a declared JavaScript-only capability |
| M11.12 Cycles | Synchronous trial-deletion cycle collection over reference counts (Bacon–Rajan), run at allocation thresholds |

**Exit.**
- markedlil's lexer and parser and katexlil's parser compile to C as portable cores, pass JS == C on their suites, and each runs at least as fast as its JavaScript under Node.
- The extern-ABI gate is green; the C library ABI builds for those cores.
- Every corpus case runs on one cross triple.
- No C-lane case is masked without a checker capability refusal.

### M12 Qualification and publication

| Task | Content |
|---|---|
| M12.1 Ports own their sources | (Done in M2.8.) `finer/port-migrations/` is retired. Sibling-line knobs and per-port budget keys leave port configs (M3.5); every `assume_*` carries a recorded reason or goes |
| M12.2 No post-minifiers | Compiler-written files for every export condition (ESM, CJS, browser), through M3.3's formats (M3.3b) and multi-entry delivery (M3.3a). motionlil first (in flight with M3.3: one compiler call, `compat.lil`, the nominal rename reverts), then zodlil's `index.cjs`, posthoglil's and katexlil's CJS/UMD re-bundles and micromarklil's "smaller of compiler or esbuild". Library-world files are consumer-shakeable (M3.8a/b). Builds for other objectives that a port wants scored ship as `[[delivery.also]]` files (BC10) |
| M12.3 Every library wins | Per cell of the benchmark contract (M2.10): per library, export condition, objective (with robustness rows), open and closed world, consumer lanes, under the pass rule (rule 4), with the decomposition published (BC7). Work list, from the closure ledger (architecture §18.4): motionlil, zodlil (package and consumer lanes), the micromark family's browser cells (a `browser` condition decoding entities through the DOM, R17), katexlil's code lane and contrib files, posthoglil's error-tracking pack, then the ports without a standing; the held-out set at every phase end |
| M12.4 Rebuild and publish | Every port's `dist/` and Pages site is rebuilt by the pinned compiler with compile times per level (single-threaded and at `-j 4`, with Terser's time and upstream's build time on the same library) and runtime ratios from `scripts/perf.mjs`; one report against the last release and the competitors. Every port's package build at the default level is at or below upstream's own build, with jquerylil ledgered until M7 turns its structural proposals into rules |
| M12.5 Receipts | Refreshed on the final binary; separate conclusions for architecture, correctness, cost and size |
| M12.6 Runtime parity and residual pairing, per port | Port-side work the runtime ledger and the closure ledger name, starting after batch A1: katexlil's idioms (`isPrototypeOf` → `is`, `hasOwnProperty.call` → `Object.hasOwn`, constructor formals where the port spells them); hand-pairing of katexlil core's 417-byte transliteration residual, mhchem's +89..+117 and posthoglil error-tracking's +121..+144 against the strict bar, each finding filed as idiom debt (NO4) with a generic case; diagnosis of jquerylil's retained memory (1.52× on `deferred`) and motionlil's 1.076 workload, each ending in an owner task |

**Exit.** The scoreboard has no losing cell: no size loss in any cell of the benchmark contract, no runtime floor miss, no native gate miss, on the development and the held-out libraries.

---

## Old-route optimizations: disposition

Every pass of the old optimizer chain (`optimizer.rs:243-430`, `compress_passes.rs`) and its emitter planning, with what replaces it. "Measure" means: measure the old route's value with the reference binary before deciding.

| Old-route pass | Disposition |
|---|---|
| `promote_locals_to_ssa` (mem2reg) | Replaced by the cell-SSA view (M5.5) |
| `optimize_scalar_fixed_point`: `fold_and_propagate_control_flow`, `simplify_algebraic_expressions`, `eliminate_redundant_phis` | M7.8 folding, M7.1 algebraic identities that remove operations; phis have no counterpart (regions) |
| `eliminate_common_subexpressions` | M7.1, where it removes operations; otherwise a choice (repetition is load-bearing) |
| `fold_owned_plain_object_reads` | M7.7 / M9.7 on field facts |
| `elide_single_use_stringify` | M7.8 on values |
| `propagate_single_assignment_globals`, `forward_single_assignment_global_aliases`, `internalize_entry_globals`, `eliminate_unread_globals` | M7.4, M7.1 |
| `devirtualize_methods` | Free by construction |
| `devirtualize_known_closure_calls`, `clone_constant_capture_signatures` | M7.5 |
| `specialize_constant_parameters` | M7.3 |
| `specialize_profiled_call_sites` | Dropped: profile-guided optimization is removed (architecture §20) |
| `optimize_unused_parameters`, `optimize_unused_returns` | M7.3 |
| `validate_declared_purity` | M6.3 |
| `optimize_inlining_fixed_point` (`inline_small_functions`, `inline_single_use_control_flow_function`, `eliminate_dead_functions`) | M7.5, M7.1 |
| `subsume_private_functions`, `merge_permuted_private_functions`, `merge_single_operand_private_functions`, `fold_identical_private_functions` | M9.9 (choices) |
| `analyze_escapes` | M6.6 |
| `scalar_replace_linear_classes`, `scalar_replace_control_flow_aggregates`, `scalar_replace_loop_carried_structs` | M7.9 |
| `eliminate_overwritten_field_stores` | M7.7 |
| `propagate_path_sensitive_constants` (SCCP) | M7.8 |
| `superoptimize_pure_expressions` | Dropped unless measured: it was off by default |
| `sink_partial_escape_allocations` | M11.5 (native storage); JS: dropped, it was off by default |
| `fuse_array_pipelines` | Dropped: 0 occurrences of `.map(…).map(` in the six ports; revisit with evidence |
| `outline_repeated_regions` | M9.9 as a choice (it wins raw and loses Brotli) |
| `collapse_single_use_byte_array_buffers` | Measure; else dropped |
| `call_array_methods_directly` | M8.2: formation emits receiver calls |
| `normalize_ambient_regex_constructions` | M4.6 catalog (regex literals are a spelling choice, M9.3) |
| `strip_console_output`, `lower_known_js_host_calls` (name-keyed) | The `debug` effect class (M10.11); name-keyed host helpers are dropped by design |
| Emitter planning (`IrJsEmitter::prepare`: 21 passes) and the 48 scored emission families | Each family is checked for whether it ever changed a winner on the last old-route fleet run (`--explain json` on the reference binary). Families that did become M9 choices; the rest are dropped with the measurement recorded |
| The text peephole (`js_peephole`, about 190 folds) | Dropped. Its insights (structurally identical functions spell identically, same-length names in source order) become naming seeds (M9.5) |

## Port-shaped rules and assumptions: inventory (owned by M8.7)

| Rule or setting | Where | Disposition |
|---|---|---|
| `self_method_calls`, `dissolve_receiver_adapters`, `array_receiver_calls` | `js/calls.rs`, `js/typed.rs` | Deleted in batch A1 (`dissolve_receiver_adapters`), M10.4/M10.7 (`self_method_calls`) and M6.4b (`array_receiver_calls`), each as its replacement lands (M5.3b) |
| Shared receiver-adapter factories (`host_factories`, "one per convention") | `program/javascript_host.rs:43,538-555`; `program/javascript.rs:1328` | Never emitted for private lambdas (batch A1, law P1); the adapters leave the language with R7 (M10.4) |
| Per-port budget and basin keys (`candidate_search = "always"` in 16 configs, beam widths in 13, byte budgets in 14, probe limits in 4) | port configs | Warned, then refused (M3.5, Y7); the port configs keep contract, objective, effort, performance floor and permissions |
| Rules justified by one port's measurement (`js/inline.rs:241`, `js/naming.rs:484`, `js/print.rs:779`, `js/mod.rs:1891`, `js/declarations.rs:6`) | `src/js` | Each becomes a choice or a rule with a declared legality (M8.7), checked by the NO3 grep test |
| `group_prototype_stores` ("only katexlil declares both assumptions") | `declarations.rs:77` | Generic legality stated or deleted |
| Per-site namespace flattening for katex's `let _c;…;_c=$c` | `inline.rs` | Replaced by M7.6 |
| `fold_logical_assignments` / `fold_logical_returns` (transliteration temporaries) | `statements.rs` | Kept as canonical if a generic legality holds; otherwise a choice |
| Nullish narrowing for a port's `isNull` | `simplify.rs` | Replaced by value facts (M6.4) |
| The `undef()` helper-call fold | 008 batch 5 | Replaced by M7.5 inlining |
| Pinned foreign import spelling "for katex's build" | `javascript.rs:2705-2707` | Import identity by `(source, imported)` (batch A1, from M8.1) |
| `loop_head_declarations` (two contradictory measurements) | `javascript.rs:504`, `mod.rs:1043` | Choice (M9.3) |
| `assume_unconstructed_callbacks` (added to patch an unsound rule) | contract | Type fact (M10.4) |
| Every port's `assume_*` settings | port configs | Each gets a recorded reason, or is removed (M12.1) |

---

## Where the old milestones went

| Old | Now |
|---|---|
| 001–007 | Foundations of the compiler; receipts refreshed in M12.5 |
| 008 | M3.3, M8 |
| 009 | M7, M9 |
| 010 | M5.4, M9 |
| 011 | M1, M3 |
| 012 | M3.5 (the budget model, with M3.6 and M9.4 merged into it), M9.10, and the compile-time gate (rule 3) |
| 013 and 013-T1..T7 | M4–M9 and M12. T7.1 → M6.2/M7.2; T7.2 → M9.3; T7.3 → M5.3; T7.4 → M6.5/M7.4; T7.5 → M7.3; T7.6 → M7.5; T7.7 → M6.6/M7.9/M9.7; T7.8 → M4.1/M6.7; T7.9 → M7.7; T7.10 → M7.8/M8.3; T7.11 → M6.4; T7.12 → M7.6; T7.13 → M9.6/M9.7; T7.14 → M9.9 |
| 014 | M1 (retirement), M12 (certification) |

---

## Dependency graph

Every open task, what it needs, and the "Next action" step it belongs to. "—" means nothing beyond landed work.

| Task | Needs | Step |
|---|---|---|
| M3.3a, M3.3b, M3.3e (motionlil) | — (in flight) | 2 |
| Layout commit (architecture §15) | M3.3a merged | 2 |
| M2.5 Admission parse | — | 3 |
| M2.10 Benchmark contract | — | 3 |
| M2.11 Held-out set, blind ports | — | 3 |
| M2.12 Perf runner, runtime ledger | — | 3 |
| M2.13 Generic ratchet | — (landed 2026-09-27) | 3 |
| M2.14 Frozen baselines | — (must run on 2026-09-27 sources) | 3 |
| M2.15 Calibration corpus | — | 3 |
| M2.4 Interpreter features | — | 3, then ahead of each fact |
| M2.7 Differential | M2.2 | 3 |
| M8.2 A1 (with M11.2, M2.3's item, M9.5's seed, M8.1's imports) | — | 4 |
| M5.1 Edit kernel and DCE | M2.5 | 5 |
| M6.4a Values, exact tier | M5.1 | 5 |
| M7.8a Literal and branch folding | M6.4a | 5 |
| M7.3 Parameters and returns | M5.1 | 5 |
| M7.5a Removal-only inlining | M5.1 | 5 |
| M3.1 first slice | — | 6 |
| M3.2 Family registry | — | 6 |
| M3.4 Public API, shared formation | — | 6 |
| M5.6 Resource accounting | — | 6 |
| M3.5 Budget model | M3.1 first slice, M3.2, M3.4, M5.6, M2.14, M2.15 | 6 |
| M3.9 Caches and lock | M3.5 | 6 |
| M3.7 Environment variables | M3.5 (`LILSCRIPT_SEMANTIC_WORK` goes there) | 6 |
| M4.2 + M10.2 | — | 7 |
| M10.4, M10.7, M10.16 | M10.2 | 7 |
| M4.6 Catalog | — | 8 |
| M10.13, M10.9, M10.15 | M4.6 (catalog spellings) | 8 |
| M10.11 `debug` class | M4.6 | 8 |
| M10.14 checker half | — | 8 |
| M10.12 | Y1 | 8 (after Y1) |
| M4.3, M4.4 | — | 9 |
| M5.2 Annotations (with M8.1) | M4.3, M4.4 | 9 |
| M5.5 Dataflow, cell SSA | — | 9 |
| M5.3a Scheduler | M5.2, M2.5 | 9 |
| M8.2 A2 | M2.5, M4.6, M5.2 | 9 |
| M8.3 Pure printer (first half) | M5.2 | 9 |
| M3.3c, M3.8b | M5.2, M6.2, M9.1's rest | 11 |
| M3.3d | M3.3a | 13 |
| M6.4b Values, range tier | M10.12 (or its coerce-once alternative), M10.9, M5.5 | 10 |
| M6.6 Escape and uniqueness | M5.2 | 10 |
| M6.7 Field facts | M5.2, M10.14 checker half | 10 |
| M6.8 Frequency | M5.2 | 10 |
| M7.1, M7.4 | M5.3a, M5.5 | 10 |
| M7.5 rest, M7.6, M7.7, M7.8 rest | M5.3a, M6.4b, M6.7 | 10 |
| M7.9 | M6.6 | 10 |
| M8.5 Prelude helpers | M5.3a | 10 |
| M7.10 | M8.5 | 10 |
| M8.4 Host modules | M5.1 | 10 |
| M5.3b | each replacement | 10–12 |
| M5.7 Incremental tail | M5.3a | 10 |
| M8.3 second half | M6.4b | 10 |
| M4.5, M11.1, M11.11 | — (M11.11 needs M4.5) | 10 |
| M11.5 Native from the optimized program | M7 | 10 |
| M10.3 Const data | — | 11 |
| M9.1 rest, M9.3, M9.5, M9.7, M9.9, M9.11, M9.12 | M3.5, M5.2, M8.3, M7.5 | 11 |
| M9.2's gzip row, M9.10 | M3.5, M2.15 | 11 |
| M9.6 Property names | M2.10, M10.14 checker half | 11 |
| M9.8 rest | M10.3, M8.5 | 11 |
| M10.19 Specialization | M10.3 | 11 |
| M10.1, M10.8, M10.5, M10.6, M10.10, M10.17 | the core | 12 |
| M10.18 Value structs | M6.6, M7.9; Y2 for `ref` | 12 |
| M10.14 constructibility | Y3 | 12 (after Y3) |
| M10.11 termination, pins | Y4 (termination); M9.1 (pins) | 12 |
| M12.2 other ports | M3.3b | 13 |
| M8.6 Source maps | M3.3a (placement), M5.2 (tree origins) | 13 |
| M8.7 Port-shaped rules | M2.13 (the NO3 allowlist); each rule's replacement | 9–11, continuous |
| M12.6 Runtime parity and pairing | A1 (katexlil idioms); continuous | 4–13 |
| M11.3, M11.4, M11.6–M11.10, M11.12 | M11.5 (M11.8 before M11's exit) | 13 |
| M12.3–M12.5 | everything above | 13 |

---

## Effort and critical path

**The critical path:** batch A1 → floor slice (M5.1, M6.4a, M7.3, M7.5a, M7.8a) → M3.5 → language slice 1 → the core → M4.3/M4.4 → M5.2 → M5.3a → M6.4b → the rest of M7 → M9.1's rest → M12.3.

**It waits on no ruling.** Y1 gates only M10.12, and M6.4b has a designed alternative if the answer is no (coerce-once crossings). Y2 gates only the `ref` removal, Y3 only the constructibility of exported functions, Y4 only whether declared `pure` asserts termination, Y5 only the IIFE default and frames for application scripts (it affects the `comparison/algorithms` script lanes of M7's exit), Y6 only the verdict threshold, Y7 only when per-port keys stop warning and start refusing.

**Size of each step** (batches of 4–8 changes; estimates, recorded so the next re-plan can be measured against them):

| Step | Batches | Runs where |
|---|---|---|
| 2 M3.3a/b, motionlil | in flight | parallel session |
| 3 Tools (M2.5, M2.10–M2.15) | 5–7 | parallel session |
| 4 Batch A1 | 1–2 | this host |
| 5 Floor slice | 3–5 | this host |
| 6 Budget model | 4–6 | this host |
| 7 Language slice 1 | 3–5 | this host |
| 8 The core | 6–9 | this host |
| 9 Machinery | 5–8 | this host |
| 10 Facts and the rest of the floor | 8–12 | this host |
| 11 One choice system | 7–11 | this host |
| 12 The rest of M10 | 5–8 | this host |
| 13 Qualification and native | 8–14 | this host, ports in parallel |

About 55–85 batches after the tools. Recent throughput was four batches in three days with a parallel session (T and I on 2026-09-24, D and N on 2026-09-27); the estimate assumes about one batch per working day on this host.

**Verification cost per batch on this host** (estimates): unit tests and the case runner about 45 minutes; the reference port suites with the family about an hour; the perf lanes in a quiet window about an hour, run first when the burstable host's CPU credits are full. So about 2–3 hours per batch; the per-phase fleet, held-out set and contract cells add about 3–4 hours.

**Checkpoints the owner can expect** (projected dates; they move with the rulings and with host availability):

| Checkpoint | Steps done | Expected scoreboard |
|---|---|---|
| CP1, about 2026-10-04 | 2–4 | katexlil Node steady about 1.06 (from 1.10) and −62 Brotli; micromarklil −283 Brotli and a gzip win; mhchem about −250; posthoglil error-tracking toward 5,100 with its port change; motionlil compiler-written; SWC-pinned bars, the re-verdicted closure ledger, and the frozen compile-time and runtime ledgers |
| CP2, about 2026-10-25 | 5–6 | `comparison/apps` and `algorithms` at or below the old route (558, 2,305: the ratchet's bars) or each remaining case named; level 13 at or below the frozen baseline at `-j 4` on every reference port (katexlil about 3–3.6 s, from 17.8 s); katexlil's package build at or below 14.4 s; levels monotone by test; codec settings configurable |
| CP3, about 2026-12-06 | 7–9 | The `JS.*` census on the reference ports down through the rewrite script; posthoglil error-tracking a win against its shipped bar; katexlil's code lane about −170 more (`instanceof`, spreads); the core in force as the rulings allow; `quiet.rs` deleted; annotations on the tree |
| M12, about 2027-02 | all | No losing cell, no runtime miss, native portable cores at least as fast as their JavaScript |

---

## Next action

M1 is closed; M2.1 is green; the interim release shipped on 2026-09-24 (`docs/reports/2026-09-24-release.md`). M4.1, M5.4, M6.1–M6.3 and M6.5 (computed, not consumed), M7.2, M9.1's first slice, M9.2, M9.3's first family and M9.8 have landed. Version 1 of the design (2026-09-27) sets this order; the dependency graph above lists each step's prerequisites. Steps 2 and 3 run in parallel sessions; the rest run one at a time on this host.

1. **Done: katexlil's release with its font metrics in LilScript** (2026-09-27, `07d90d6`/`f039133`, pinned binary `~/lilscript-work/bin/release-d`): 60,281 Brotli, live on its Pages site; a win over Terser (63,044), SWC (62,399) and `katex.min.js` (62,686). Its code lane still loses (53,238 against SWC's 53,029 as published); the full file wins through M9.8's data encoding. The site's runtime figure is a single noisy draw (M2.12 replaces it).
2. **In flight (parallel session, branch `m3-delivery`): M3.3a, M3.3b and motionlil's M12.2**, from the judged design (`~/lilscript-work/out/design/m3.3/design.md`), then motionlil as one compiler call with `compat.lil` and the nominal rename reverts (`~/lilscript-work/portwork/nominal/`). The compiler's own motionlil output wins (34,395 against the 39,871 bar); making the delivered files compiler-written is the remaining step. **Hand the session architecture §10.6's delta before M3.3b starts** (scores only where the judge needs them, `-j` only, new root statements through `RootRow`, M3.8a's annotation rule). The layout commit (architecture §15) lands right after M3.3a merges, and M3.3b branches from it. M3.8a is due with M3.3b.
3. **In parallel, tools only: M2.5, M2.10–M2.15** (M2.13 landed 2026-09-27, branch `m2-tools`). The admission parse; the benchmark contract with SWC and the Rolldown bundler pinned; the held-out set with its first blind ports; the perf runner with the runtime ledger, the warm-up study, the structural counters and micro perf gates; the generic ratchet with the NO3 allowlist; the frozen baselines (today, while the pre-M1 binary still compiles the sources); the calibration corpus. They are the gates every later step is judged by.
4. **Batch A1: canonical formation without tree analysis** (M8.2 A1 with M11.2, M2.3's item, M9.5's level-0 seed and M8.1's imports; 8 changes; needs nothing new). Pre-registered: katexlil −62 Brotli and Node steady −0.04; micromarklil −283 Brotli and −311 gzip; mhchem −250; zodlil within noise; mobxlil at or below zero. M12.6's katexlil idioms can start after it.
5. **The floor slice** (needs M2.5 and M2.13): M5.1 (edit kernel and DCE), M6.4a, M7.8a, M7.3, M7.5a, each deleting its JS-tree twin or its removing half. Gated per change on the generic ratchet. Interim exit: `comparison/apps` and `algorithms` at or below the frozen old route, or each remaining case named with its M7 owner.
6. **The budget model** (needs M2.14 and M2.15): M3.1's first slice, M3.2, M3.4, M5.6, M3.5 and M3.9. Its exit (monotone levels by construction and by test; level 13 at or below the frozen baseline at `-j 4` and within 1.3× single-threaded; katexlil's package build at or below 14.4 s) gates every new choice family (rule 9).
7. **Language slice 1** (no ruling): M4.2 with M10.2, then M10.4, M10.7 and M10.16, with the `JS.*` rewrite script and port patches in `~/lilscript-work/portwork/`.
8. **The core, in batches keyed to rulings:** without a ruling, M4.6, M10.13 (two batches), M10.9 (two batches, with the development-check lane), M10.15, M10.11's `debug` class and M10.14's checker half; after Y1, M10.12.
9. **The machinery:** M4.3 and M4.4, then M5.2 (absorbing M8.1, with the rule-2 deletions once each column verifies), M5.5, M5.3a with its transitional rules, batch A2, and M8.3's first half. Before it starts, the projected WU table for the facts and rules of steps 9 and 10 (rule 3).
10. **The facts and the rest of the floor:** M6.4b, M6.6, M6.7, M6.8, then M7.1, M7.4, M7.5, M7.6, M7.7, M7.8, M7.9, M8.5 and M7.10, with M5.3b deleting each transitional rule as its replacement lands, until `comparison/apps` and `comparison/algorithms` no longer lose to Closure ADVANCED. M5.7 (incremental tail) and M8.3's second half. M4.5, M11.1, M11.11, and M11.5 directly after M7.
11. **One choice system:** M10.3 first, then M9.1's rest (the structural search and the five family analyses deleted), M9.2's gzip row, M9.3 per site, M9.5, M9.6, M9.7, M9.8's rest, M9.9, M9.10, M9.11, M9.12 and M10.19; M3.3c with M3.8b, whose exit gates the consumer lanes.
12. **The rest of M10:** M10.1, M10.8, M10.5, M10.6, M10.18 (with P2), M10.10, M10.17, M10.14's constructibility after Y3, M10.11's termination after Y4 and its pins.
13. **Qualification and native:** M12.2 for the other goal ports once M3.3b lands (zodlil's `index.cjs`, posthoglil's and katexlil's CJS/UMD re-bundles, micromarklil's "smaller of compiler or esbuild"), M3.3d, M12.6 throughout, M11's remaining tasks (M11.3, M11.4, M11.6–M11.10, M11.12) to M11's exit, then M12.3–M12.5 under the benchmark contract.
