# Ledger

Parent: [board](README.md). Protocol: [read order](README.md#read-order--the-context-budget).
Updated 2026-08-19. Orchestrator writes this file; subagents write notes.

The current fight: **typed web syntax → direct JS → global codec win**. The blocking
class is JavaScript identity, not any one library. Candidate search stays off until
that lane is green, because a search that ranks incorrect programs ranks noise.

| Lane | Question it answers |
|---|---|
| `ident` | Does the emitted JS still name the same object the source named? |
| `emit` | Is the emitted JS valid and direct — the spelling a careful human would write? |
| `marked` | Does a real parser port run green and beat the JS heroes? |
| `search` | Can candidate search be trusted back on? |
| `cases` | Does the paired-case corpus still prove compressability? |
| `gate` | Does the release gate actually run green? |

## ident — JavaScript identity (blocks everything)

| id | status | intent | gate | note |
|---|---|---|---|---|
| `ident-01` | landed | A saved value must stay readable across its own update. Two-address coalescing merged a loop phi with its own incoming value, so `prev = cur` before `cur` advanced collapsed into one name. | `cargo test --release --lib keeps_a_saved_previous_value_readable_across_its_own_update` | [notes](notes/ident-01.md) |
| `ident-02` | todo | Make the invariant a **class**, not a call site. `source_receiver_overwritten_between` (`src/js_peephole/folds/copies.rs:781`) has exactly one caller (`:2044`); other folds carry ad-hoc `name_rebound` scans (`copies.rs:117–238`). One shared check, used at every rematerialization site. | Every rematerialization fold routes through the shared check, one test per site | [notes](notes/ident-02.md) |
| `ident-03` | todo | Catch the class without a library: receiver-rebinding shapes in the differential oracle. | `target/debug/lilscript-differential` finds the seeded shapes before a port does | [notes](notes/ident-03.md) |
| `ident-04` | todo | Freeze the bug as paired canonical folders so it can never silently return. | `node comparison/cases/run.mjs --only identity/` green | [notes](notes/ident-04.md) |
| `ident-06` | landed | A comma sequence was accepted as a single assignment, so a guarded branch body became the right-hand side of `\|\|` and ran unconditionally. `parse_single_assignment` (`src/codegen_ir_js.rs`) plus all five shapes of `fold_statement_or_assigns`. | `cargo test --release --lib js_peephole::folds::boolean` and error-tracking compat 5/5 | [notes](notes/ident-06.md) |
| `ident-07` | landed | Retire the identity emulation a fused ES class makes unreachable: the `new.target` guard, its declarator, and the `name`/`length`/`prototype` finisher. | error-tracking Brotli-11 5,156 with compat 5/5; `comparison/cases` unchanged at 617/617 | [notes](notes/ident-07.md) |
| `ident-08` | landed | Two more folds that read a sub-expression's value as the enclosing expression's: a parameter default that read a *later* formal (TDZ `ReferenceError`), and `(name=EXPR)?name:F` folded when the assignment was only an operand. | `cargo test --release --lib parameter_defaults_never_read_a_later_formal assigned_truthy_ternary_needs_the_assignment_to_be_the_whole_condition`; five packs green at both configs | [notes](notes/ident-08.md) |
| `ident-05` | active | Candidate search can rank an artifact whose names do not resolve (`Se is not defined`). Pre-existing, search-only, and the broken artifacts are the smaller ones. | Every `local_name_reserve` value passes marked's 660-case gate | [notes](notes/ident-05.md) |

## emit — emission validity and directness

| id | status | intent | gate | note |
|---|---|---|---|---|
| `emit-01` | todo | `?:break` was emitted while a stronger receiver coloring was tried. The coloring was backed off; **the emission path that put a statement in expression position was not fixed**. Isolate it independently. | A minimized peephole test that reproduces statement-in-expression, then passes | [notes](notes/emit-01.md) |
| `emit-07` | landed | Measure our own artifact against a minifier instead of against another program: naming and formatting are already better than terser's, and the entire remaining advantage is value placement. Three folds landed from it. | Brotli 25,605 → 25,459 across nine jQuery submodules, every module improved; 36 new tests | [notes](notes/emit-07.md) |
| `emit-06` | landed | A total use-to-binding resolver for generated JavaScript, plus converged naming scored on it. The primitive answers `Bound`/`Free`/`Unresolved` for every identifier and fails closed per name. | 23 unit tests; Brotli −76 across nine artifacts with no regression; corpus unchanged | [notes](notes/emit-06.md) |
| `emit-05` | active | LilScript emits fewer raw bytes than `jquery.min.js` and more compressed ones. Cause found and measured — header spelling diversity — but converged naming only reaches a third of it because each function's name pool diverges. | jQuery Brotli 29,770 against the hero's 27,445; converged naming −30 so far | [notes](notes/emit-05.md) |
| `emit-02` | landed | String / Regex / `JS.encodeURI` lower to JS members, not host trampolines. | Keep the existing regression tests; do not re-derive | [notes](notes/emit-01.md) |
| `emit-03` | landed | `if`/`return` regex picks emit `?:`. | Keep the existing regression tests | [notes](notes/emit-01.md) |
| `emit-04` | landed | Identifier inlining follows JS precedence rather than `\|0` patches. | Keep the existing regression tests | [notes](notes/emit-01.md) |

## jquery — the port that loses to its own minifier

| id | status | intent | gate | note |
|---|---|---|---|---|
| `jquery-01` | active | Bottom-up attribution on jQuery: where do the compressed bytes actually go? Convergence recovered 875 Brotli; arrow spelling and header diversity were refuted as causes. | Brotli-11 29,011 shipped, against `jquery.min.js` 27,445 | [notes](notes/jquery-01.md) |

## md — the react-markdown stack

| id | status | intent | gate | note |
|---|---|---|---|---|
| `md-01` | active | The `@itslil` react-markdown stack must beat `terser(official)` on Brotli-11 in both worlds. Bottom-up diagnostic against acorn-tokenized `katex.min.js`: we emit **23.3% more tokens** (127,602 vs 103,449), and the whole raw gap is ~5,850 extra `NAME = …;` statements. `LILSCRIPT_STORE_CENSUS` named the cause — the `unstable` taint, whose largest source is treating every dynamic member read as a coercion hook. New scored assumption **`assume_pure_property_reads`** (Terser's `pure_getters`, default off) plus a precedence fix in `fold_negated_conditional_arms` (`!1 !== x` is `(!1) !== x`) is worth **−6,359 Brotli** across nine packages with every suite green. Refuted along the way: name coalescing (off is **+2,493 worse** and fails 274 tests), typed state bags (+60), operand-fusion run length (byte-identical). **16/16 ports green. Nine win, seven lose (+31,709, from ~86,000).** | Every package at or under its official Terser row, official suites green | [notes](notes/md-01.md) |

## marked — the parser port

| id | status | intent | gate | note |
|---|---|---|---|---|
| `marked-01` | landed | The port lives in `/Users/yeargun/markedlil` (`@itslil/marked`), a separate repository built against this compiler. | `node scripts/build.mjs --compile` in that repo | [notes](notes/marked-01.md) |
| `marked-02` | landed | 660/660 GFM + CommonMark, plus the parse-only official comparison. | `node --test test/compat.test.mjs test/official-parse.test.mjs` — 5/5 | [notes](notes/marked-01.md) |
| `marked-03` | landed | Beat the parse-only heroes on all three metrics: raw 34,015 vs 37,022, gzip-9 10,347 vs 10,930, Brotli-11 **9,318 vs 10,092** (−7.7%). Also 13% faster on documents. | `node scripts/measure.mjs` and `node e2e/run.mjs` in `markedlil` | [notes](notes/marked-01.md) |
| `marked-04` | landed | No host file and no bundler wrapper: the whole npm surface, `marked()` included, is expressed in `entry.lil` and emitted by the compiler. | `src/host.lil` / `src/host.ts` deleted; `dist/marked.esm.js` is banner + compiler output | [notes](notes/marked-01.md) |

## search — candidate search

| id | status | intent | gate | note |
|---|---|---|---|---|
| `search-01` | landed(md stack) | Search is `always` on **15 of 16** markdown ports, each gated by its own suite; `remark-gfm` flips to a win and the stack gains ~5,600 Brotli. Three compiler bugs fixed to get there: `extends push`, a fluent method fused into a class, and a beta-reduced conditional losing its parentheses in a ternary test. `react-markdown` stays `off`: its failure is the real `ident-05` shape — a local shadowing an outer binding a nested body still reads, which the resolver cannot see because the read resolves to the *wrong* binding rather than to none. | 15/16 ports green with `candidate_search = "always"` | [notes](notes/md-01.md) |
| `search-03` | landed | The proposal budget is artifact-scaled (`div_ceil(4)` above 16 KiB), so an 18 KiB module at level 15 gets 96 work units against ~38 beam families and never reaches the naming or class-shape ones. Finding recorded; pack config shipped, compiler default unchanged. | `--explain human` shows the budget exhausted; `lilscript.identity.toml` reaches Brotli-11 5,156 | [notes](notes/search-03.md) |
| `search-04` | landed | Is more search always better, and does the configured `cost_model` win its own metric? No and mostly: wider beams, raw-growth admission and an extra naming family all lose under a fixed work budget; the objective is honored wherever the search converges. | Recorded sweeps; per-pack configs shipped | [notes](notes/search-04.md) |
| `search-02` | todo | After the flip, re-run the corpora and record the deltas — including where search costs raw bytes and wins compressed ones. | Recorded per-corpus deltas under `gate` numbers | [notes](notes/search-01.md) |

## cases — paired-case corpus

| id | status | intent | gate | note |
|---|---|---|---|---|
| `cases-00..06` | ongoing | The existing phase plan, unchanged. This board does not restate it. | Per [migration phases](../README.md#phases) | [notes](notes/cases-00.md) |

## gate — release gates

| id | status | intent | gate | note |
|---|---|---|---|---|
| `gate-01` | todo | The codec contract test fails at HEAD: five runners import Node compressors directly, so `scripts/release-check.sh` is red before this migration touches anything. | `node --test benchmarks/codec-contract.test.mjs` green without weakening the pattern list | [notes](notes/gate-01.md) |

## board — the system itself

| id | status | intent | gate | note |
|---|---|---|---|---|
| `board-01` | landed | This board, its templates, and `scripts/board.mjs`. | `node scripts/board.mjs check` exits 0 | [journal](JOURNAL.md) |
