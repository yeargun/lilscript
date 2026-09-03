# Where katexlil's remaining Brotli gap is (measured 2026-09-03, build X = 65062)

Terser's published lane is 63044. Every number below is `lilscript-codec` Brotli-11 on
whole artifacts, differences of whole-artifact measurements, never a sum of parts.

## The gap is not the symbol table, the names, or anything Terser knows

| decomposition | ours | terser | delta |
|---|---:|---:|---:|
| whole artifact | 65062 | 63044 | **+2018** |
| the 648 `defineSymbol` calls, deleted from each | 3635 | 3762 | **−127** |
| everything else | 61427 | 59282 | +2145 |
| pure code (that, minus the shared 57626-byte metrics table) | 51954 | 49908 | +2046 |

The symbol table is a **win**: we inline `"math","main","textord"` at 648 call sites where
Terser keeps `me,pe,Me` bindings, which is +6483 raw and −127 Brotli. Outlining them back
into variables was measured three ways and is a loss at every threshold below 20 uses
(+1012 at ≥2, +28 at ≥8, −15 at ≥20, inside the ±60 floor). Brotli prices the repeated
literal below the reference.

What the gap is *not*:

- **Naming.** Terser's own mangler over our artifact: **−24**.
- **Anything in Terser's compressor.** Full `compress{passes:3}+mangle` over our artifact:
  −774; after the string-concat fold landed, −605. Per-pass on our own output:
  `evaluate` −124 (all printer), `collapse_vars` −173, `conditionals` −59, `if_return`
  **+51**, `sequences` +28, `booleans` +30. The printer alone (`{defaults:false}`) is −124
  of that, and ~20 of *that* is our license banner.
- **Property mangling.** Symmetric: −2358 for us, −2355 for them.
- **Code structure.** With every string and regex literal counted as one character and the
  metrics table removed, our code is 135164 characters against 134604 — **+560**, 0.4% —
  and we spend **2228 fewer** identifier bytes.

## What it is

Pure code is +2861 raw for +2046 Brotli: **0.72 Brotli per excess raw byte**, the rate of
genuinely novel content, spread thin rather than pooled anywhere:

| class (literals as one char) | ours | terser | delta |
|---|---:|---:|---:|
| `=` | 5471 | 4184 | +1287 |
| plain assignments (`;x=`, `,x=`) | 2185 | 1135 | +1050 |
| grouping parens | 789 | 619 | +170 |
| `,` | 11510 | 11036 | +474 |
| `(` total | 9685 | 9264 | +421 |
| identifiers | 77197 | 79425 | −2228 |
| `:` | 2928 | 3440 | −512 |

We materialise ~1050 more intermediates than Terser's output holds, and pay for them in
`=`, commas and parens while saving on identifier bytes. Terser's `collapse_vars` — which
inlines exactly those — recovers only 49 Brotli beyond its printer, so the materialisation
is *not* what the codec is charging for. Per function we are at parity or ahead:
`overline`'s builder is 319 bytes against KaTeX's 327 after 050's fixes landed.

## Ranked leads

1. **Object completion and temp inlining into literals.** `d={p:1};d.k=[e,c]` stays split;
   `fold_fresh_empty_object_assign` only takes `{}`, and `fold_single_use_temporaries` only
   takes a *pure read* initialiser, not an object or array literal. ~15 `.children=` sites
   plus ~19 short arrays of identifiers; ceiling ~340 raw.
2. **Printer.** 54 single-statement braces, ~20 redundant parens, ~15 strings that would
   escape less under `'`, two `Infinity`. Ceiling −100 after the banner.
3. **Phi copies sunk into branches.** `makeStackedDelim` repeats `A="",j=0,e="Size1-Regular"`
   in ~30 arms where KaTeX initialises once before the chain: 618 bytes of repeated
   assignments against their 372. Local to one function; ~250 raw.
4. **Spread.** The port hand-writes `pushAll`/`spliceSpread`/`maxOfList` because the
   language has none; `o.splice(e,2,...s[k])` costs us a 120-byte IIFE. 8 `.apply` sites.

## Dead ends, measured

Redundant `+` coercions (`3*+h`, `0-+c`): raw −236, Brotli **0**. `let`→`var`: +18.
Statement joining to 49 top-level items: −76. Outlining repeated literals: above.
Larger terminal probe budget (256→1024): byte-identical.
