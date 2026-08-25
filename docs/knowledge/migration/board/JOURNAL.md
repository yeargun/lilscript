# Journal

Parent: [board](README.md). Append-only, newest first. One entry per landed change or
recorded verdict. This is cold storage: read it when you are about to redo something.

Format: `## YYYY-MM-DD — <task id> — <one line>` then two or three lines of what
changed, what the gate said, and the commit if there is one.

## 2026-08-25 — ident-08 / search-04 — all five packs, every objective, no performance loss

Second pass on the same lane. Published compiler output against the best-of-breed
minifier (terser-mangle on every pack), all `gate`:

| pack | raw | gzip-9 | Brotli-11 |
|---|---|---|---|
| surveys | −11.8% | −11.4% | −12.2% |
| error-tracking | +3.7% | +1.7% | +1.5% |
| otlp | −0.4% | −1.2% | −1.8% |
| autocapture | −20.0% | −21.4% | −21.7% |
| replay-core | −15.0% | −13.4% | −14.4% |
| **combined** | **−8.0%** | **−9.0%** | **−9.5%** |

otlp flipped from a loss on all three to a win on all three. error-tracking still
loses to Terser but beats Oxc (5,136 vs 5,224) while keeping the eleven class
names Terser and Oxc both mangle away.

Two more miscompiles, both exposed only once the budget was lifted, both the same
class as [ident-06](notes/ident-06.md) — a fold reading a sub-expression's value
as the enclosing expression's ([ident-08](notes/ident-08.md)). A parameter default
that read a *later* formal threw `ReferenceError` on every call omitting it; and
`""!=(e=e.trim())?e:null` folded to `…||null`, returning `true` where the source
returned the string. Three of three miscompiles this lane share that shape.

Three emission defects retired: a regex literal left in statement position (the
pattern was emitted twice), an `async` free function reaching a fused class as
`m(a){return (async(self,a)=>{…})(this,a)}` instead of `async m(a){…}`, and the
identity-finisher declarator. Worth −169 raw / −20 Brotli on error-tracking,
−5 Brotli on replay-core.

Measured and **rejected**: `local_name_reserve` as a beam family (error-tracking
+75 Brotli — proposal work units are scarce and the breadth displaced deeper
families); wider beams (12 → 24 → 48 monotonically worse); raw-growth admission;
and the frequency-derived identifier alphabet, whose −180 estimate is stale
because the emitted assignment is already frequency-ordered
([search-04](notes/search-04.md)).

Performance: same config, before vs after, all five packs — performance score flat
(autocapture −0.1%), deoptimization risk flat, allocation pressure flat, parse
cost −5.3% on error-tracking. The lifted budget *improves* the model further
(parse cost −14%, startup memory −12%). Two direct microbenchmarks could not
resolve a runtime difference in either direction. The cost is compile time:
5.4 s → 10.9 s on error-tracking, which is why the compiler default is unchanged
and the lift lives in `lilscript.deep.toml` / `lilscript.deep-r48.toml`.

Regression: jQuery raw −8,736 / gzip −1,043 / Brotli −857 with compat 6/6; Monaco
and marked byte-identical; zod +32 Brotli, still the price of refusing the
`ident-06` fold. `comparison/cases` unchanged at 617/617, strict wins 617/612/613.
`cargo test --release --lib` 1275 passed, the same 3 pre-existing failures.

## 2026-08-25 — ident-06 / ident-07 / search-03 — error-tracking beats Oxc on Brotli

`@itslil/posthog-js/error-tracking` compiler output went from raw 18,835 / gzip-9
6,808 / Brotli-11 6,200 to raw 15,332 / gzip-9 5,659 / **Brotli-11 5,156**, against
the Oxc hero's 14,662 / 5,700 / 5,224. Brotli and gzip now win; raw does not, and
the LilScript artifact carries the eleven real class names, arities, and prototype
descriptors that the Oxc artifact drops (`Oe`, `Ae`, …). Compat 5/5.

Three things had to be true at once. The candidate-search proposal budget is
artifact-scaled, so an 18 KiB module at level 15 got 96 work units against ~38 beam
families and never reached the naming or class-shape ones ([search-03](notes/search-03.md));
`posthoglil/lilscript.identity.toml` lifts it and the compiler default is unchanged.
Unstarving the search then exposed a miscompile that had been hiding behind the
budget: `parse_single_assignment` accepted a comma sequence as one assignment, so
`if(!m){m={};m.handled=!0}` became `m=m||{},m.handled=!0` and overwrote a
caller-supplied object ([ident-06](notes/ident-06.md)); the same class of bug was
latent in all five shapes of `fold_statement_or_assigns`. Finally, a fused named ES
class makes the port's identity emulation unreachable, so the `new.target` guard,
its declarator, the `name`/`length`/`prototype` finisher, and the
`(function(){var v;v=…;return v})()` husk are now retired
([ident-07](notes/ident-07.md)).

Sibling regression check, same source and config, baseline compiler vs this one,
all `gate`: jQuery raw −8,737 / gzip −1,047 / Brotli −866 with its compat suite
6/6; Monaco byte-identical on all three; marked byte-identical with 5/5; zod
+660 / +99 / **+32** — the price of refusing the unsound fold, and the only
regression. `comparison/cases` unchanged: 617/617, strict wins raw 617, gzip 612,
Brotli 613. `cargo test --release --lib`: 1269 passed, 3 failed — the same three
`codegen_ir_js::tests` cluster/IIFE failures that were already red in this working
tree before any of this.

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
