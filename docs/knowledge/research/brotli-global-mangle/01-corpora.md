# Corpora

Parent: [index](README.md).

These are complete shipped or compiler-emitted files, not 95-byte snippets.
HTTP would compress each file independently. That is how they were scored.

| Id | File | Raw | gzip-9 | br q5 | br q11 |
|---|---|---:|---:|---:|---:|
| jquery-min | upstream `jquery.min.js` 3.7.1 | 87533 | 30342 | 29763 | 27445 |
| jquery-src | unminified `jquery.js` | 285314 | 83697 | 79680 | 69545 |
| jquery-lil-raw | LilScript public raw emit | 102681 | 38948 | 36306 | 33283 |
| jquery-lil-measured | historical measured LilScript JS | 162971 | 44309 | 43391 | 38560 |
| jquery-lil-min | downstream min of LilScript | 110749 | 42826 | 41380 | 37901 |
| glmatrix-js-vite | npm gl-matrix Vite run | 73505 | 17722 | 16504 | 14330 |
| glmatrix-lil-vite | LilScript gl-matrix Vite run | 68496 | 17192 | 16396 | 14116 |
| glmatrix-js-raw | unminified / raw gl-matrix bundle | 142374 | 22769 | 21093 | 17791 |
| glmatrix-lil-raw | LilScript raw gl-matrix | 116878 | 21413 | 19940 | 17352 |

## What the pairs already say

LilScript gl-matrix Vite is **214 Brotli bytes** under the JS Vite row
(14116 vs 14330) at **5 KB less raw**. That win is mostly shape (typed
kernels, less export glue), not a mangling trick. The mutations below are
applied **on top of each artifact** to see leftover codec headroom.

LilScript jQuery raw is **larger** than jquery.min (33283 vs 27445). The
port is not an eligibility win. It is still the right file to mangle:
it is already identifier-compressed, already full of `function` / `return`,
and it is the compiler’s own alphabet.

Unminified jQuery (69545 br) vs minified (27445 br) is a 42 KB Brotli gap
from whitespace, long names, and comments. After minification the remaining
gaps are tens to hundreds of bytes. That is the regime these pages care
about.

## Why nine files, not one

A mutation that helps jquery.min can hurt gl-matrix Vite. The playbook
only keeps a heuristic when it **repeats on more than one corpus** or
when the disagreement itself is the lesson (gzip vs Brotli, q5 vs q11).
