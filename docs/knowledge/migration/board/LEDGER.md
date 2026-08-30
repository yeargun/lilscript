# Ledger

Parent: [board](README.md). Protocol: [read order](README.md#read-order--the-context-budget).
Updated 2026-08-29. Orchestrator writes this file; subagents write notes.

The current fight is replayable large-library evidence, stronger emitted-ABI
validation, legal incumbent recovery, and **arch-05** hygienic target JavaScript.
Execution order: [planned migration](../planned-migration.md). Current snapshot:
[`docs/current-status.md`](../../../current-status.md).
Canonical/codec/semantic gates remain a standing loop, not a phase to redo.
Numbers in landed rows are historical evidence from their notes; they are not
current release measurements. Tracked generated reports own current numbers.
Documentation: [current architecture](../../compilation/current-architecture.md),
[planned architecture](../../compilation/planned-architecture.md),
[objectives](../../compilation/objectives.md),
[decision registry](../../compilation/decision-registry.md),
[compressor surface](../../language/compressor-surface.md).

| Lane | Question it answers |
|---|---|
| `ident` | Does the emitted JS still name the same object the source named? **07.1** |
| `emit` | Is the emitted JS valid and direct — the spelling a careful human would write? |
| `jquery` | Why does the jQuery port lose compressed bytes to `jquery.min.js`? |
| `md` | Does the react-markdown stack beat official Terser on Brotli-11? |
| `marked` | Does a real parser port run green and beat the JS heroes? |
| `search` | Can candidate search be trusted on, and what does it cost? |
| `cases` | Does the paired-case corpus still prove compressability? (00–06) |
| `gate` | Does the release gate actually run green? |
| `board` | Does this board stay loadable? |
| `arch` | 07.2–07.7 and the docs that describe the decision system |

## ident — JavaScript identity (blocks everything)

| id | status | intent | gate | note |
|---|---|---|---|---|
| `ident-01` | landed | A saved value must stay readable across its own update. Two-address coalescing merged a loop phi with its own incoming value, so `prev = cur` before `cur` advanced collapsed into one name. | `cargo test --release --lib keeps_a_saved_previous_value_readable_across_its_own_update` | [notes](notes/ident-01.md) |
| `ident-02` | landed | Make the invariant a **class**, not a call site. Shared peephole `source_receiver_overwritten_between` covers rebound **and** `obj.prop=` (not siblings). Expression cache snapshots before member writes. | `cargo test --lib snapshot_of_a_record_field_survives_a_later_write` | [notes](notes/ident-02.md) |
| `ident-03` | landed | Catch the class without a library: receiver-rebinding shapes in the differential oracle, including invoked captured rebind. | `target/debug/lilscript-differential --cases 8 --compiler target/debug/lilscript` matches evaluator on every JS lane; `cargo test --lib snapshot_of_a_record_field_survives_a_captured_rebind` | [notes](notes/ident-03.md) |
| `ident-04` | landed | Freeze the bug as paired canonical folders so it can never silently return. | `node comparison/cases/run.mjs --only identity/` 5/5 | [notes](notes/ident-04.md) |
| `ident-06` | landed | A comma sequence was accepted as a single assignment, so a guarded branch body became the right-hand side of `\|\|` and ran unconditionally. `parse_single_assignment` (`src/codegen_ir_js.rs`) plus all five shapes of `fold_statement_or_assigns`. | `cargo test --release --lib js_peephole::folds::boolean` and error-tracking compat 5/5 | [notes](notes/ident-06.md) |
| `ident-07` | landed | Retire the identity emulation a fused ES class makes unreachable: the `new.target` guard, its declarator, and the `name`/`length`/`prototype` finisher. | error-tracking Brotli-11 5,156 with compat 5/5; `comparison/cases` unchanged at 617/617 | [notes](notes/ident-07.md) |
| `ident-08` | landed | Two more folds that read a sub-expression's value as the enclosing expression's: a parameter default that read a *later* formal (TDZ `ReferenceError`), and `(name=EXPR)?name:F` folded when the assignment was only an operand. | `cargo test --release --lib parameter_defaults_never_read_a_later_formal assigned_truthy_ternary_needs_the_assignment_to_be_the_whole_condition`; five packs green at both configs | [notes](notes/ident-08.md) |
| `ident-05` | landed | **07.1.** Search must not rank unresolved or wrong-nearer names. Last hole: beta-reduce skipped `[...r]` as a property, so `points(delim)` read the live match. | marked `local_name_reserve` 0/8/12/48 with `candidate_search = always` is 660/660 vs official; react-markdown `always` 93/93 | [notes](notes/ident-05.md) |
| `ident-09` | landed | V-02: make whole-artifact convergence/remapping require total declaration resolution, protect fixed descendant names, and make bounded name generation total. | Targeted rename/remap tests, full release suite, canonical cases, and five-fork G2 | [notes](notes/ident-09.md) |

## emit — emission validity and directness

| id | status | intent | gate | note |
|---|---|---|---|---|
| `emit-01` | landed | Prove the branch-to-conditional path accepts only complete expression statements and rejects `break`, `continue`, `return`, and `throw` in either arm. | Direct fold regressions for all control-transfer forms | [notes](notes/emit-01.md) |
| `emit-07` | landed | Measure our own artifact against a minifier instead of against another program: naming and formatting are already better than terser's, and the entire remaining advantage is value placement. Three folds landed from it. | Brotli 25,605 → 25,459 across nine jQuery submodules, every module improved; 36 new tests | [notes](notes/emit-07.md) |
| `emit-08` | landed | V-03: preserve ordinary-object assignment semantics unless the explicit pristine-prototype contract authorizes literal collection. | Inherited-setter regression, full release suite, canonical cases, and five-fork G2 | [notes](notes/emit-08.md) |
| `emit-06` | landed | A total use-to-binding resolver for generated JavaScript, plus converged naming scored on it. The primitive answers `Bound`/`Free`/`Unresolved` for every identifier and fails closed per name. | 23 unit tests; Brotli −76 across nine artifacts with no regression; corpus unchanged | [notes](notes/emit-06.md) |
| `emit-05` | parked | Header-spelling diversity as the remaining jQuery gap. Resolver landed as emit-06; jquery-01 refuted naming as the residue (IR control-flow). The −602 Brotli “prize” is Terser-on-our-artifact (post-minify, refused). | — | [notes](notes/emit-05.md) |
| `emit-02` | landed | String / Regex / `JS.encodeURI` lower to JS members, not host trampolines. | Keep the existing regression tests; do not re-derive | [notes](notes/emit-01.md) |
| `emit-03` | landed | `if`/`return` regex picks emit `?:`. | Keep the existing regression tests | [notes](notes/emit-01.md) |
| `emit-04` | landed | Identifier inlining follows JS precedence rather than `\|0` patches. | Keep the existing regression tests | [notes](notes/emit-01.md) |

## jquery — the port that loses to its own minifier

| id | status | intent | gate | note |
|---|---|---|---|---|
| `jquery-01` | active | Reproduce the public boundary, then pursue effect-safe value placement, wider array-ness, and migration to landed expression-if/ordinary-object/constructor features. Do not retry post-hoc contraction or naming without new evidence. | Fresh fingerprinted public artifact vs eligible official baseline | [notes](notes/jquery-01.md) |

## md — the react-markdown stack

| id | status | intent | gate | note |
|---|---|---|---|---|
| `md-01` | active | The `@itslil` react-markdown stack must beat `terser(official)` on Brotli-11 in both worlds. Bottom-up diagnostic against acorn-tokenized `katex.min.js`: we emit **23.3% more tokens** (127,602 vs 103,449), and the whole raw gap is ~5,850 extra `NAME = …;` statements. `LILSCRIPT_STORE_CENSUS` named the cause — the `unstable` taint, whose largest source is treating every dynamic member read as a coercion hook. New scored assumption **`assume_pure_property_reads`** (Terser's `pure_getters`, default off) plus a precedence fix in `fold_negated_conditional_arms` (`!1 !== x` is `(!1) !== x`) is worth **−6,359 Brotli** across nine packages with every suite green. Refuted along the way: name coalescing (off is **+2,493 worse** and fails 274 tests), typed state bags (+60), operand-fusion run length (byte-identical). **16/16 ports green. Nine win, seven lose (+31,709, from ~86,000).** | Every package at or under its official Terser row, official suites green | [notes](notes/md-01.md) |

## marked — the parser port

| id | status | intent | gate | note |
|---|---|---|---|---|
| `marked-01` | landed | The port lives in `/Users/yeargun/markedlil` (`@itslil/marked`), a separate repository built against this compiler. | `node scripts/build.mjs --compile` in that repo | [notes](notes/marked-01.md) |
| `marked-02` | landed | 660/660 GFM + CommonMark, plus the parse-only official comparison. | `node --test test/compat.test.mjs test/official-parse.test.mjs` — 5/5 | [notes](notes/marked-01.md) |
| `marked-03` | landed | Historical parse-only comparison passed its recorded size/runtime gate. Current numbers must come from the tracked large-library report, not this ledger. | `node scripts/measure.mjs` and `node e2e/run.mjs` in `markedlil` | [notes](notes/marked-01.md) |
| `marked-04` | landed | No host file and no bundler wrapper: the whole npm surface, `marked()` included, is expressed in `entry.lil` and emitted by the compiler. | `src/host.lil` / `src/host.ts` deleted; `dist/marked.esm.js` is banner + compiler output | [notes](notes/marked-01.md) |

## search — candidate search

| id | status | intent | gate | note |
|---|---|---|---|---|
| `search-01` | landed | Search is `always` on **15 of 16** markdown ports; `remark-gfm` flips to a win. `react-markdown` stays `off` — that failure is ident-05 (wrong nearer binding, not unbound). | 15/16 ports green with `candidate_search = "always"` | [notes](notes/md-01.md) |
| `search-03` | landed | Proposal budget is artifact-scaled; late families starve. Finding recorded; pack config shipped; compiler default unchanged. Becomes 07.6 reserved slices. | `--explain human` shows the budget exhausted; `lilscript.identity.toml` reaches Brotli-11 5,156 | [notes](notes/search-03.md) |
| `search-04` | landed | More search is not always better. Configured `cost_model` is honored where search converges. Wider beams / raw-growth / extra naming family lost under a fixed budget. | Recorded sweeps; per-pack configs shipped | [notes](notes/search-04.md) |
| `search-02` | todo | After ident-05, re-run the corpora and record deltas — including where search costs raw and wins compressed. react-markdown `always` is already 93/93. | Recorded per-corpus deltas under `gate` numbers | [notes](notes/search-01.md) |

## cases — paired-case corpus

| id | status | intent | gate | note |
|---|---|---|---|---|
| `cases-00..06` | ongoing | Standing evidence loop: 54 canonical cases at the current snapshot; catalog + algorithms remain release gates. Generated reports own the count. | `node comparison/cases/run.mjs --canonical-only` green | [notes](notes/cases-00.md) |

## gate — release gates

| id | status | intent | gate | note |
|---|---|---|---|---|
| `gate-01` | landed | Benchmark/publication runners use the canonical codec wrapper; explicit historical/generated exclusions are reviewed by the contract test. | `node --test benchmarks/codec-contract.test.mjs` 10/10 | [notes](notes/gate-01.md) |
| `gate-02` | landed | Add MotionLil direct-output boundaries and MobXLil's true `production-min` lane to the pinned large-library matrix. | Five pinned repositories, fresh semantics, direct artifacts, and zero-regression matrix check | [notes](notes/gate-02.md) |
| `gate-03` | landed | Pin the intended MotionLil multi-entry and MobXLil production-min source/config states in their sibling repositories. | Clean sibling Git objects contain every matrix input | [notes](notes/gate-03.md) |
| `gate-04` | active | V-01 implementation is complete under targeted gates; release promotion waits for the explicitly deferred five-fork checkpoint. | Validator rejection fixtures, incumbent replay, and five-fork G2 | [notes](notes/gate-04.md) |

## recovery — phase-1 legal incumbent recovery

| id | status | intent | gate | note |
|---|---|---|---|---|
| `recover-01` | landed | The provisional MobX production-min regression mixed boundaries; exact `2d2268` → `06b89aa` on the first reproducible true min lane improves 521 Brotli bytes. | Same pinned source/config, fresh semantics, zero selected-metric regression | [notes](notes/recover-01.md) |
| `recover-02` | landed | Direct Motion animate/stagger/lab/export tie exact `2d2268`, mini/full improve, and template-aware naming recovers animateMini to one byte smaller. | Per-boundary direct output, fresh semantics, zero selected-metric regression | [notes](notes/recover-02.md) |
| `recover-03` | landed | Current exact Marked Brotli is 9,506, nine bytes below the best committed historical artifact; raw/gzip remain ineligible. | Pinned compiler/source/config, 660-case semantics, zero Brotli regression | [notes](notes/recover-03.md) |
| `recover-04` | landed | jQuery's exact migration pair ties at Brotli-11 30,275; its independent JavaScript-baseline gap remains `jquery-01`. | Fresh semantics and zero compiler-incumbent regression | [notes](notes/recover-04.md) |

## phase 2 — decision consolidation

| id | status | intent | gate | note |
|---|---|---|---|---|
| `phase2-01` | active | Route existing candidate paths through one materialize/validate/score/guard/compare acceptance primitive without adding alternatives. | Byte-identical focused fixtures, candidate reachability, targeted Rust gates | [notes](notes/phase2-01.md) |

## board — the system itself

| id | status | intent | gate | note |
|---|---|---|---|---|
| `board-01` | landed | This board, its templates, and `scripts/board.mjs`. | `node scripts/board.mjs check` exits 0 | [journal](JOURNAL.md) |

## arch — architecture and documentation

| id | status | intent | gate | note |
|---|---|---|---|---|
| `arch-01` | landed | Separate current and planned compressor architectures; define authority, exactness vocabulary, objectives, and migration without requiring speculative solver machinery. | Current claims cite implementation; planned claims are explicitly unshipped; board check passes | [notes](notes/arch-01.md) |
| `arch-02` | landed | **07.2** One contract/decision registry plus stable source/generated provenance. Beam iterates only the scored set. | Explain answers whether layout is searched, why ABI is fixed, and whether each operation is source-authored or generated | [notes](notes/arch-02.md) |
| `arch-03` | landed | **07.3** Reversible priors: packing, pooling, scalar replacement, and call/closure choices have scored opposites. | Size-first Brotli cartesian seeds include packing and identifier pooling on; `keep-object` admitted; `scalar_replacement_on_and_keep_object_are_both_legal` | [notes](notes/arch-03.md) |
| `arch-04` | landed | **07.4** Proof-marked classes, owner/slot property naming, and lexical/lifted closure environments. | Named-class and identity-free fixtures green; inherited slots collision-free; mutable captures stay lexical | [notes](notes/arch-04.md) |
| `arch-05` | active | **07.5** All final challengers are scored; migrate parsed-text folds to hygienic target JS AST. | Canonical/search-off ranking landed; production no longer reparses output to recover binding identity | [notes](notes/arch-05.md) |
| `arch-06` | landed | **07.6** Deterministic priority slices, starvation reporting, measured joint family, deep release tier, and fingerprinted bundle objective. | Explain names starved families; canonical objectives green; bundle manifest separates selected codec from deployment cost | [notes](notes/arch-06.md) |
| `arch-07` | landed | **07.7** Explicit lowering, ABI validation, constructor export, expression-if/scalar match, ordinary objects, and conservative own-read proof. | Objective/runtime tests green; canonical expression cases green | [notes](notes/arch-07.md) |

## plan — compression migration review

| id | status | intent | gate | note |
|---|---|---|---|---|
| `plan-01` | landed | Fresh architecture/language review of the design-first compression migration. | `node scripts/board.mjs check` and `node scripts/check-doc-links.mjs` | [notes](notes/plan-01.md) |
| `plan-02` | landed | Fresh compiler-algorithm review against current source and the Closure comparison. | `node scripts/board.mjs check` and `node scripts/check-doc-links.mjs` | [notes](notes/plan-02.md) |
| `plan-03` | landed | Fresh verification/corpus review of sequencing, gates, rollback, and parallel execution. | `node scripts/board.mjs check` and `node scripts/check-doc-links.mjs` | [notes](notes/plan-03.md) |
