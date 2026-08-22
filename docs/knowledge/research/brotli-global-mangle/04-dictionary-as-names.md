# ROM words as locals lose at this scale

Parent: [index](README.md). Tiny-file lab suggested `length` / `index` /
`value` as cheap first-occurrence strings. That does **not** transfer to
hot locals on a 70–160 KB minified file.

## What we did

Take the hottest 1–2 character locals and rename them, in frequency
order, to:

- `length`, `index`, `value`, `name`, `type`, `data`
- or `function`, `return`, `undefined`, `prototype`, `document`, `window`

Bindings after `.` and before `:` were left alone. This is still often
illegal (shadowing `length` as a local next to `e.length`). It is a
codec probe.

## Results (Δ br11)

| Corpus | → length/index/value | → function/return/… |
|---|---:|---:|
| jquery-min | +1600 | +2116 |
| jquery-lil-raw | +2364 | +3148 |
| jquery-lil-measured | +1618 | +2311 |
| jquery-lil-min | +2831 | +3664 |
| glmatrix-js-vite | +1362 | +1742 |
| glmatrix-lil-vite | +1748 | +2152 |
| glmatrix-lil-raw | +1378 | +1677 |
| jquery-src | +107 | +262 |

Raw grew 20–50 KB. Brotli paid 1–3 KB. Gzip paid 1–4 KB. There is **no**
corpus where this won.

## Why the tiny-file story dies here

On a 13-byte program, `function a(){return "function"}` can beat a unique
string because the ROM pays the first `function` and LZ77 pays the
second. On jquery.min, `e` already appears thousands of times. Replacing
it with `length`:

- spends 6× raw per use
- the ROM may make the **first** `length` cheap
- every later `length` is a 6-byte LZ77 copy, still worse than a 1-byte
  copy of `e`
- you also **steal** the identifier `length` from `.length`, so the
  encoder’s existing `.length` transform (`"." + length`) now competes
  with a flood of local `length` tokens

The static dictionary is a discount on **rare long literals**. Hot
locals want the shortest legal spelling that the rest of the file
already uses.

## Where ROM words still belong

- one-off string values and object keys with no prior copy
- host / DOM names you must emit anyway (`addEventListener`, `document`)
- glue the file already contains (`function(){`, `return;`, `.length`)

Not as the mangler’s name table.

## Heuristic

Never promote a local to a dictionary word because the word is “free.”
Score it if you want, but the prior is **strong lose** once frequency
is more than a handful. The beam should spend budget on alphabet
permutation and layout, not on `length`-as-`e`.
