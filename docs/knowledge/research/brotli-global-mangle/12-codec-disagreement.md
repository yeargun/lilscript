# When gzip, q5, and q11 disagree

Parent: [index](README.md). Every row is a ranking inversion:
gzip-9, Brotli q5, and Brotli q11 do not all have the same sign
versus that file’s baseline. Extracted from [results.json](results.json).

Negative = smaller = better.

## The expensive fights

| Corpus | Mutation | Δ gzip | Δ br5 | Δ br11 |
|---|---|---:|---:|---:|
| jquery-lil-raw | fn-by-length | **−1188** | +167 | +185 |
| jquery-lil-raw | fn-by-prefix | **−1054** | +292 | +162 |
| jquery-lil-raw | fn-reverse | **−178** | +14 | +40 |
| glmatrix-js-vite | fn-by-length | **−234** | +8 | +22 |
| glmatrix-lil-vite | fn-by-length | −127 | **−159** | **+5** |
| glmatrix-lil-vite | fn-reverse | +30 | −83 | **−68** |
| glmatrix-js-vite | fn-reverse | +53 | −14 | −9 |
| jquery-lil-min | alphabet-function-letters | +35 | +6 | **−31** |
| jquery-min | alphabet-rare | +319 | +241 | **−33** |
| glmatrix-js-vite | alphabet-rare | +130 | +85 | **−24** |
| glmatrix-js-vite | alphabet-function-letters | **−74** | −65 | **+12** |
| jquery-min | quotes-single | +13 | +11 | **−16** |
| jquery-lil-measured | quotes-single | +8 | +9 | **−23** |
| jquery-lil-raw | quotes-single | +13 | +8 | +5 |
| jquery-min | var-to-let | −1 | +2 | **+17** |
| jquery-src | var-to-let | −14 | −8 | **+36** |
| glmatrix-js-vite | var-to-let | −10 | +5 | +10 |
| jquery-lil-measured | bool-expand | +28 | +26 | **−12** |
| glmatrix-js-raw | bool-minify | −3 | −1 | **+58** |
| jquery-lil-raw | pool-strings-4x6 | +16 | +10 | **−18** |
| jquery-lil-measured | pool-strings-4x6 | +90 | +84 | **−18** |
| jquery-lil-raw | dot-length-bracket | +82 | +80 | **−12** |
| glmatrix-js-raw | dot-length-bracket | +7 | +15 | **−10** |
| jquery-min | bait-unique | +16 | +17 | **−13** |
| jquery-lil-measured | bait-unique | +14 | +6 | **−51** |
| jquery-lil-min | bait-javascript-type | +21 | +16 | **+148** |
| glmatrix-lil-raw | rotate-short-13 | +66 | +41 | **−16** |
| jquery-lil-measured | rotate-short-13 | +216 | +127 | **−36** |
| glmatrix-lil-raw | alphabet-rare | −1 | +3 | **−81** |
| jquery-lil-measured | const-to-var | −2 | −2 | **−53** |

The full inversion list is `extra.json` → `inversions`.

## Three oracles, three compilers

If you rank candidates with gzip-9 you will:

- ship function-by-length on LilScript jQuery raw (−1.2 KB gzip,
  **+185 Brotli**)
- ship function-letter alphabet on Vite JS gl-matrix (−74 gzip,
  **+12 Brotli**)
- reject single quotes on jquery.min (+13 gzip, **−16 Brotli**)
- reject rare-letter alphabet on jquery.min (+319 gzip, **−33 Brotli**)

If you rank with Brotli **q5** you will:

- ship function-by-length on LilScript gl-matrix Vite (−159 q5,
  **+5 q11**)
- almost miss alphabet-function-letters on jquery-lil-min (+6 q5,
  **−31 q11**)

q5 is a faster Brotli. It is not a monotone approximation of q11.
The encoder’s backward-reference search and block split change
between 5 and 11. Layout is the family that flips most often.

## Why LilScript already has `cost_model`

[gzip-brotli.md](../gzip-brotli.md) says entropy and window
clustering are **candidate-generation** heuristics; the configured
codec of the complete artifact ranks the finalist. This table is
the measurement that slogan is about.

A gzip-optimal layout search is not “close enough” for a
Brotli-default compiler. A q5 probe is a **filter**, not a ranker:
it can drop obviously-dead candidates (uniquify, ROM locals) and
must not choose among the survivors.

## Heuristic

1. Generate with cheap proxies (reuse score, letter histogram,
   gzip, q5).
2. Rank the last handful with **the served codec at the served
   quality**. For this repo that is official Brotli 1.1.0 generic
   q11 `lgwin=22`.
3. When gzip and Brotli disagree, **keep both candidates in the
   beam** until the configured codec scores them. Do not average
   the bytes.
4. Never call a mutation “generally good” from one codec column.
