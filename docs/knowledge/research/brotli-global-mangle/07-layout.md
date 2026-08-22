# Function order: gzip and Brotli disagree

Parent: [index](README.md).

These mutations only permute top-level `function` declarations (and
loose `function` statements the tokenizer treated as separable). They
do not change names. Raw length is identical. Any Δ is pure codec.

## Results (exact Δ vs baseline)

Raw length is unchanged on every row below.

| Corpus | fn-by-length gzip / br5 / br11 | fn-by-prefix gzip / br5 / br11 | fn-reverse gzip / br11 |
|---|---|---|---|
| jquery-min | **+731 / +239 / +212** | +595 / +311 / +188 | +169 / +65 |
| jquery-lil-raw | **−1188 / +167 / +185** | −1054 / +292 / +162 | −178 / +40 |
| jquery-lil-min | +195 / +233 / +142 | +600 / +533 / +354 | +69 / +217 |
| glmatrix-js-vite | **−234 / +8 / +22** | +1064 / +748 / +408 | +53 / −9 |
| glmatrix-lil-vite | −127 / **−159** / **+5** | +836 / +642 / +384 | +30 / **−68** |
| jquery-src, measured, both gl-matrix raw | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 |

The tokenizer only found separable top-level `function` cuts on some
files. Unminified / ESM-wrapped sources often have one big IIFE or
`export` body, so reorder is a no-op. That is itself a finding: layout
search only matters when the emit actually has many sibling functions.

**Sort-by-length is not “helps gzip, hurts Brotli.”** On jquery.min it
hurts **both** (+731 gzip, +212 Brotli). The fight appears on
LilScript jQuery raw and Vite gl-matrix: gzip loves the sort, Brotli
q11 does not. On LilScript gl-matrix Vite, **q5 agrees with gzip**
(−159) and **q11 disagrees** (+5). A q5 layout oracle would ship a
q11 regression.

`fn-reverse` on LilScript gl-matrix Vite is the opposite fight:
Brotli **−68**, gzip **+30**. Source-adjacent order was already a
local gzip optimum and a Brotli miss.

## Why

Gzip is a 32 KB sliding window with no static dictionary. Putting
similar-length functions next to each other (or grouping by first
identifier) increases the chance that a 200-byte helper is still in
the window when its cousin appears.

Brotli q11 has:

- a larger window (`lgwin=22`, 4 MB)
- a static dictionary that already supplies `function(){`, `return;`,
  `prototype`
- a more expensive backward-reference search that **already** finds
  distant copies

So “put the twins next to each other” is often redundant for Brotli and
**disrupts** whatever locality the original emit had for Huffman
context (which depends on the previous 1–2 bytes at each function
boundary). Reordering can also move a dense `function` cluster away
from a region that was a good dictionary-transform neighborhood.

jquery-lil-raw is the loudest fight: gzip **−1.2 KB**, Brotli **+185**.
A gzip-first layout search would ship a Brotli regression and call it
a win.

## `fn-by-prefix`

Grouping functions whose first identifier starts with the same letter
looks like “help LZ77.” It usually **loses Brotli** more than
sort-by-length. Prefix clustering creates long runs of `function e`
then long runs of `function t`. That can help a weak encoder and
starve a strong one of mixed context that it had already specialized.

## Heuristic

1. Treat function order as a **codec-specific** candidate, not a
   universal minify pass.
2. If the product is Brotli-first, **do not** accept a gzip-winning
   reorder without a Brotli score on the full file.
3. Source order (and LilScript’s emit order) is a strong baseline.
   Distant copies are cheap at q11 + 4 MB window. Reorder budget is
   better spent on alphabet than on topological cleverness.
4. Tiny-file “put copies adjacent” folklore is a gzip-32K story.

## What we did not try

Call-graph clustering, extract-then-inline of 3-line clones, or moving
string tables. Those can still win. The lesson from this page is only:
**do not trust gzip Δ as a Brotli layout oracle.**
