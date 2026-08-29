# Journal

Parent: [board](README.md). Append-only, newest first. One entry per landed change or
recorded verdict. This is cold storage: read it when you are about to redo something.

Format: `## YYYY-MM-DD — <task id> — <one line>` then two or three lines of what
changed, what the gate said, and the commit if there is one.

## 2026-08-29 — gate-04 — syntax and binding admission before codec

All production compiler scoring paths now run the generated-JavaScript syntax
and binding analyzer before invoking an exact codec. Malformed and unresolved
candidates leave the codec-call counter at zero; full Rust is 1,605/1,605 and a
fresh Marked gzip canary passes 660/660. ABI/property/obligation witnesses remain.

## 2026-08-29 — gate-02 / ident-09 / emit-08 — five-fork G2 landed

The matrix now has 15 immutable boundaries, including seven direct Motion
compiler outputs and current lockfile-safe Marked/MobX lanes. Candidate `7128462`
passes every selected semantic cell; 13 eligible Brotli rows tie incumbent
`06b89aa`. Marked raw/gzip expose an invalid incumbent and pass 660/660 after the
generic local-phi recovery fix. Full release Rust is green. V-01 is next.

## 2026-08-29 — cloud handoff — pushed migration state

LilScript `main` pushed through `182efd0`, MotionLil through merge `8b4fcad`, and
MobXLil through merge `960f2fb`, all without force. The cloud resume queue and
known red gates are in `board/HANDOFF.md`; `gate-02` remains the active task.

## 2026-08-29 — gate-02 / ident-09 / emit-08 — compression migration checkpoint

Closure research, the three-review migration plan, V-02 terminal rename closure,
V-03 ordinary-object setter safety, and six pinned large-library boundaries are
recorded. MotionLil is pinned at `fde1aed`; MobXLil at `820c9a8`; LilScript full
snapshot at `06b89aa`. Generated-JS 524/524, canonical 54/54, codec 10/10, and
five-fork behavior preflights passed; full Rust still has four named red tests.
The migration compiler evidence run remains open. See `board/HANDOFF.md`.

## 2026-08-28 — arch-03 — reversible packing, pooling, and keep-object priors

Size-first Brotli cartesian seeds include `pack_string_arrays` and
`pool_identifier_strings` on. `SCORED_IR_VARIANTS` owns keep-object plus the
named call-graph off-clones; `--explain` lists them. Dedicated scalar-replace
on/off ablation emits both ways. Root TOML exact `identifier-mangling` does not
admit keep-object. Uncommitted.

## 2026-08-28 — arch-02 — one IrJsOptions registry in code

Every emission field is classified; cartesian axes and 45 scored families live in
`src/decision_registry.rs`; the beam iterates that set. `--explain` names layout,
removed size-first families, cartesian axes, scored families, and source/generated
counts. Source `|0` carries `NodeId` + `PreserveJavaScriptBitOrZero`. Uncommitted.

## 2026-08-28 — ident-04 — identity shapes are canonical paired folders

`comparison/cases/canonical/identity/` has write, rebind, computed, captured-rebind,
and saved-loop-phi. `LILSCRIPT=target/debug/lilscript LILSCRIPT_CODEC=target/debug/lilscript-codec
node comparison/cases/run.mjs --only identity/` is 5/5 (strict wins raw=5, gzip=5,
brotli=4). Uncommitted compiler change.

## 2026-08-28 — ident-03 — differential oracle owns receiver-rebinding

`differentialIdentity` now includes invoked captured rebind. Callee-code flush
binds non-reusable cached expressions so a global `Record` snapshot cannot replay
after an IIFE. No-opt configs keep `print`. `--cases 8` matches the evaluator on
optimized, optimizer-disabled, and peephole on/off JS, plus C and native.
`cargo test --lib snapshot_of_a_record_field_survives_a_captured_rebind` and
`snapshot_of_a_top_level_record_field_survives_a_captured_rebind` print `89`.
Uncommitted compiler change.

## 2026-08-28 — ident-02 — property write is the same rematerialization class

Peephole `source_receiver_overwritten_between` refuses `obj.prop=` / `++` /
`delete` without `obj=`; sibling writes still fold. Production still replayed
`a.href??0` after `a.href=` from the expression cache; snapshots now bind first.
`cargo test --lib rematerialization_folds_refuse` (2) and
`snapshot_of_a_record_field_survives_a_later_write` passed. Invoked captured
rebind remains ident-03. Uncommitted compiler change.

## 2026-08-28 — ident-05 — spread operand is not a property

`is_property_identifier` treated `[...r]` as `obj.r` (lexer emits three `.`
tokens). Beta-reduce left the helper parameter, and marked `points(delim)`
read the live `exec` match. After the fix, marked `local_name_reserve`
0/8/12/48 with `candidate_search = always` is 660/660 vs official (0 throws);
react-markdown `always` is 93/93. Uncommitted compiler change.

## 2026-08-28 — arch-01 — current and goal architecture split

`current-architecture.md` now records only implemented pipeline behavior and
known gaps. `goal-architecture.md` defines the proof-constrained `ChoiceGraph`,
per-entity representation families, exact sub-solvers, deterministic anytime
portfolio, Pareto archive, bundle objective, caches, search certificates, and
full pseudocode. Exact codec score is no longer conflated with a global optimum.
No compiler code changed.

## 2026-08-28 — arch-01 extension — semantic firewall and target pipeline

Phase 07 now separates immutable language/ABI/source-lowering contracts from
raw/gzip/Brotli profitability. The first exact-intent gate is live source
`x | 0` versus generated i32 normalization. The plan adds application/library
ABI manifests, aggregate/closure/property representation families, stable
node/binding/property IDs, a hygienic target JS AST, deterministic family-budget
search, and source-attributed explain output. No compiler code changed; ident-05
still blocks search expansion.

## 2026-08-28 — 07 size-first library contract — designed wins, not glue

07 is no longer a refactor checklist. It now states the product: a library
compiled `priority = size-first` must beat Terser/Oxc/Closure ADVANCED on
equivalent supported LilScript, via proof → legal shapes → scored *T*, not
library matchers. Root `lilscript.toml` is named as a test subset (omits
joint-representation, property-mangling, …). Phase-complete is typed-library
non-regression, IR `class` for identity-observed APIs, plain-data without
`assume_*`, and >16 KiB search that finishes. Refuse-list includes post-minify,
pack-local budget lifts, and flipping `export class`. Board notes arch-02/03/04/06/07
point at the contract. No compiler code.

## 2026-08-28 — migration folder refresh — 00–06 standing, 07 on the board

Phases 00–06 no longer read as a start-from-scratch catalog-era plan. Migration
README states current status (47 canonical folders; 07 is the architecture
track). Board gained `arch-02`–`arch-07` mapped to 07.2–07.7, all blocked on
ident-05 except 07.7 (RFCs in parallel) and 07.5/07.6 (blocked on 07.4/07.2).
`search-01` status is `landed` (was illegal `landed(md stack)`). `emit-05`
parked: resolver is emit-06; residual jQuery is jquery-01 / 07.7. ident-05 next
step is the react-markdown wrong-binding shape. `node scripts/board.mjs check`
must pass. No compiler code.

## 2026-08-28 — arch-01 — documentation of implemented vs intended compressor architecture

Knowledge tree now has [architecture](../../compilation/architecture.md),
[decision registry](../../compilation/decision-registry.md), and
[migration 07](../07-global-compressor.md). Clean-context audit confirmed the
glue inventory (irreversible Brotli packing/pooling, no scalar-replace off-clone,
unscored search-off peephole, TypeOnly `export class`, ident-05) and corrected
two doc errors: joint chunk search is layout/name-reserve only; omitted
`length-to-number-elision` can still turn on. No compiler code changed. Next
compiler work remains ident-05, then 07.2.

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
