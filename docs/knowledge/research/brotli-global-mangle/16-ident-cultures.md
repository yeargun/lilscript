# Identifier cultures: `a b c` vs `e t n`

Parent: [index](README.md). Counts from [extra.json](extra.json) `stats`.

A minified file is not “using short names.” It is speaking a
**dialect**. The dialect decides which alphabet rewrite has anything
left to do, and which letter a color-merge should land on.

## Top locals

```
jquery.min          e 1835  t 1186  n 872   r 701   ce 548  i 546
jquery-lil-raw      a 2290  b 1766  c 1320  d 1257  e 971   f 689
jquery-lil-min      r 2836  e 2286  t 1609  n 1263  l 980   u 845
glmatrix-js-vite    e 2975  t 1670  n 1283  r 851   a 742   i 720
glmatrix-lil-vite   e 2821  t 2061  n 1596  r 923   a 724   i 630
```

Three dialects on five already-short files:

| Dialect | Who | How it happens |
|---|---|---|
| `e t n r` | jquery.min, both Vite gl-matrix | Terser / frequency-from-`function` folklore |
| `a b c d` | LilScript jQuery raw | canonical base-54, reserved for locals |
| `r e t n` | LilScript jQuery after downstream minify | a second mangler rewrote LilScript’s `a b c` |

The third row is the warning. Passing LilScript emit through another
minifier does **not** preserve the reserved alphabet. It also does
not land on `e t n` first: hottest is `r`. That is why
`hottest-to-e` on jquery-lil-min is **−548 Brotli** — the downstream
pass picked a letter that is merely “short,” not a letter the
Huffman tree already lives in.

## Why LilScript raw has more alphabet headroom

`alphabet-function-letters` Δ br11:

- jquery.min (already `e t n`): **−67**
- jquery-lil-raw (still `a b c`): **−176**
- glmatrix-lil-vite (already `e t n`): **−79**
- glmatrix-js-vite (already `e t n`): **+12**

Moving `a b c` onto `e n i` is a real dialect change. Shuffling a
file that already speaks `e t n` is a small permutation and can
lose.

## `Math` as a “local”

Both gl-matrix Vite files list `Math` in the top-12 “locals”
(322 / 549). The tokenizer treats a bare `Math` as a name that is
not after a dot. Remapping it would be illegal (it is a global).
The harness’s `localName` predicate is a **heuristic**, not a
scope checker. Alphabet passes that walk “hottest short names”
must skip referenced globals. The compiler already reserves them;
this lab’s token rewrite does not.

## Heuristic

Identify the dialect before proposing an alphabet:

- `a b c` → try `e t n` (high expected value)
- `e t n` → try a small permutation and a hostile control; do not
  expect −150
- `r …` after a second minifier → you already lost the reserve;
  do not run a second mangler on size-first LilScript emit

Color-merge ([15](15-color-merge.md)) lands on `e` or `t` in every
winning row. It does not land on `a` unless `a` is already the
file’s keyword letter, which it is not.
