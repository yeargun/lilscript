# 039 — Terser spells names by frequency

**Status: FALSIFIED — a same-length relabel in Terser's frequency order moves −27 Brotli on
jquerylil (threshold 500); the compiler already spells from frequency alphabets. Terser's real
gain is its *local* rename (−765), and it exists only on the working-tree artifact: on the
committed one Terser's mangle loses 274. Both artifacts throw `ReferenceError` on `animate`.**
Lane: compiler. Objective: brotli. Ports: jquerylil first, then mobxlil and micromarklil. Opened: 2026-09-01.

## Claim

035 measured that Terser's `mangle` pass, run over our *finished* jquerylil artifact, saves 1136
Brotli, and then showed jquerylil already uses all 54 one-character names with a better
most-frequent-60 average than Terser's own (2.65 against 2.85). 036 falsified the shadowing route
(+432). So on jquery the gain is not shorter names. The claim is that it is *which* characters the
names are made of: Terser's `base54` counts the character frequencies of the unmangled output and
hands out digits in that order, so the most-used identifiers are spelled from the bytes the artifact
already repeats most, and both the Huffman side of gzip and Brotli's context modelling pay for that.
We allocate names by use count but spell them from a fixed alphabet. This is a cost-model defect of
exactly the kind objective.md §2 names: a renaming that is raw-neutral and objective-positive.

Confirming number: a **bijective relabeling** of our artifact's mangled identifiers — same lengths,
same scopes, only the spelling permuted to Terser's frequency order — recovers **≥ 500** of the 1136
on jquerylil. Falsifying number: **< 100**, in which case the 1136 is somewhere else in `mangle`
(named function expressions losing names, catch parameters, class identities) and the experiment's
per-class diff says where.

## Read

- `finer/objective.md`, `finer/status.md`, this folder
- `finer/hypotheses/035-where-the-compiler-headroom-is/README.md` — the method that produced the 1136
  and the allocation facts; do not repeat its measurements
- `finer/hypotheses/036-the-fix-exists-and-starves/README.md` Status line only — shadowing is closed
- `src/codegen_ir_js.rs`: `assign_top_level_names`, `LocalNames`, `next_name` — where names are spelled
- Terser's `lib/scope.js` (`base54`, `reset`, `consider`, `sort`) in
  `benchmarks/popular/node_modules/terser` — read the technique, record it in
  `finer/refs/competitor-techniques.md` as PRESENT/ABSENT with file:line

## May touch

- This folder; `finer/out/039/` for scripts and artifacts
- `finer/refs/competitor-techniques.md` (one row)
- `src/codegen_ir_js.rs` only if step 3 is reached, and then behind the cost model, never for raw

## Method

Sizes from `lilscript-codec` only. The fleet A/B is rebuilding every port on this host right now:
do not rebuild ports, do not trust wall clock, and read artifacts from a copy under `finer/out/039/`
taken at the start (`../jquerylil/dist/jquery.esm.js`, `../mobxlil/dist/mobx.esm.js`,
`../micromarklil/dist/micromark.esm.js`).

1. **Reproduce the ceiling.** Terser (`benchmarks/popular/node_modules/terser`) over the copy with
   `compress: false, mangle: { toplevel: true }`, module mode, measured with the codec. Expect about
   −1136 on jquery. Record the exact number and the Terser version.
2. **Pure relabeling.** With acorn (Terser's dependency), collect every binding declared in the
   artifact and its references; build a bijection from our names to new names of the *same length*,
   assigning spellings in Terser's frequency order (count characters of the artifact with identifiers
   removed, order the alphabet by that count, hand digits out by identifier use count) and keeping
   every non-mangled global untouched. Emit, `node --check`, run `../jquerylil`'s tests against it,
   measure. This is the number that confirms or falsifies.
3. **If confirmed**, price the compiler change: spell names from a frequency-ordered alphabet under
   `cost_model = gzip | brotli`, fixed alphabet under `raw`, in `codegen_ir_js.rs`; measure the three
   ports; report the counters. Do not land without the fleet A/B in status.md's terms.
4. **If falsified**, classify Terser's rename diff per identifier class (locals, functions, catch
   params, class names, named function expressions, labels) and price each class by reverting it
   alone; the largest class is the next folder.

```sh
mkdir -p finer/out/039 && cp ../jquerylil/dist/jquery.esm.js ../mobxlil/dist/mobx.esm.js ../micromarklil/dist/micromark.esm.js finer/out/039/
node -e 'const t=require("./benchmarks/popular/node_modules/terser");...'   # step 1, write as finer/out/039/mangle.mjs
./target/release/lilscript-codec --json finer/out/039/*.js
```

Run details — versions, hashes, the scripts and the gates — are in
[measurements.md](measurements.md).

## Result

All sizes from the codec; deltas are Brotli against the row's own "ours". The full table, with
every Terser variant and every alphabet, is in [measurements.md](measurements.md).

| variant | port | raw | gzip9 | brotli11 | delta brotli vs ours | tests |
|---|---|---:|---:|---:|---:|---|
| ours (dist copy, working tree) | jquery | 83778 | 32540 | 29270 | 0 | 6/6 (`animate` throws, below) |
| Terser mangle locals only | jquery | 83375 | 31737 | 28505 | **−765** | |
| Terser mangle (step 1) | jquery | 83863 | 31803 | 28557 | **−713** | 6/6 |
| **relabeled, same lengths, Terser order** | jquery | 83778 | 32549 | 29243 | **−27** | 6/6 |
| ours (committed, 30c000d) | jquery | 83044 | 31530 | 28225 | 0 | 6/6 (`animate` throws) |
| Terser mangle | jquery committed | 83680 | 31721 | 28499 | **+274** | |
| relabeled, Terser order | jquery committed | 83044 | 31551 | 28257 | +32 | 6/6 |
| ours (dist copy) | mobx | 56707 | 17379 | 15708 | 0 | |
| Terser mangle | mobx | 56235 | 16697 | 15056 | −652 | |
| relabeled, Terser order | mobx | 56707 | 17369 | 15727 | +19 | `--check` |
| ours (dist copy) | micromark | 87117 | 30563 | 26097 | 0 | |
| Terser mangle | micromark | 84081 | 29732 | 25397 | −700 | |
| relabeled, Terser order | micromark | 87117 | 30386 | 26061 | −36 | `--check` |

The relabels are raw-identical (6939 / 4895 / 7542 edits, no shorthand expansions). Terser's own
frequency alphabet, isolated inside its own mangle, is worth 38 (jquery tree), −36 (jquery committed:
the fixed alphabet wins), 18 (mobx), 13 (micromark).

**Step 4, the per-class diff on the jquery dist copy.** Format (reprint) +25. Locals only −765.
Adding module-level names to the rename *costs* 52 (−713 against −765; Terser assigns the module
scope in declaration order, so the hottest binding `u` gets a two-character name and raw grows 85).
Spelling 38. Function- and class-holding bindings, by reverting their rename alone, 156. Catch
parameters, labels and class identities were not separable through Terser's options and cannot be
larger than the residual. The class is **locals**, and the mechanism is header convergence:

| artifact | multi-parameter functions | distinct header spellings | most common |
|---|---:|---:|---|
| jquery, working-tree copy | 278 | **90** | `e,t`:48, `e,t,r`:33 |
| jquery, committed | 278 | 25 | `e,t`:126, `e,t,n`:75 |
| Terser over the working-tree copy | 278 | 24 | `e,t`:126, `e,t,r`:76 |
| mobx, ours | 131 | 39 | `a,t`:35 |
| Terser over mobx | 131 | 14 | `e,t`:69 |
| micromark, ours | 92 | 16 | `b,a,i`:41 |
| Terser over micromark | 92 | 11 | `e,r,t`:42 |

On micromark the locals-only rename also drops raw by 3118: our artifact spends 3337 occurrences on
two-character names where Terser's spends 301 — 035's pool finding, unchanged.

## Verdict

**Falsified.** The confirming number was ≥ 500; the relabel recovers 27 on the working-tree copy and
loses 32 on the committed one, 19 on mobx, and gains 36 on micromark. The claim's premise was also
wrong at the mechanism: we do not spell from a fixed alphabet. `src/compiler.rs:3630-3652` proposes
`IdentifierAlphabet::for_code`, `for_code_excluding_binding_characters` and `javascript_keyword`
alongside the configured alphabet under `entropy-aware-mangling` (which jquerylil enables), and
`search_identifier_alphabets` (`:8872`) probes bijective character swaps, all voted by the codec;
`src/js_peephole/rename.rs:197` derives a second frequency alphabet for converged locals. Our
implied order on jquery is `etraniluos…`, Terser's `etrniaols…`; the top eleven one-character names
map onto themselves or a neighbour. Spelling is worth under 40 bytes on every artifact measured,
in both directions.

The 1136 does not reproduce on today's artifacts: Terser's step-1 mangle finds **713** on the
working-tree copy and **loses 274** on the committed artifact. What it finds on the working-tree copy
is its *local* rename, −765, and the committed artifact already has it: 25 header spellings against
Terser's 24, where the working-tree build has 90. The pass that produces those 25 is
`converge_local_names` (`src/js_peephole/rename.rs`), whose caller (`src/compiler.rs:7756-7790`)
runs it on each beam candidate only while `codec_budget.reserve_work_unit()` succeeds — the same
budget sharing 1632fb1 diagnosed and un-gated for `apply_selected_canonical_peephole`, not here —
and which returns immediately if the artifact contains any template literal (`rename.rs:37`;
micromark's copy contains 16, so the pass never runs there). Whether on jquery it never got a work
unit, bailed at `BindingResolution::is_total`, or lost the vote cannot be told from the artifact
without a build; the 1045 Brotli between the committed artifact (28225) and the working-tree one
(29270) is that question.

Found on the way, and larger than anything above: **the shipped jquerylil artifact throws.** Both
copies contain a call to an undeclared global — `,returnHr(r,n,t,e)||` committed, `,returnRn(…)`
working tree — inside `createTween` (the `Animation.tweeners[prop] || []` lookup; `Hr`/`Rn` is
declared nowhere in the file). `$(el).animate({opacity:.5},0)` raises `ReferenceError: returnHr is
not defined` on the committed artifact and `returnRn` on the working-tree one; the port's six compat
tests never animate. `scrollTop(1)` throws a `TypeError` on both. `node --check`, Terser and acorn
all accept the text because `returnHr` lexes as one identifier, so no gate sees it.

## Next

1. **Trace `converge_local_names` on the working-tree jquerylil build** (`LILSCRIPT_TIMING=1`, one
   counter per exit: no work unit at `compiler.rs:7764`, `rewrites == 0`, `is_total` false,
   admission, lost vote). The committed artifact proves the converged form wins here by ~1045
   Brotli; the caller's comment still says "jQuery +43". Give the pass its own ledger as 1632fb1 did
   for the canonical peephole, and let it run past the template-literal bail (`rename.rs:37`),
   which alone hides Terser's −785 on micromark. That is the next folder; it is all of the
   remaining "rename headroom" on the three ports.
2. **Hand the `returnHr` miscompile to the owner.** It is in the committed artifact
   (jquerylil 30c000d) and in every working-tree dist (`esm`, `cjs`, `umd`, `impl`, `raw`). Find
   which fold prints `return` glued to the next identifier inside a comma sequence and then drops
   the now-unreferenced declaration; add the shape to the peephole corpus and an `animate` case to
   the port's tests. status.md's jquerylil row is measured on an artifact that throws.
3. Status.md: replace "Terser extracts −1136 from jquerylil's finished artifact" with the numbers
   above; the standing "rename headroom" lead (open lead 4) closes into item 1.

Artifacts and scripts: `finer/out/039/` (`mangle.mjs`, `relabel.mjs`, `sizes.mjs`, the copies, every
variant, `*.relabel-*.json` stats with alphabets and mappings, `jqtest/` harness).
