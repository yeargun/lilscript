# emit-07 — measuring our own output against a minifier, and the folds that came out of it

Parent: [ledger](../LEDGER.md). Status: landed.
Predecessor: [emit-06](emit-06.md), [jquery-01](jquery-01.md).

## Question

[jquery-01](jquery-01.md) left the gap attributed to control-flow shape, from a
census of *our* output against `jquery.min.js`. A census compares two programs
that were never the same program. Ask a sharper question instead: **given the
exact program we emit, how many bytes can a minifier still find in it?** That
isolates compiler quality from port quality and from tree shaking, because both
sides then have the same code and the same dependency closure.

## Method

Run the heroes over LilScript's own final artifact and split the result:

| variant | Δ raw | Δ Brotli |
|---|---:|---:|
| terser reprint only (no compress, no mangle) | −107 | **+3** |
| terser mangle only | −147 | **+22** |
| terser compress only, no mangle | −1,843 | **−529** |
| terser compress + mangle | −1,919 | −460 |

Measured over nine jQuery submodules, 64,397 raw / 25,605 Brotli.

Two of those lines settle old questions. Our **naming is better than terser's**
— it costs 22 Brotli bytes to remangle our output — so [emit-06](emit-06.md)'s
work holds and naming is not where the gap lives. And our **formatting waste is
Brotli-neutral**: 107 raw bytes of `;}` and a stray space before `in`, worth +3.
Everything left is structural.

Then attribute the −529 by switching off one terser transform at a time and
watching how much of the win disappears — marginal contribution in the presence
of the others, which is the number that matters when transforms combine:

| transform disabled | win lost |
|---|---:|
| `unused` | +331 |
| `reduce_vars` | +211 |
| `collapse_vars` | +134 |
| `conditionals` | +84 |
| `sequences` | +80 |
| `if_return` | +29 |
| `booleans` | −10 |
| `comparisons` | −7 |

The last two are negative: terser's boolean and comparison canonicalization make
Brotli *worse* on our output, so our choices there already beat its.

## Current hypothesis

The whole var family is one mechanism — **a value that is read once does not
need a name** — and it does not decompose. Isolated on our output:
`collapse_vars` alone −53, `unused` alone −40, **the pair −134**. Neither half
is worth much; together they are worth more than their sum, because collapsing
is what makes a binding dead and removing it is what pays. Chasing either alone
wastes the work.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | AMD→ESM conversion of jQuery 3.7.1 source, then bundle + minify | `amd2esm.mjs`, esbuild, terser | terser 27,435 Brotli against the official `jquery.min.js` 27,445 — ten bytes apart, so the converted tree is a faithful baseline | diag |
| 2026-08-25 | dead bindings in our own output | acorn walk over nine artifacts | 1–2 unread bindings per module: we do **not** ship dead code, and terser's `unused` is the cleanup half of collapsing, not a garbage finder | diag |
| 2026-08-25 | single-use function bindings | acorn walk, ours vs terser | ours 1–15 per module, terser 0–1: it inlines essentially all of them | diag |
| 2026-08-25 | three folds, nine jQuery submodules | `lilscript-codec --json` | Brotli 25,605 → 25,459, every module improved | gate |
| 2026-08-25 | body fold standalone, four ports | `lilscript-codec --json` | jQuery −66, mobx −6, marked neutral (19 sites, −96 raw), posthog no such shape | gate |
| 2026-08-25 | assignment sinking, `f=E,null==f` → `null==(f=E)` | scripted rewrite over nine artifacts | raw-neutral, **+12 Brotli** — rejected | diag |
| 2026-08-25 | relaxing IR instability propagation | `LILSCRIPT_NO_UNSTABLE_PROPAGATION` ceiling probe | ajax +14, effects +14, callbacks 0 — rejected | diag |
| 2026-08-25 | terminal cleanup budget on a 58KB artifact | `LILSCRIPT_CLEANUP_DEBUG` on mobx | enters with budget 0, occasionally 2 — a scored candidate placed there never runs unless the port raises `terminal_codec_probe_limit` | diag |

## Log

- 2026-08-25 — `src/js_peephole/folds/inline.rs`: move a function bound once and read once to the site that reads it. Creating a function is pure, so the move reorders nothing; identity is not, so the read must sit in the same function scope with no loop between. Gated on identifier mangling, because the move drops the inferred name and a mangling build has already replaced `Deferred` with `B`. Scored, 15 tests. — **LANDED**
- 2026-08-25 — `src/js_peephole/folds/bodies.rs`: `()=>{q();return v}` → `()=>(q(),v)`, and braces off a statement body that is only expressions. Runs in the ordinary pipeline rather than as a scored candidate: it removes syntax that carried no information, which is what the rest of that pipeline does, and the scored slot was unreachable on any artifact over 16KiB whose config does not raise the probe limit. 13 tests. — **LANDED**
- 2026-08-25 — `fold_returned_temporaries` now also accepts the last declarator of a list. `var t=e.length,n=…;return n` was the most common shape left in **every** module measured, and the fold only recognised a store owning its whole statement. 8 further tests. — **LANDED**
- 2026-08-25 — Combining the first two folds on jQuery is worth +2 where the body fold alone is −66 and the inline fold alone −33. Running the ordinary passes over either result costs Brotli as well (−33 becomes −16): they buy raw bytes by specializing shapes. Both facts are why the beam has to choose rather than chain. — **LANDED**

## Next step

Answer the compressibility question, not the statement-shape one. Our artifact is
smaller raw and larger compressed than the hero's, so the next measurement is
which whole-artifact policy costs the ratio — property mangling is the first
candidate, since we mangle and every hero does not, and it is a general question
about every port rather than a fact about jQuery.


## The naming lever, and what it was worth

The question in "Next step" above -- which whole-artifact policy costs the ratio
-- has an answer, and it was not property mangling. Terser remangling our own
jQuery artifact was **+300 raw and −459 Brotli**: bigger, and better compressed.
Isolating the two halves of its mangler, with a plain `a..z` alphabet instead of
its frequency-sorted one, gives −444. So the clever alphabet is worth 15 and the
whole effect is **which name goes where**.

The mechanism is measurable as header convergence:

| | headers | distinct spellings |
|---|---:|---:|
| ours | 312 | 68 |
| `converge_local_names`, `abc` alphabet | 312 | 50 (and +350 Brotli) |
| terser | 309 | 26 |

Two things were wrong with the pass. It converged onto letters the artifact does
not use -- `(a,b)` in a file whose identifiers are `e,t,n,r` -- so every name it
introduced was a byte the codec had not seen. And it decided availability by
"does this spelling appear anywhere inside the scope", which is a sufficient
condition rather than the real one: a nested function binding `e` for itself
blocked `e` for its parent, when binding it again is exactly what shadowing
means. Terser asks instead which names spoken inside a scope resolve somewhere
else, and `BindingResolution` answers that exactly.

With both fixed, jQuery converges to 27 spellings with terser's own
distribution, and the pass is worth more than terser's remangle.

## Corpus result

Matched entries and configs, session start against HEAD, Brotli:

| port | before | after | Δ |
|---|---:|---:|---:|
| jquery | 28,889 | 28,250 | −639 |
| zod | 31,894 | 31,001 | −893 |
| mobx | 16,103 | 15,872 | −231 |
| marked | 9,497 | 9,474 | −23 |
| posthog | 5,781 | 5,779 | −2 |
| monaco | 34,181 | 34,181 | 0 |
| **total** | **126,345** | **124,557** | **−1,788** |

jQuery is +2.93% against `jquery.min.js`, from +5.26%.

## Where the remaining 805 bytes are

By Brotli window, the local half of the gap is gone -- at a 1KB window the
deficit fell from 870 to 143 -- and what is left is mid-range repetition:

| window | gap |
|---|---:|
| 1KB | 143 |
| 4KB | 669 |
| 16KB | 1,023 |
| 64KB+ | 805 |

Two measured budgets account for it. Terser's compression on our current
artifact is worth −345 (var family −135, expression family −126, together −253),
and the per-module scoreboard puts `deferred` +155 and `queue` +236 as programs
that are worse than jQuery's even after terser has done its best to both -- that
is `.lil` source work, not compiler work.

Re-measured and still rejected: hoisting every function's declarations to one
leading `var` list, which produces the 28-gram `function(e,t,n){var r,i,o,a,`
that jQuery repeats five times. It is +279 now, against +277 measured before the
naming work, so convergence did not make it pay.


## Spellings measured and rejected, so they are not retried

Every one of these is the same shape of idea -- "a longer form that repeats
should compress better" -- and the shorthand result proves the idea is real, so
the negatives matter as much as the win. All on the full jQuery artifact,
Brotli, because submodule magnitudes do not transfer (core says +5 where the
whole artifact says -94 for the very same change).

| change | Δ raw | Δ Brotli |
|---|---:|---:|
| object-method shorthand off | **−404** | **−94** (landed) |
| `function_spelling = "function"` (global) | +7,089 | +547 |
| arrows → `function` in property position | +96 | +5 |
| top-level arrow declarators → `function` declarations | +651 | +55 |
| hoist declarations to one leading `var` | −19 | +279 |
| disable structured-closure inlining | −141 | +295 |
| property mangling off | −335 | +24 |
| `local_name_reserve` 0 / 48 | | +216 / +36 |
| rematerialize single-use pure paths | −36 | +83 |
| sink an assignment into its next use | 0 | +12 |

The three arrow experiments settle the `function` question: jQuery's build spells
`function` 604 times against our 168, and closing that gap costs bytes at every
scale it was tried. Our arrows are the better spelling for the code we emit.

The patterns flagged from the marked work -- `(0,function` name suppression,
`W={};W.k=v` instead of an object literal, `Object.assign` -- are almost absent
here: 3, 2 and 0 occurrences against the official build's 0, 0 and 0. They are a
markedlil concern, not a jQuery one.


## The remaining budget, isolated

Measured one transform at a time on the shipped artifact (83,163 raw / 28,206
Brotli), terser with `defaults:false` and no mangling, so each number is that
transform alone rather than its marginal contribution:

| transform alone | Δ raw | Δ Brotli |
|---|---:|---:|
| `collapse_vars` | −512 | −75 |
| `join_vars` | −190 | −65 |
| `unused` | −201 | −44 |
| `conditionals` | | −70 |
| `if_return` | | −55 |
| `sequences` | | −50 |
| all four var-family together | −687 | −134 |
| everything terser has | −1,424 | −345 |
| `hoist_vars` | −17 | **+289** |

Two of these decompose in a way that matters for whoever picks this up.

**Sinking is not where the var family's value is, and it is not the declaration
either.** Rewriting `X = EXPR` to `(X = EXPR)` at the first place that reads it,
across all three populations terser reaches -- a preceding statement, a preceding
`var X = EXPR;`, and a preceding element of a comma sequence -- is **192 sites,
+100 raw and +64 Brotli**. Worse, not better. A narrower version limited to
statement-to-statement is 26 sites and −3.

Nor is it elimination: only 12 declarators in the artifact name something already
declared in the same function, so there is almost nothing to eliminate for free,
and terser does not eliminate them anyway -- it prints `var t;if(!(t=e.nodeType))`
and keeps the declaration.

Reading the diff rather than inferring from the name, `collapse_vars`'s −512 raw
is **block-brace removal** -- `{u=arguments[n];if(u!=null)…}` becoming
`if((u=arguments[n])!=null)…` -- and redundant parentheses around function
expressions. Both are printer-shaped, and reprinting alone measures −107 raw and
**+3 Brotli**. So the raw is real and the codec does not pay for it.

**`join_vars` is mostly terser's printer, not variable joining.** Its −190 raw is
`;}` → `}`, a dropped space in `"length" in`, and redundant parens around
function expressions; the actual joining is four merges. Reprinting alone was
measured at −107 raw and **+3 Brotli**, so the formatting half is free bytes that
the codec does not care about.

## Next step

The var family, and only as one fold: sink the initializer *and* drop the
declaration, which needs the binding to reach an existing declaration list. Worth
−134 with `collapse_vars` alone at −75. Everything else left is 44 to 70 apiece
and several interact, so they should be scored together rather than one at a
time.

## What this does not close

Capturing every remaining terser advantage on our own output is worth about
1.25% of these modules. The jQuery artifact is +5.2% against its hero. So
statement shaping is **not** the residual gap, and the next question is not a
sharper fold. Our output is 2,275 raw bytes *smaller* than terser's and 1,576
Brotli bytes *larger*: compressibility 2.914 against 3.164. Whatever explains
that is a property of the whole artifact, not of any statement in it.

Not to be retried. A compiler change has to pay on JavaScript generally, not on
one port's source style, so imitating jQuery's own idioms is off the table —
hoisting
`var slice=arr.slice` for the 42 `Array.prototype.*.call` sites, or forcing
`function(e,t,n){var r,i,o,a,` headers. Both would buy bytes on jQuery and cost
them elsewhere.
