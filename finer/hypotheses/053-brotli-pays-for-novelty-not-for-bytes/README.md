# 053 — Brotli pays for novelty, not for bytes

**Status:** confirmed, with three transforms measured against it (one landed,
two rejected) and a noise floor that retires a class of earlier reasoning.

## The question

The owner's standing instruction is that LilScript must be able to employ any
strategy Oxc or Terser employs, and finer. So: run Terser over our own artifact
and see what it can still find. Anything it finds is a strategy we are missing.

## Method

`scripts/pattern-census.mjs` parses two finished artifacts and counts, for every
input shape Oxc's `peephole/` passes recognise, how many survive in each. A
shape that survives in ours but not in theirs is a strategy we lack — measured,
not guessed.

`scripts/_terser-probe2.mjs` then runs Terser over our artifact one compress
option at a time. Its baseline is **Terser's own `{defaults:false}` output**,
not our file: Terser's printer alone rewrites 5,416 bytes and moves Brotli by
+171, and charging that to every option makes each one look ~171 better than it
is. That mistake is easy to make and it inverts the ranking.

## The noise floor, first

Before reading any of it, `scripts/_noise.mjs` measures what a *semantically
empty* change is worth. It swaps two same-length local names inside a single
function — zero raw change, identical program:

| swap | refs | raw Δ | Brotli Δ |
|---|---|---|---|
| `i`↔`l` | 1505 | 0 | **−78** |
| `e`↔`t` | 68 | 0 | −68 |
| `h`↔`r` | 60 | 0 | **−89** |
| `m`↔`b` | 55 | 0 | **−125** |
| `t`↔`e` | 49 | 0 | −36 |
| `t`↔`n` | 49 | 0 | +30 |

One rename in one function out of ~540 moves Brotli by up to 125 bytes.

The band itself is basin-dependent: re-run on the artifact after the regex fold
landed, the same six swaps span −35 to +57 instead of −125 to +30. Which is the
same warning twice — the number a rename is worth is not stable either, so the
floor is "about ±100", not a constant to subtract.

**A single-build Brotli delta under about 150 bytes is not evidence.** Several
"wins" in this folder's earlier entries, and most of the per-option table below,
sit inside that band. This also explains why individual folds behaved as coin
flips all through folder 050: they were being scored by a measurement whose
noise exceeded their effect.

## What Terser can still find

Marginal against Terser's own no-op output:

| option | raw Δ | Brotli Δ |
|---|---|---|
| collapse_vars | **−629** | −306 |
| conditionals | −190 | −64 |
| unused | −96 | −81 |
| join_vars | −91 | −112 |
| booleans | −23 | −169 |
| inline | −17 | −104 |
| evaluate | −15 | −205 |
| dead_code | −14 | −132 |
| sequences | −1 | −123 |
| side_effects | +2 | −114 |
| *all defaults, passes:3* | −1646 | −572 |

Read with the noise floor in hand, only the top four carry content. The rest
change almost no bytes and move Brotli by more than they change — they are
sampling the naming basin, not compressing anything. `collapse_vars` is the one
real item: folding a single-use temporary into its one reader, so the binding
dies (`t=[],q=[t]` → `q=[t=[]]`; `s=parseFloat(t);if(!s||s<0)` →
`if(!(s=parseFloat(t))||s<0)`).

## Why raw wins do not become Brotli wins

Three transforms, all real reductions in raw bytes, all worth nothing compressed:

| change | raw Δ | Brotli Δ |
|---|---|---|
| `Array.prototype.push.call(a,b)` → `a.push(b)` (26 sites) | −546 | −33 |
| hoist `var _Ap=Array.prototype.push` | −413 | **+5** |
| ambient `new RegExp("…","")` → `/…/` (42 sites) | **−898** | +57 |

All three delete *repeated* text, and Brotli already stores the second and later
copies of a repeated string for a few bits. The artifact is 5,416 raw bytes
larger than Terser's reformat of it and still 171 Brotli bytes smaller, for
exactly this reason.

The corollary runs the other way too, and it is the useful half: what costs
Brotli is content that appears **once**. Which is why a rename — pure novelty,
zero bytes — outweighs every boilerplate deletion above.

## Where we are already finer than Terser

Terser's `evaluate` re-spells our 98 template literals as quoted strings. Doing
that to the finished artifact costs **+222 raw and +78 Brotli**: 96 of the 98
hold a real newline, one byte inside backticks and two as `\n`. Our quote
selection is correct and Terser's would be a regression. Recorded so nobody
"fixes" it later.

## Landed

`normalize_ambient_regex_constructions` (optimizer): `new RegExp(p, f)` on the
ambient `RegExp` global is the `RegexNew` intrinsic, so the pipeline now says
so. Everything downstream already knew that intrinsic — fresh-allocation
classification for escape analysis, and the literal spelling the emitter can
already prove — so the pass is a normalisation, not a new special case. It is
output-neutral without `javascript.regex_literals`, and that flag already
requires the pristine-builtin contract that makes a literal and a constructor
call interchangeable.

katexlil: 44 constructor sites → 2, **−898 raw**, Brotli inside noise. 1230
official tests and 123 snapshots pass; 1688 compiler tests pass.

Fleet, 25 ports on the pool: no port moved outside ±20 Brotli except katexlil
itself, which went +1,856 → **+1,770** against upstream. micromark +20,
remark-parse +16, mdast-util-from-markdown +20, mobx / unified / remark-math
byte-identical. Generic, and it costs nothing anywhere it does not apply.

## Where the gap actually is

With the regex fold landed, katexlil and upstream-Terser's katex are the same
size raw — 276,737 against 276,701 — and we are 2,468 Brotli bytes behind
(65,154 against 62,686). So the gap is not bytes. It is what the bytes are.

`scripts/_split.mjs` cuts each artifact into the three things it is made of:

| region | ours raw | ours Brotli | Terser raw | Terser Brotli | Δ |
|---|---|---|---|---|---|
| string literals | 92,604 | 22,664 | 87,185 | 23,103 | **−439 ours** |
| number literals | 43,588 | 9,718 | 44,425 | 9,770 | **−52 ours** |
| code structure | 140,545 | 30,242 | 145,091 | 27,306 | **+2,936 theirs** |

Our data wins. Our *code* is 4,546 bytes shorter and compresses 2,936 bytes
worse. `scripts/_ident.mjs` splits that again:

| stream | ours raw | ours Brotli | Terser raw | Terser Brotli | Δ |
|---|---|---|---|---|---|
| punctuation + keywords | 75,528 | 13,828 | 79,226 | 12,947 | +881 |
| identifier occurrences | 86,377 | **17,853** | 86,307 | **15,740** | **+2,113** |

The identifier stream is the same size in both and compresses 2,113 bytes
worse in ours. Not longer names — the *same* number of bytes of names, arranged
so they repeat less.

## What that rules out

Measured and dead, so nobody re-opens them:

- **String pooling.** Hoisting repeated literals into short names is an enormous
  raw win and a Brotli loss: top-10 literals −8,613 raw for −138 Brotli, top-60
  **+56**, top-400 **+1,265**. `mangle.pool_strings = false` is correct.
  Brotli already stores the second copy of `"math"` for a few bits; a fresh
  two-character name is new entropy.
- **Function-shape duplication.** Both artifacts have the same share of
  functions whose structural skeleton repeats — 9.6% ours, 9.7% theirs. We are
  not emitting less regular *shapes*.
- **The mangling algorithm alone.** Running Terser's mangler over our own
  finished artifact, with `compress:false` so only names move, is worth −251
  Brotli. Real, reproducible across rebuilds — unlike the swap and canonical
  numbers — and worth having, but an eighth of the 2,113.

The last one is the useful negative: if simply adopting Terser's mangler recovers
an eighth, then most of the identifier-stream gap is not in the names at all. It
is in the *order and repetition of the references* our codegen emits. Two
functions that upstream wrote alike, and Terser therefore spells alike, come out
of SSA spelled differently, because each was specialised against its own
context. Every such decision is locally shorter and globally less repetitive,
which is exactly the trade this folder's title names.

## Next

Two threads, in this order.

1. **Identifier assignment.** −251 is available from Terser's mangler alone, and
   canonicalising local names across 329 scopes to a fixed sequence is 0 raw and
   −67 Brotli. Both say assignment is an unexploited lever; neither Terser nor
   Oxc optimises it for compressed size, so this is where we can be strictly
   finer rather than merely equal. What remains of the 2,113 after naming is a
   property of the reference *order* our code emits, which is a codegen
   question, not a mangler one.
2. **`collapse_vars`** — the only item on Terser's list with real content behind
   it (−629 raw): fold a single-use temporary into its one reader so the binding
   dies. We have SSA and Terser does not, so we should be able to decide this
   with strictly better information than `reduce_vars` gives it.
