# Bait preambles and host-name glue

Parent: [index](README.md).

## ROM bait vs unique bait

Three 20–34 byte preambles:

- `bait-function-return`: `function(){return;}` — two exact ROM phrases
- `bait-javascript-type`: `var __="type=\"text/javascript\""` — exact
  22-byte dictionary word
- `bait-unique`: same shape as the first, with `function` replaced by
  `qwxkzzzz`

| Corpus | fn-return Δ br11 | js-type Δ br11 | unique Δ br11 |
|---|---:|---:|---:|
| jquery-min | +7 | +7 | **−13** |
| jquery-src | **−18** | +21 | +6 |
| jquery-lil-raw | +10 | +18 | +18 |
| jquery-lil-measured | −29 | −8 | **−51** |
| jquery-lil-min | **−16** | **+148** | +15 |
| glmatrix-js-vite | **−14** | +35 | +90 |
| glmatrix-lil-vite | +23 | −2 | +72 |
| glmatrix-js-raw | +11 | +17 | **−14** |
| glmatrix-lil-raw | −35 | +23 | **−52** |

Tiny-file bait worked because the **entire** program was the first
occurrence. At 70–160 KB the file already contains `function`
thousands of times. A preamble cannot make the 4000th cheaper.

What the table actually says:

1. ROM bait is **not reliably better** than unique padding. Unique
   wins Brotli on jquery.min (−13), measured LilScript (−51), and
   LilScript gl-matrix raw (−52).
2. An “exact dictionary phrase” can be a **disaster**. jquery-lil-min
   `bait-javascript-type` adds 34 raw bytes and **148 Brotli bytes**.
   The first-block Huffman stats get a phrase the rest of the file
   never repeats in that spelling.
3. A 20-byte preamble that “wins” 16–51 bytes is still not a compiler
   tactic. You are rolling first-block dice. The same preamble loses
   on the next artifact.

Heuristic: do not emit a dictionary warmup string. If you want a
cheap first `function`, you already have a `function` keyword in the
first helper.

## `.length` → `["length"]`

`dot-length-bracket` rewrites `.length` / `.prototype` / `.name` /
`.type` to `["length"]` etc. The idea is to spend the quoted-string
ROM transform instead of the `"." + length` transform.

| Corpus | Δ raw | Δ gzip | Δ br11 |
|---|---:|---:|---:|
| jquery-min | +920 | +60 | +50 |
| jquery-src | +924 | +157 | +101 |
| jquery-lil-raw | +1288 | +82 | **−12** |
| jquery-lil-measured | +1568 | +93 | +53 |
| jquery-lil-min | +1320 | +79 | +71 |
| glmatrix-js-vite | +96 | +14 | +53 |
| glmatrix-lil-vite | +84 | +15 | +16 |
| glmatrix-js-raw | +96 | +7 | **−10** |
| glmatrix-lil-raw | +84 | +11 | +7 |

Usually a loss. Two small Brotli wins (−12, −10) against 80–90 gzip
losses. The extra `["` `"]` is two or three raw bytes per use.
jquery.min has hundreds of `.length`. The quoted ROM discount does
not cover the brackets, and you **steal** the word `length` from the
dot-transform neighborhood.

Heuristic: keep `.length`, `.indexOf`, `.prototype`. Bracketize only
when the key is not a valid identifier. Do not bracketize to “feed
the dictionary.”

## Host-name aliases

Writing `document` / `window` / `addEventListener` in full is
correct: they are ROM words you must emit anyway. Aliasing them to a
local (`var d=document`) is the old gzip-32K move. On Brotli q11 + a
file that already says `document` twenty times, the alias is often a
wash: you pay `var d=` and you **remove** copies of a ROM word.

Heuristic: alias a host name only when it appears **very** often in a
**short** window (one hot function) and the alias is a letter already
in the chosen alphabet. Score it. Do not do it as a global pass.
