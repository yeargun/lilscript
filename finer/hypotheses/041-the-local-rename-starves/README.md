# 041 — the local rename starves

**Status: FALSIFIED AS STATED; THE PASS DOES NOT RUN FOR ANOTHER REASON.** The ledger never starves
it (`rename_starved=0`); it rewrites nothing on any jquery candidate because the resolver calls a
second `var t` in one function ambiguous and the pass demands a total resolution (`rename.rs:46-48`).
Narrowed, it is −765 Brotli on the finished artifact and −733 through the pipeline (one binary, one
variable) but ships a syntax error from `fold_common_conditional_arms`, so it is held as
`finer/out/041/narrow-the-bail.patch`, not landed. Closed 2026-09-02.
Lane: compiler. Objective: brotli. Ports: jquerylil first; micromarklil (template-literal bail);
fleet A/B before landing. Opened: 2026-09-01.

## Prior art

Read by the harvest of 2026-09-01; rows in
[refs/competitor-techniques.md](../../refs/competitor-techniques.md) Section E (local renaming).

- **Terser** renames every local in one unconditional whole-program pass at the end
  (`lib/scope.js:808-908`), restarting the short-name counter per scope (`:502, 696-730`), refusing
  only names *enclosed* by this or an inner scope (`:501, 654-667, 706, 723-727`), and naming
  parameters first, by position (`:850-856, 681-694, 895` — no use-count sort anywhere). That is why
  its output has 24 distinct multi-parameter header spellings on jquery: every `(e,t)` is `(e,t)`.
- **Closure** (`RenameVars`, not vendored) assigns names by global frequency with per-scope reuse;
  same effect, same last-pass position. **Oxc**'s mangler is a separate crate not in `finer/refs/`
  (`oxc_minifier-0.147.0/Cargo.toml:103-104`) — a harvest gap to close.
- **LilScript** names locals twice. The IR emitter (`codegen_ir_js.rs:6708-6774`, `local_mangler` +
  `rewind`; order at `:19512-19680`) sorts by use count and colour and never by position, which is
  where the 90 header spellings come from. `js_peephole/rename.rs:33-180` (`converge_local_names`)
  then does what Terser does — parents first, parameters by position, the rest by descending use,
  one alphabet ordered by the artifact's own identifier bytes (`:144, 197-214`) — but it is a scored
  *candidate* behind a gate, not a last pass: `compiler.rs:7752-7790` runs it only inside
  `apply_late_javascript_cleanup`, only with a work unit left in the codec ledger's fair slice
  (`:7763-7764`; the first slice is `min(allowance, 8)` at `:5997-5998`, of which the canonical
  peephole already spends 2), only without a template literal in the artifact (`rename.rs:35-37`,
  zero rewrites otherwise), only when the binding resolution is total (`:46-48`; a duplicate name or
  a destructured parameter makes it not), and only if the codec then votes for it (`:7783`) and it
  survives the beam (`:7985`). The caller's comment still says "jQuery +43" (`:7757`); 039 measured
  the committed artifact, where the pass evidently ran, at −1045 against the tree build.

## Claim

**A.** On the jquerylil tree build the pass never reaches the vote: the ledger has no unit for it, so
the beam's first candidate breaks out at `compiler.rs:7764` (or the whole cleanup exits at `:7681`).
Confirms: an exit counter shows 0 arrivals at `:7783` and the artifact has 90 header spellings.
Falsifies: ≥ 1 candidate reaches `:7783` — then the loss is the vote or the beam, not the ledger.

**B.** Run ungated over the tree artifact, the pass reproduces the committed build's shape.
Confirms: ≤ 30 distinct multi-parameter header spellings and ≥ 700 Brotli saved on
`finer/out/039/jquery.esm.js` (Terser's locals-only rename saved 765; the gap is 1045). Falsifies:
< 300 saved or 0 rewrites — then report which bail fired (`rename.rs:35-37` or `:46-48`) and how
many scopes were unsound or ambiguous.

**C.** (only if A and B confirm) The pass gets its own ledger, as commit 1632fb1 gave the canonical
peephole, and the template-literal bail is lifted or narrowed. Confirms: jquerylil ≥ −600 and
micromarklil ≥ −500 Brotli (Terser: −700 / −785) with no port worse than +50 in the fleet A/B.
Falsifies: jquerylil less than −200, or any port more than +100.

## Read

- `finer/objective.md`, `finer/status.md`, this folder; [039](../039-terser-spells-names-by-frequency/README.md) Result and its `measurements.md`
- `src/js_peephole/rename.rs` (whole file, 535 lines); `src/compiler.rs:7670-7800` and `:5990-6000`, `:4940-4960`
- `src/js_peephole/binding.rs:20-60, 185-200, 445-490, 545-590` for what makes a resolution not total

## May touch

- `src/compiler.rs` (the gate and its counters), `src/js_peephole/rename.rs`, `src/timing.rs` (a
  counter), `src/js_peephole/tests.rs`; this folder; `finer/out/041/`

## Method

A fleet pass may be running on this host (`pgrep -f fleet.mjs`); while it is, do not build any
port and do not trust wall clock. Step B needs no port build; A and C do — wait for the fleet.

1. **B first** (no build): a `#[test]` that reads `LILSCRIPT_RENAME_INPUT` and runs
   `converge_local_names` over `finer/out/039/jquery.esm.js`, writing `finer/out/041/jquery.converged.js`
   and printing the rewrite count and any bail. Count distinct multi-parameter headers before and
   after (`039`'s script or a regex over `(\w+(,\w+)+)=>|function \w*\((\w+(,\w+)+)\)`), measure both
   with `./target/release/lilscript-codec --json`, and check `node --check` plus the 039 harness
   (`finer/out/039/jqtest/`, 6/6) on the converged file.
2. **A** (one jquery build): add a deterministic counter for the pass's exits (`LILSCRIPT_TIMING=1`
   JSON or `--explain json`), build the port with its own config from `../jquerylil` via
   `LILSCRIPT_COMPILER=$PWD/target/release/lilscript node scripts/build.mjs --compile`, read the counter.
3. **C** (only on A ∧ B): the ledger change, suite (`env -u FORCE_COLOR cargo test --release`), and
   `node finer/tools/fleet.mjs` with the fixed binary against the snapshot of the previous pass;
   record every port's delta.

## Result

Tables, offsets and run details: [measurements.md](measurements.md).

**B** (no build). The shipped pass rewrites nothing on either jquery artifact: 0 unsound scopes, 6
ambiguous names, bail at `rename.rs:46-48` — the same six functions in both, each declaring a name
with `var` twice (`var t=e.nodeType;if(!t){var r,t="",a=0;…}`, `for(var r=[],e=t[0],o=0,…)` after
`var o=[]`), which `binding.rs:451,485` records as ambiguous and `is_total()` refuses. With the bail narrowed
to unsound scopes (`narrow-the-bail.patch`, two tests):

| artifact, pass narrowed | rewrites | headers | raw | gzip9 | brotli11 | tests |
|---|---:|---:|---:|---:|---:|---|
| tree (83778 / 32540 / 29270, 90 headers) | 5880 | **25** | 83470 | 31759 | **28505 (−765)** | 6/6, `--check` |
| committed (83044 / 31530 / 28225, 25 headers) | 836 | 25 | 83044 | 31550 | 28233 (+8) | 6/6, `--check` |

Terser's locals-only mangle is 28505 too. mobx bails on `(s,p,...c)`, micromark on 4 templates.

**A** (two jquerylil builds, one source, one variable; the port's config: level 15, `always`,
1536 probes, not level 13's 42–84). Counters, narrowed build: `cleanup_entered` 10,
`cleanup_unbudgeted` **19** (the cleanup exits at `:7681` on an empty ledger), `rename_candidates`
20, `rename_starved` **0**, `rename_ambiguous` **20 of 20** (the shipped bail would have fired on
every one), idle/unparsed/refused/unprobed 0, arrivals at `:7783`: `rename_won` **15** (Σ 8681),
`rename_lost` 5 (Σ 196). So the shipped binary: 20 idle exits, 0 arrivals, 90 spellings — the predicted
number, from the pass's own gate.

Artifacts (raw / gzip9 / brotli11): base (HEAD compiler) 83837 / 32564 / 29304, 91 headers, 6/6;
narrowed as shipped 82596 / 31879 / 28567 but **`node --check` fails** at one site; with its two
parentheses restored by hand 82598 / 31878 / **28571 (−733)**, 40 headers, 6/6.

The site: tree `A?B?(o=…,s=u in e,s&&(r=e[u])):s=o:s=o`, narrowed `A&&B?o=…,s=u in e,s&&(r=e[u]):s=o`.
`fold_common_conditional_arms` merges the identical else-arms and renders the inner consequent via
`strip_parenthesized_range` without restoring the parentheses a sequence needs in a ternary arm
(`src/js_peephole/folds/boolean.rs:2352-2360`), and `analyze_generated_javascript` admits `a?b,c:d`;
`node` refuses it. The converged candidate validates.

**C** — not reached: A is falsified as a mechanism and the narrowed build fails its gate. No fleet A/B;
`step-c-fleet.sh` and `scoreboard.baseline.json` are ready. Suite on the tree as left (counters,
artifact test, shipped bail): 1660 passed, 0 failed.

## Verdict

**Falsified as stated.** The pass does not starve on the ledger: `rename_starved=0`, every candidate
that reaches the loop is charged, probed and voted, and 15 of 20 votes go its way once it runs. It
never ran on jquery — in the tree build *or* the committed one — because `binding.rs:451,485` records
a name a function declares twice as ambiguous, `is_total()` reports that as a non-total resolution,
and `rename.rs:46-48` closes the whole artifact on it. The premise was wrong too: the committed
artifact's 25 header spellings are not this pass's work; they are the emitter's before 2d2268a
(2026-08-28) added `reserve_enclosing_js_bindings` for ident-05, one commit after the compiler
(bcef1c4) that built that dist. Since then each function's pool is reserved against its enclosing
names, the headers diverge to 90–91 spellings, and no later stage re-converges them. That is the
1045.

**B confirmed once the gate is opened.** Over the finished tree artifact the narrowed pass reaches
25 spellings and 28505 Brotli, Terser's locals-only figure exactly, 6/6.
Through the pipeline it is −733 against a one-variable baseline, 40 spellings left because the
remaps after it re-diverge some — and it ships `a?b,c:d`, a latent bug in
`fold_common_conditional_arms` the validator does not catch. Correctness outranks the byte, so the
narrowing is a patch in this folder, not a change in the tree; the tree keeps the exit counters and the artifact test.

The narrowing is sound by construction: every token of an ambiguous name resolves `Unresolved`
(`binding.rs:649-652`), which the pass blocks in every scope containing one (`rename.rs:126-128`),
while the scope's other names resolve exactly; only an unsound scope hides uses of outer bindings.
The other two bails are real: micromarklil's `scan_template` swallows the whole template, `${…}`
included, into one token; mobx's rest parameter `(s,p,...c)` is an unsound scope
(`binding.rs:579-586`). Neither is in this folder's May-touch.

## Next

1. **Fix the fold, then land the patch.** `fold_common_conditional_arms` must re-parenthesize a
   `then_value`/`else_value` whose stripped range holds a top-level comma (anything below
   AssignmentExpression), and the validator must refuse `?a,b:c`. Then `git apply
   finer/out/041/narrow-the-bail.patch`, suite, and the fleet A/B with `step-c-fleet.sh` against
   `scoreboard.baseline.json`: jquerylil about −733 against 29304, micromarklil and mobxlil 0.
2. **Run the pass last, ungated, scored**, as `apply_selected_canonical_peephole` runs the canonical
   rewrite (1632fb1): the finished artifact converges to 25 spellings, the pipeline to 40, because
   the remaps after the beam undo part of it.
3. **The resolver, not the pass, is where the duplicate belongs.** A `var` declared twice in one
   function is one binding; `binding.rs:456-459` marks it ambiguous "because only block scoping could
   tell those bindings apart", which is true of `let` and false of `var`. Merging `var` duplicates
   makes the resolution total with no bail change; `...rest` is one more parameter shape
   (`:579-586`) and unblocks mobx.
4. **micromarklil needs a template-aware lexer**: expressions inside `${…}` as tokens with binding
   identity; Terser's −785 there is this pass's value on that port.
5. **The ledger is exhausted elsewhere**: the cleanup is skipped for an empty ledger 19 times in 29
   on jquerylil at 1536 probes — 036's budget question, measured; it did not decide this claim.
