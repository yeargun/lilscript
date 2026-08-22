# Literals: quotes, booleans, pooling

Parent: [index](README.md). Δ is versus that file’s baseline.

## Quote flip

`quotes-single` forces every string literal to single quotes (escapes
internal apostrophes). This is a whole-file style, not a mixed file.

| Corpus | Δ raw | Δ gzip | Δ br5 | Δ br11 |
|---|---:|---:|---:|---:|
| jquery-min | +5 | +13 | +11 | **−16** |
| jquery-src | +5 | +113 | +106 | +117 |
| jquery-lil-raw | −25 | +13 | +8 | +5 |
| jquery-lil-measured | −10 | +8 | +9 | **−23** |
| jquery-lil-min | −7 | +7 | 0 | **+34** |
| glmatrix-js-vite | 0 | 0 | 0 | 0 |
| glmatrix-lil-vite | 0 | 0 | 0 | 0 |
| glmatrix-js-raw | 0 | +1 | −3 | +17 |
| glmatrix-lil-raw | 0 | 0 | −8 | +12 |

Same mutation, opposite Brotli sign on two LilScript jQuery artifacts
(measured −23, later min +34). gzip **never** liked the flip on these
nine files; Brotli sometimes did. That is a context-1 Huffman choice:
after `=` `(` `[` the trees for `"` and `'` differ, and the file
already specialized one of them.

Heuristic: do not default to single quotes because gzip folklore says
so. Flip the **whole** file, score the served codec, keep the winner.
Mixing styles is worse than either pure style (tiny lab).

## Boolean / undefined minify

`bool-minify` turns `true` / `false` / `undefined` into `!0` / `!1` /
`void 0` when the tokenizer sees those keywords (not property names).

| Corpus | Δ raw | Δ gzip | Δ br11 |
|---|---:|---:|---:|
| jquery-min | −65 | −6 | **−9** |
| jquery-src | −952 | −96 | **−55** |
| jquery-lil-raw | −51 | −6 | **−23** |
| jquery-lil-measured | −1044 | −83 | **−123** |
| jquery-lil-min | 0 | 0 | 0 |
| glmatrix-js-vite / lil-vite | 0 | 0 | 0 |
| glmatrix-js-raw | −7 | −3 | **+58** |
| glmatrix-lil-raw | −17 | −9 | **−22** |

The measured LilScript jQuery emit still spells `true` / `undefined`
in many helpers; collapsing them is a 123-byte Brotli win. Vite
gl-matrix already has no `true` tokens left. Unminified JS gl-matrix
**loses 58 Brotli bytes** while saving 7 raw: the file is already
saturated with `0` and `1`, and `!` is a new context.

`bool-expand` (the reverse, including accidental `!0` inside numbers)
is usually a loss. Exception: jquery-lil-measured **−12 Brotli** at
**+465 raw**. Expanding can win when the file already paid for
`true` as a dense token and `!0` is the minority spelling.

Heuristic: `!0` is a **prior**, not a law. Files that still spell
`undefined` fifty times should try `void 0` **and** a shared
`undefined` binding. Those two compete. Score both.

## String pooling

`pool-strings-4x6` lifts the hottest strings of length ≥6 that appear
≥4 times into `var P=[...];` and replaces uses with `P[i]`.
`pool-strings-8x8` is stricter (len ≥8, count ≥8).

| Corpus | pool-4×6 Δ raw / gzip / br11 | pool-8×8 Δ br11 |
|---|---|---:|
| jquery-min | −658 / +78 / **+24** | +15 |
| jquery-src | −658 / +125 / +126 | +22 |
| jquery-lil-raw | −219 / +16 / **−18** | 0 |
| jquery-lil-measured | −1112 / +90 / **−18** | **−34** |
| jquery-lil-min | −1079 / −26 / **−59** | +77 |
| all four gl-matrix | 0 / 0 / 0 | 0 |

Pooling **cuts raw** and often **grows gzip**. Brotli is mixed: the
later LilScript jQuery min wins 59 bytes; jquery.min loses 24. The
declaration plus `P[` `]` traffic only wins when the string is long
enough that a 1-byte name plus a copy of that name beats a 6–20 byte
LZ77 copy of the string itself.

gl-matrix has almost no poolable string literals (numeric kernels).
The pass is a no-op. That is why a global “always pool” flag looks
free on some artifacts and expensive on others.

`audit-no-string-pool` is byte-identical to `audit-lean` on the
checked-in jQuery raw emits: the compiler’s own pool was already
off or empty. See [09](09-audits.md).

Heuristic: pool only **long** strings with **medium** frequency.
Do not pool `"div"` / `"px"`. Do not treat raw decrease as a Brotli
win — jquery-min is the counterexample.
