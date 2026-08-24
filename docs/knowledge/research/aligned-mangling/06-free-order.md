# Orders that cost nothing to change

Parent: [index](README.md). Produced by `pool.mjs` and `layout.mjs`.

Two emission orders are free to the program and not free to the codec: the
order of a literal declaration list, and the order of hoisted function
declarations. Neither changes a byte of raw size. One of them is worth taking.

## String-pool order: a small, free win

LilScript's emit opens with a long `var` of pooled literals:

```js
var …,hg="nodeType",ig="parentNode",jg="type",kg="ownerDocument",lg="display",…
```

325 entries on the public jQuery artifact. Those declarators cannot reference
each other, so any permutation of a contiguous run of them is the same program.
Which one is emitted decides which strings sit next to each other.

Δ against the shipped order:

| Order | jquery-lil-raw gzip | jquery-lil-raw br11 | jquery-lil-min gzip | jquery-lil-min br11 |
|---|---:|---:|---:|---:|
| as shipped | 0 | 0 | 0 | 0 |
| alphabetical | −124 | −12 | −94 | −22 |
| by reversed string (suffix) | −76 | **−50** | −61 | −58 |
| greedy longest-shared-affix chain | −104 | −15 | −101 | **−70** |
| by length, ascending | −44 | +19 | −18 | 0 |
| by length, descending | +53 | +53 | +9 | +17 |
| dictionary-servable first | −53 | −9 | −47 | −15 |

Small, but free, repeatable on both artifacts that have a pool, and never
negative for the two suffix-shaped orders. The two codecs disagree about which
order is best — gzip prefers alphabetical, Brotli prefers suffix or chained —
which is the usual pattern and the usual instruction: score the configured
codec, do not port a gzip winner.

Sorting by *reversed* string wins on Brotli because JavaScript property names
share endings (`…Node`, `…Element`, `…Type`, `…Name`) more than beginnings, and
a shared ending is still a copy.

The `dictionary-servable first` row is the only place in this folder where
ordering by dictionary coverage helps at all, and it is worth −9 to −15 bytes.
That is the true size of the "spell it so the ROM can serve it" effect once
it is measured instead of imagined.

## Function layout: measured, and it loses

Function declarations in one body are hoisted, so permuting them among their
own slots is legal. [01](01-where-the-bits-are.md) says distances are the
largest consumer of bits and that a fifth to a third of full distance codes
land within 64 bytes of a cached distance, which makes "put similar functions
next to each other, or at equal strides" sound obvious.

Δ Brotli-11, all groups of three or more sibling declarations permuted:

| Order | jquery-min | jquery-lil-raw | jquery-lil-min | glmatrix-js-vite | glmatrix-lil-vite |
|---|---:|---:|---:|---:|---:|
| nearest-neighbour similarity | +13 | +44 | +72 | +15 | **−40** |
| size buckets, then similarity | +67 | +107 | +113 | +53 | **−77** |
| by length | +41 | +129 | +177 | +173 | +10 |
| by name | +87 | +234 | +301 | +433 | +430 |
| reversed | +66 | +21 | +189 | +49 | **−45** |

Only glmatrix-lil-vite shows a win, and it is 0.5%. Everything else loses,
which matches [07 layout](../brotli-global-mangle/07-layout.md).

The interesting part is the census of the reordered files: the implicit
distance rate moves by less than one point, and distance bytes move by less
than 1%. **Reordering whole functions does not convert near-miss distances into
cache hits.** The near misses are not "the same shape at a slightly different
offset one function away"; they are structure recurring at fine granularity
inside and across functions, which function-level permutation cannot address.

That is worth recording as a closed door: the distance-cache lever exists, and
whole-function layout is not the handle.

## Heuristic

- Emit pooled literals in reversed-string order under a Brotli cost model.
  Keep it a scored proposal, not a hard rule: the two codecs disagree and the
  win is tens of bytes.
- Do not add more function-layout orders to the beam. Source order plus at most
  one control is already the right budget, and this folder is a second
  independent confirmation.
- Any future "arrange for the distance cache" proposal must show its census
  row. If the implicit-distance rate does not move, the mechanism it claims is
  not the mechanism it has.
