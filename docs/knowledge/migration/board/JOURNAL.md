# Journal

Parent: [board](README.md). Append-only, newest first. One entry per landed change or
recorded verdict. This is cold storage: read it when you are about to redo something.

Format: `## YYYY-MM-DD — <task id> — <one line>` then two or three lines of what
changed, what the gate said, and the commit if there is one.

## 2026-08-19 — ident-01 / marked-01..04 — marked beats the minified official parser

Fixed an SSA lost copy in `safe_two_address_phi_pairs` (`src/codegen_ir_js.rs`): a loop
phi could share a JavaScript name with its own incoming value even when its result was
copied into a second phi on the same edge, so `prev = cur; cur = …` collapsed and the
loop ran once. Regression test in `src/compiler.rs`, verified to fail on the unfixed
compiler. Library suite unchanged at 57 pre-existing failures, 0 new.

`@itslil/marked` (`/Users/yeargun/markedlil`) then went 659/660 → 660/660, and after
deleting the esbuild wrapper in favour of the whole npm surface written in `entry.lil`,
and turning candidate search on: raw 34,015 / gzip-9 10,347 / Brotli-11 9,318 against
the Oxc parse-only hero's 37,022 / 10,930 / 10,092 — 7.7% smaller Brotli, 13% faster on
the document suite.

Opened [ident-05](notes/ident-05.md): candidate search can still rank an artifact whose
names do not resolve. Pre-existing, search-only, and the broken artifacts are the
smaller ones, so the search is currently able to win by emitting a program that throws.
A selection-time scope guard was tried and reverted — see the note before rebuilding it.

## 2026-08-19 — board-01 — the board exists

Created `docs/knowledge/migration/board/` (protocol, ledger, journal, notes, briefs,
templates) and `scripts/board.mjs`. Seeded from the identity/marked/search thread so
those findings survive a lost context.

Recorded as facts of this checkout, not as remembered claims: `cargo check` clean
(cargo 1.97.1, exit 0); `source_receiver_overwritten_between` at
`src/js_peephole/folds/copies.rs:781` with one caller at `:2044`; marked's JS heroes
`gate`-measured at raw 35,031 / gzip-9 10,694 / Brotli-11 9,889 (oxc); the marked port
absent from this working tree; `candidate_search` off for the monaco lane.

Not verified here, and therefore not claimed: 660/660, the parse-only hero, and the
state of the marked host-file removal.
