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
| `ident-05` | active | Candidate search can rank an artifact whose names do not resolve (`Se is not defined`). Pre-existing, search-only, and the broken artifacts are the smaller ones. | Every `local_name_reserve` value passes marked's 660-case gate | [notes](notes/ident-05.md) |

## emit — emission validity and directness

| id | status | intent | gate | note |
|---|---|---|---|---|
| `emit-01` | todo | `?:break` was emitted while a stronger receiver coloring was tried. The coloring was backed off; **the emission path that put a statement in expression position was not fixed**. Isolate it independently. | A minimized peephole test that reproduces statement-in-expression, then passes | [notes](notes/emit-01.md) |
| `emit-02` | landed | String / Regex / `JS.encodeURI` lower to JS members, not host trampolines. | Keep the existing regression tests; do not re-derive | [notes](notes/emit-01.md) |
| `emit-03` | landed | `if`/`return` regex picks emit `?:`. | Keep the existing regression tests | [notes](notes/emit-01.md) |
| `emit-04` | landed | Identifier inlining follows JS precedence rather than `\|0` patches. | Keep the existing regression tests | [notes](notes/emit-01.md) |

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
| `search-01` | blocked(ident-05) | Search is on for marked and is what wins the codec fight (10,135 → 9,318 Brotli), but it can still rank an unbound artifact at other settings. | `ident-05` green before search is trusted at every setting | [notes](notes/search-01.md) |
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
