# 013 — Statement density is jQueryLil's whole remaining gap

**Status: MECHANISM IDENTIFIED AND PRICED at −540 Brotli. Deliberately not taken, because the
available lever is not a legitimate win — the legitimate one is language work.**

## Where this sits

[012](../012-port-scoreboard/README.md) established the real numbers for the **shipped** library
(the committed `jquerylil/dist/jquery.esm.js`, not the in-repo benchmark port that
[008](../008-jquery-compressibility-gap/README.md) originally analysed):

| | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| `jquerylil` HEAD | **83044** | 31530 | 28225 |
| official `jquery.min.js` | 87533 | **30336** | **27445** |
| delta | **−4489 (win)** | +1194 | **+780 (loss)** |

**jQueryLil emits 4489 fewer raw bytes than Terser and still loses on Brotli by 780.** The entire
deficit is compressibility, and the objective declares Brotli, so it is the deficit that counts.

## Measurement

Punctuation and statement-shape census, normalized per KB and expressed as the excess carried by the
83 KB artifact:

| pattern | jQueryLil /KB | official /KB | excess bytes on 83 KB |
|---|---:|---:|---:|
| `,` then `<ident>=` | 14.21 | 6.42 | **+647** |
| total `,` | 52.67 | 39.38 | +1104 |
| total `;` | 15.31 | 7.15 | **+677** |
| `;` then `var`/`let`/`const` | 2.00 | 0.48 | +126 |
| `;` then `<ident>=` | 1.71 | 0.25 | +121 |
| `;` then `if(` | 2.22 | 0.96 | +104 |
| `;` then `return` | 2.22 | 1.58 | +53 |

Compressibility census (from 008, re-run against the shipped artifact):

| metric | jQueryLil HEAD | official |
|---|---:|---:|
| distinct 8-grams / total | 0.703 | **0.643** |
| byte entropy | 5.258 | 5.263 |
| identifier occurrences | 17990 | **16719** |
| distinct identifiers | 1198 | **1056** |

## Findings

1. **We emit roughly twice as many assignment statements as Terser.** Counting statement starts as
   `;` plus comma-sequenced assignments: **2451 for jQueryLil against 1188 for official** — 2.06x.
   This is the single structural difference that survives every other normalization.
2. **It is not a failure to comma-sequence.** A tempting reading of "more semicolons" is that we
   fail to merge statements into comma expressions the way Terser does. The data says otherwise: we
   have more semicolons **and** more commas (52.67/KB against 39.38). We are not sequencing worse —
   we are producing more assignments to sequence. The surplus is upstream of the peephole, in SSA
   destruction.
3. **The identifier count moved much less than the assignment count** (+7.6% identifiers against
   +106% assignments). So the surplus assignments largely target *already-coalesced* names —
   `local_name_coalescing` is working. What is not happening is eliminating the assignment itself.
4. **The entropy gap is closed; the repetition gap is not.** Byte entropy matches official almost
   exactly (5.258 vs 5.263), so the entropy-aware identifier alphabet is doing its job. 8-gram
   uniqueness is still 0.703 against 0.643, which is what a surplus of short, structurally-distinct
   `,x=…` fragments looks like to an LZ matcher.

## The compiler already has a census for this, and it corrects the hypothesis

`LILSCRIPT_STORE_CENSUS=1` reports why SSA destruction gave each value its own statement instead of
nesting it into its consumer. jQuery port, level 13:

| bucket | count | meaning |
|---|---:|---|
| `unstable` | **1681** | evaluating the value is observable, or it depends on something that is |
| `cross_block` | **1468** | the uses live in a different basic block |
| `use_count>1` | 339 | genuinely needs a name |
| `single_use` | **33** | one use, stored anyway |
| `fallthrough_only` | 10 | cross-block over a boundary the emitter invented |

**This falsifies the first version of this hypothesis.** The prediction was that single-use values
were getting their own assignment instead of being folded into their one consumer. There are **33**
such values. The emitter is not leaving obvious single-use inlining on the table, and `repeat`ing
the peephole harder would find nothing — which is consistent with the fold experiment below.

The surplus is `unstable` (48%) and `cross_block` (42%). `unstable` is a transitive closure
(`unstable_values`, `src/codegen_ir_js.rs`): a value is unstable if its evaluation is observable, or
if it *depends on* an unstable value. One observable read poisons everything downstream of it.

## What makes them unstable: the getter-hook assumption

`javascript.assume_pure_property_reads` is the flag that decides whether `o[k]` is treated as a
possible getter call. jQueryLil does not set it. Turning it on (level 13, search off, so the numbers
are comparable to each other rather than to the shipped artifact):

| | `unstable` | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|---:|
| default (`false`) | 1681 | 95403 | 35111 | 31387 |
| `assume_pure_property_reads = true` | **1276** | 94060 | 34512 | **30847** |
| delta | −405 | **−1343** | **−599** | **−540** |

**−540 Brotli — 69% of jQueryLil's entire +780 deficit — from one flag.** The mechanism is exactly
the one this hypothesis predicted, just reached through a different bucket than expected: 405 values
stop being unstable, so 405 assignments collapse into their consumers.

## ...and it would not be a legitimate win

The flag should **not** be set to close this gap, for a reason that is decisive:

`src/config.rs` documents it as *"the way Terser's `pure_getters` does... false by default because a
library cannot assume its callers' objects have no accessors."* **Terser's `pure_getters` is also off
by default**, and the official `jquery.min.js` baseline was produced without it. Enabling it on the
LilScript side would be beating the baseline by relaxing a safety assumption the baseline did not
relax — changing the rules rather than winning under them.

The project has already reached this conclusion elsewhere and it is worth quoting, because it is the
same judgement: `docs/knowledge/language/compressor-surface.md` records for the markdown stack that
`assume_pure_property_reads` is *"a flag, −6 359 Brotli, **not a type**"*.

**The legitimate version of this win is to make it a type.** The same document names the target:
`JsValue` bags where "every `o[k]` is a getter/proxy hook" versus **plain-data objects the compiler
can prove**. When the source says the object is plain data, the compiler *knows* the read is pure
instead of *assuming* it, the 405 values stop being unstable on the same mechanism, and the win is
real. That is objective.md's *"sometimes lilscript code might be the reason why we compile into less
optimized code"* — measured, and now priced at **−540 Brotli on jQueryLil alone**.

## Why the peephole route was not attempted

The obvious lever is `fold_identifier_copies` and the single-use temporary folds. The exact shape was
extracted from the artifact and tested against them:

```js
var Ka=Fg(Gg,ah,Na,ch),La=Fg,_a=Gg;j.call(Za,Ya,Ka,La(_a,ah,X,ah.notifyWith))
```

`La=Fg` and `_a=Gg` are pure copies read exactly once. In isolation the pipeline **does** remove them
(a regression test for that is now in `js_peephole/tests.rs`). In the artifact it does not, because
the enclosing code assigns `Gg` (via `Gg++`) and rebinds `Fg` recursively, and the fold's guards —
`has_prior_identifier_copy_assignment`, `nested_function_assigns_captured_name`, the `var`-hoisting
check — correctly refuse.

Those guards exist because getting this wrong is a **miscompile in a DOM library whose test surface
this repo does not fully own** (`docs/knowledge/evidence/jquery.md`: the semantic gate "does not
prove every upstream jQuery test or plugin"). Loosening them on the strength of one artifact sample,
with no way to run jQuery's own suite, is not a trade worth making. Recorded as specified work rather
than done work.

## What the fix actually is

Ranked by measured value, now that the census has replaced guesswork:

1. **Give the `.lil` source a plain-data object type** so property reads are provably pure without a
   blanket assumption. Worth **−540 Brotli** on jQueryLil — 69% of its gap — and it is the honest
   version of the flag above. Language work, and the largest single measured win found anywhere in
   this workstream.
2. **Attack `cross_block` (1468 values).** Terser reconstructs expressions from an AST where
   definition and use already share a tree; LilScript reconstructs them from a CFG where they do
   not. Note `fallthrough_only` is only **10**, so naive block merging is *not* the lever — this
   needs real expression reconstruction across control flow.
3. **Do not widen single-use sinking.** Only 33 values are in that bucket. This was the original
   plan and the census retired it.

## Adjacent, smaller, and safe

18 real `=void 0` initializers survive (`var e=void 0`, `,x=void 0`), ~7 bytes each, ~126 raw.
`strip_void_initializer_before_write` requires the first use after the declaration to be an
unconditional write; most survivors have a *conditional* first write (`n&&(e=…)`). The safe
generalization is different and simpler: for `let`, dropping `=void 0` is unconditionally
equivalent; for `var`, it is equivalent whenever nothing writes the name **before** the declaration
in the same function. Worth ~126 raw and perhaps 30 Brotli — 4% of the gap, listed for completeness
rather than as a priority.
