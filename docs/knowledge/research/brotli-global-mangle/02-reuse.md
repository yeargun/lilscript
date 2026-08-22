# Cross-scope reuse is the largest lever

Parent: [index](README.md). Δ is Brotli q11 versus that file’s baseline.
Negative is better.

## Breaking reuse

`uniquify-short` keeps the first use of each 1–2 character local and
renames later occurrences to `a2`, `e3`, … so the codec cannot copy a
one-byte name across functions.

| Corpus | Δ raw | Δ gzip | Δ br11 |
|---|---:|---:|---:|
| jquery-min | +24577 | +18771 | **+14319** |
| jquery-lil-raw | +37520 | +26646 | **+21269** |
| jquery-lil-measured | +34358 | +25592 | **+19035** |
| jquery-lil-min | +50468 | +36908 | **+27679** |
| glmatrix-js-vite | +36872 | +27664 | **+18561** |
| glmatrix-lil-vite | +36572 | +26394 | **+18627** |
| jquery-src (long names already) | +1919 | +2329 | +1800 |

On every minified file this is a catastrophe. The “local optimum” of
giving each function a fresh alphabet is the worst global choice in the
folder.

## Illegal upper bound: everyone is `e`

`collapse-to-e` rewrites every 1-character local to `e`. Bindings collide.
It is not shippable. It answers “how much is reuse worth if we ignore
correctness?”

| Corpus | Δ br11 |
|---|---:|
| jquery-min | **-3730** |
| jquery-lil-raw | **-5720** |
| jquery-lil-measured | **-3551** |
| jquery-lil-min | **-7015** |
| glmatrix-js-vite | **-3937** |
| glmatrix-lil-vite | **-3796** |
| jquery-src | -109 |

Thousands of Brotli bytes sit in “the same short spelling appears in
every function.” LilScript already reserves local names so repeated
helpers share `a`, `b`, `c`. The measurement says: **do not relax that**.
A frequency-ranked unique-per-scope scheme would look cleaner in a dump
and lose 15–25 KB.

## Heuristic

- Candidate generation should **prefer** cross-scope name reservation.
- A proxy that scores “unique names per function, then compress” will
  systematically pick the wrong finalist.
- The illegal `e`-collapse is a ceiling, not a tactic. The legal move is
  “same short names in similar functions,” which the compiler already
  aims at. After that, the leftover is **collapsing two reserved
  colors into one** wherever they do not interfere —
  [15](15-color-merge.md).

## Why unminified jQuery barely moves

`jquery-src` already uses `elem`, `options`, `callback`. There is almost
no 1-character local population, so uniquify / collapse have little to
do. Reuse is a **minified-file** phenomenon. Once you have shortened
names, keeping them aligned across the file is the job.
