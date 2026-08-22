# Declarations: `var` / `let` / `const`

Parent: [index](README.md).

The keywords are not interchangeable to Brotli even when a script
would accept the swap. Bulk `var` → `let` is a semantics lie for
`var` hoisting / `let` TDZ. It is still the right codec probe:
three letters, different ROM transforms, different shared letters
with `function` / `return`.

## `var` → `let`

Raw length is unchanged (both are 3 letters).

| Corpus | Δ gzip | Δ br5 | Δ br11 |
|---|---:|---:|---:|
| jquery-min | −1 | +2 | **+17** |
| jquery-src | −14 | −8 | +36 |
| jquery-lil-raw | −54 | −57 | **−56** |
| jquery-lil-measured | −54 | −50 | **−92** |
| jquery-lil-min | −84 | −75 | **−44** |
| glmatrix-js-vite | −10 | +5 | +10 |
| glmatrix-lil-vite | −51 | −46 | **−65** |
| glmatrix-js-raw | −2 | +2 | **−21** |
| glmatrix-lil-raw | −73 | −49 | **−68** |

LilScript jQuery and LilScript gl-matrix **want `let`**. Upstream
jquery.min **wants `var`**. gzip on jquery.min is almost a tie (−1);
Brotli is not. The files already contain the winning keyword as a
dense token.

`let` is ROM `lets` omit-last-1. `var` is ROM `vary` / `vars`. Both
are cheap as **first** occurrences. After that it is copy frequency
and which letters (`e t` vs `a r`) already saturate the Huffman
trees because of `function` / `return` / `length`.

## `const` → `var`

Most files have no `const` (Δ = 0). Where they do:

| Corpus | Δ raw | Δ gzip | Δ br11 |
|---|---:|---:|---:|
| jquery-lil-measured | −18 | −2 | **−53** |
| glmatrix-js-raw | −126 | −19 | −1 |
| glmatrix-lil-raw | −126 | −18 | **−53** |

`const` is five letters. The ROM path is `constant` omit-last-3, not
an identity word. Shrinking `const` to `var` saved 53 Brotli bytes
on two already-large files. ESM output that is already a `const`
culture would flip this.

`let` → `var` was a no-op on every corpus: those emits do not use
`let` yet. The `var` → `let` win is therefore “introduce the keyword
the file did not specialize,” not “swap two existing keywords.”

## Why this is a beam item

A compiler that always emits `const` because it is “correct” will
lose tens of Brotli bytes on a `var`-heavy host. A compiler that
always emits `var` will lose tens on LilScript jQuery. The right
keyword is **the one the rest of the file already paid for**, unless
TDZ / assign errors force `const`.

Do not mix `var` and `const` in the same function just because some
bindings are immutable. Majority keyword, then score.

## Interaction with alphabet

`let` shares `e`, `t` with `function` / `return` / `length`. `var`
shares `a`, `r`. The `alphabet-function-letters` win and the
`var-to-let` win can **stack** on LilScript files and **fight** on
jquery.min. Score the product. Surgical stacks live in
[extra.json](extra.json).
