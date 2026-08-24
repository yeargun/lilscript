# Aligning names across scopes: the pattern is real, the headroom is not

Parent: [index](README.md). Produced by `twins.mjs`, `experiments.mjs` and
`indexed.mjs`.

## The question

> `closure 1 { a[1] = 5; a[2] = 6; b[0] = 1; }`
> `closure 2 { b[1] = 4; b[2] = 7; a[0] = 3; }`
>
> If closure 2 had called its first array `a` as well, the two would spell
> themselves the same way and the codec could copy one from the other.

Correct in principle. A mangler that assigns names per scope has no reason to
make two similar scopes agree, and every `a[1]` it turns into `b[1]` is a copy
it just broke. The question is how much of that is actually on the table after
an ordinary mangler has run.

## Twins up to renaming: there are none

For every function, replace each of its own bindings with a positional token in
first-occurrence order and keep everything else. Two functions with the same
canonical form are the same code modulo naming.

| Corpus | functions | canonical twin groups | functions in them | differ only in names |
|---|---:|---:|---:|---:|
| jquery-min | 483 | 0 | 0 | 0 bytes |
| jquery-src | 581 | 1 | 2 | 0 bytes |
| jquery-lil-raw | 512 | 1 | 2 | 0 bytes |
| jquery-lil-min | 551 | 1 | 2 | 0 bytes |
| glmatrix-js-vite | 325 | 1 | 2 | 0 bytes |
| glmatrix-lil-vite | 318 | 1 | 2 | 0 bytes |
| glmatrix-lil-raw | 348 | 0 | 0 | 0 bytes |

Whole-function twins essentially do not exist in real code, and the handful
that do are **already byte-identical**. There is nothing to align at function
granularity, and the reason is worth keeping: frequency-ordered assignment is
*itself* a canonical order. Two functions with the same shape have the same
frequency profile, so they receive the same names without anyone trying.

## Fragment granularity: an aligner that maximises copyability

Twins are the easy case. The real claim is about fragments — the `a[1]=` inside
otherwise different functions. So: start from a complete legal assignment, walk
the file in source order, and for each function try the other names its own
bindings could legally take, keeping whichever spelling the earlier text can
copy the most of. Two objectives:

- **coverage** — maximise bytes a greedy LZ77 could copy from the prefix;
- **bits** — estimate what a Brotli-shaped coder would pay: order-0 literal
  entropy for the bytes no copy covers, plus a flat price per copy command.

Δ Brotli-11 against baseline, same alphabet in each row so only the choice
changes:

| Corpus | coverage objective | bits objective |
|---|---:|---:|
| jquery-min | +49 | ±0 (no change made) |
| jquery-src | +673 | ±0 |
| jquery-lil-raw | +212 | ±0 |
| jquery-lil-min | +380 | ±0 |
| glmatrix-js-vite | +404 | ±0 |
| glmatrix-lil-vite | +461 | ±0 |
| glmatrix-lil-raw | +517 | ±0 |

Two distinct results, and the second is the important one.

**Maximising copyability actively loses.** Choosing the name that best matches
earlier text spreads the name distribution: a letter is picked because it fits
*here*, not because the file is already full of it. The bytes a longer copy
saves are smaller than the bytes a flatter literal distribution costs.

**Under a codec-shaped cost model the greedy assignment is already optimal.**
The bits objective made **zero** changes on every corpus: at every decision
point, no legal alternative name scored better than "the first available name
in the alphabet". The search space is not being under-explored; the local
optimum is where a normal mangler already stands.

## The `a[1]` case, measured directly

How much of each file is `name[constant]`, and what would perfect alignment of
those receivers be worth? The ceiling is measured by renaming **every** indexed
receiver to one letter — illegal, a gravity probe in the sense this repository
already uses, and nothing legal can beat it.

| Corpus | `name[k]` sites | share of raw | repeated (name, index) pairs | illegal ceiling, Δbr11 |
|---|---:|---:|---:|---:|
| jquery-min | 136 | 0.63% | 29 of 40 | −61 |
| jquery-src | 142 | 0.54% | 28 of 65 | −87 |
| jquery-lil-raw | 116 | 0.46% | 24 of 48 | −96 |
| jquery-lil-min | 368 | 1.35% | 42 of 68 | −57 |
| glmatrix-js-vite | 3,623 | **20.31%** | 67 of 108 | −395 |
| glmatrix-lil-vite | 3,187 | **19.06%** | 71 of 117 | −327 |
| glmatrix-lil-raw | 3,187 | 11.21% | 102 of 174 | −382 |

On the jQuery family the whole family of ideas is bounded by about 60–96
Brotli bytes, and that bound is illegal. On gl-matrix — vector kernels
indexing arrays, exactly the shape the question describes — indexed access is a
fifth of the file and the ceiling is 327–395 bytes, 2.3–2.8%.

But look at which pairs repeat: `e[0]`×282, `e[1]`×285, `t[0]`×206, `t[1]`×208.
The hot receivers are **already aligned onto the same two letters** across
hundreds of functions, by nothing more than frequency-ordered mangling. The
remaining ceiling is the interference case — two receivers live at the same
time, so they cannot share a name — and collapsing those is exactly the illegal
merge the playbook already catalogued in
[15 color-merge](../brotli-global-mangle/15-color-merge.md).

## What is left, and where it went

The legal version of "make them share a name" is not alignment. It is
**allocating fewer names in the first place**, so that two live ranges which do
not interfere get the same spelling instead of two spellings. That is
[05](05-concentration.md), and on LilScript's own artifact it is worth −801
Brotli bytes — an order of magnitude more than the aligned-naming ceiling on
the same file.

## Heuristic

- Do not build a similarity-driven or copy-maximising name assigner. Measured:
  it loses on every corpus, and a bit-cost objective declines to make any of
  its moves.
- Frequency order already aligns twins. Keep it, and keep its tie-break stable
  (first use), so that two functions with the same shape cannot drift apart.
- The `a[1]` intuition is worth chasing only where indexed access is a large
  share of the file, and there the money is in the interference graph, not in
  the spelling.
