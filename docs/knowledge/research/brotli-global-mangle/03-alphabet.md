# The identifier alphabet is not interchangeable

Parent: [index](README.md). All rows keep raw length equal or slightly
smaller (hottest 1–2 char names remapped, no extra characters except
when the new name is shorter).

## Rotate vs retarget

`rotate-short-1` and `rotate-short-13` are permutations of `a–zA–Z_$`.
Same names, same frequencies, different letters. If letters were
interchangeable, Brotli would tie. It does not.

| Corpus | rotate+1 Δ br11 | rotate+13 Δ br11 |
|---|---:|---:|
| jquery-min | +78 | +61 |
| jquery-lil-raw | +53 | +119 |
| jquery-lil-measured | +19 | **-36** |
| jquery-lil-min | +182 | +137 |
| glmatrix-js-vite | **-4** | +52 |
| glmatrix-lil-vite | +35 | +35 |
| glmatrix-lil-raw | +24 | **-16** |

A blind shuffle of an already-good alphabet usually **loses**. Once in a
while a far permutation wins a few tens of bytes (measured LilScript
jQuery −36). That is why alphabet search belongs in the beam: the
winner is not obvious from frequency alone.

## Letters from `function` / `return`

`alphabet-function-letters` assigns the hottest short locals, in
frequency order, to `e n i o t a r s l c u f p`. Those letters already
saturate the Huffman trees because the file is full of `function`,
`return`, `length`, `undefined`.

| Corpus | Δ raw | Δ gzip | Δ br11 |
|---|---:|---:|---:|
| jquery-min | -548 | -65 | **-67** |
| jquery-lil-raw | -171 | -150 | **-176** |
| jquery-lil-measured | -1719 | -245 | **-187** |
| jquery-lil-min | 0 | +35 | **-31** |
| glmatrix-lil-vite | 0 | **-122** | **-79** |
| glmatrix-lil-raw | 0 | -192 | **-162** |
| glmatrix-js-vite | 0 | **-74** | **+12** |
| glmatrix-js-raw | -356 | +12 | +65 |
| jquery-src | -160 | -82 | -11 |

This is the strongest **legal-looking** alphabet heuristic in the folder.
It wins on both jQuery min and LilScript jQuery, and on LilScript
gl-matrix. It **loses Brotli** on Vite gl-matrix while **winning gzip**.
That is a ranking trap: a gzip-first alphabet search would ship +12
Brotli on that file.

## Rare letters

`alphabet-rare` uses `q w x y z j k Q W X Z J K`. Same remapping
structure, hostile letters.

| Corpus | Δ br11 vs baseline | vs function-letters |
|---|---:|---:|
| jquery-min | -33 | worse than −67 |
| jquery-lil-raw | +22 | **+198 vs function-letters** |
| jquery-lil-min | +113 | +144 |
| glmatrix-js-vite | **-24** | rare *beats* function-letters on Brotli |
| glmatrix-lil-vite | -36 | function-letters still better (−79) |

Rare letters are not uniformly bad. On Vite gl-matrix they won Brotli
(−24) while losing gzip (+130). The file is dense numeric kernels:
`function` is common but the hottest locals sit next to `e[0]`, `t[1]`,
digits. The “steal letters from keywords” story is a **prior**, not a
proof.

## Stronger than a full alphabet rewrite

Remapping **only the hottest name** onto `e` or `t` beat this whole
pass on every surgical file (jquery.min `e`→`t` **−288**,
jquery-lil-min `r`→`e` **−548**). That is usually an illegal merge.
See [15](15-color-merge.md) and the dialects in
[16](16-ident-cultures.md).

## Heuristic

1. Generate at least three alphabets: current, `function`-letter order,
   and one hostile / rotated control.
2. Score the **complete artifact** under the served codec.
3. Do not accept an entropy proxy (“more e’s is better”). jquery-min
   rotate+1 has the same e-count family and still lost 78 bytes.
4. Keep the gzip row. If the two codecs disagree, that candidate is
   exactly why `cost_model` exists.
5. After alphabet, try a **legal color collapse** toward `e`/`t`.
   Independent family deltas do not add (alphabet + `let` was worse
   than alphabet alone on LilScript jQuery raw).
