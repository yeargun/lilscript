# 041 — the local rename starves

**Status: OPEN — does `converge_local_names` fail to run on the jquerylil tree build, and is that the
1045 Brotli between the committed artifact and the tree one?**
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
Falsifies: jquerylil < −200, or any port > +100.

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

| variant | rewrites | distinct multi-param headers | raw | gzip9 | brotli11 | tests |
|---|---:|---:|---:|---:|---:|---|
| tree artifact (039 copy) | — | 90 | 83778 | 32540 | 29270 | 6/6 |
| ungated pass over it | | | | | | |
| committed artifact | — | 25 | 83044 | 31530 | 28225 | 6/6 |

## Verdict

<open>

## Next

<open>
