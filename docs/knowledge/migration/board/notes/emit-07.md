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
