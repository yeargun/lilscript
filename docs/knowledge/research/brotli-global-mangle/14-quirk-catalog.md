# Quirk catalog

Parent: [index](README.md). The ugly measurements that a tidy
mangler would never propose, and why they are still design input.

## 1. Unique padding beat the dictionary

`bait-unique` (`qwxkzzzz(){return;}`) beat `bait-function-return`
and the exact ROM phrase `type="text/javascript"` on several
100 KB files:

- jquery.min −13 vs ROM +7
- jquery-lil-measured −51 vs ROM −29
- glmatrix-lil-raw −52 vs ROM −35

The static dictionary is a first-occurrence discount. These files
already paid that occurrence. Extra ROM text at the top is just
literals that **look** helpful and sometimes poison the first
Huffman block. Unique junk is also literals; it is merely less
pretentious. Neither belongs in a compiler. The quirk is: **do
not trust “this string is in the ROM” as a reason to emit it.**

## 2. An exact 22-byte ROM word cost 148 Brotli bytes

jquery-lil-min + 34 raw bytes of
`var __="type=\"text/javascript\""` → **+148 Brotli**. The phrase
is in RFC 7932. The rest of the file never says it. First-block
stats specialize a symbol that has no future copies. Tiny-lab
“use ROM words as keys” dies here in public.

## 3. Expanding `!0` back to `true` can win

jquery-lil-measured `bool-expand`: +465 raw, **−12 Brotli**.
The file was mixed. Making the majority spelling win can beat
the shorter minority spelling. Compact boolean literals are a
**family**, not a rewrite-all pass.

## 4. Rare letters beat `function` letters on one file

Vite JS gl-matrix:

- `alphabet-function-letters`: gzip **−74**, Brotli **+12**
- `alphabet-rare`: gzip **+130**, Brotli **−24**

The “steal `e n i t o` from `function`” prior lost Brotli on the
numeric-kernel file and won gzip. The hostile alphabet won
Brotli. [03](03-alphabet.md) keeps this as a **control candidate**,
not a reason to ship `q` by default.

## 5. A 1-char permutation can win without changing frequencies

`rotate-short-13` on jquery-lil-measured: **−36 Brotli**, +216
gzip, same raw, same name frequencies, different letters.
`rotate-short-13` on glmatrix-lil-raw: **−16 Brotli**. Blind
shuffle usually loses (jquery-min +61 / +78). Sometimes the
current assignment is a local Huffman trap. That is why
`entropy-aware-mangling` already does a bounded 1-char
permutation search.

## 6. Two-char → one-char looks like an alphabet win

`alphabet-function-letters` on jquery.min is **−548 raw** and
−67 Brotli. The pass maps the hottest **1–2 character** locals
onto a 13-letter alphabet, so some `ab`-class names become `e`.
jquery-lil-min has **0 raw** and still **−31 Brotli**: that row
is the pure letter-quality effect. When you cite the −67, say
which part is shortening.

## 7. Illegal `e`-collapse is the reuse ceiling

Same raw, every 1-char local spelled `e`: jquery-lil-min
**−7015 Brotli**, jquery-min **−3730**. Not shippable. It
answers “how much of the stream is ‘a short name I have seen’?”
Legal reserve (`local_name_reserve`) is the approximation. The
audit `no-reserve` pays +275 Brotli to turn that off. See
[02](02-reuse.md) and [09](09-audits.md).

## 8. Smaller raw, worse Brotli is normal

`audit-lean` vs `jquery-lil-raw`: −3205 raw, **+90 Brotli**.
`audit-no-inlining` vs lean: −796 raw, +240 Brotli.
`pool-strings-4x6` on jquery.min: −658 raw, +24 Brotli.
A profitability filter that only looks at raw will keep these.

## 9. q5 is not a little q11

LilScript gl-matrix Vite `fn-by-length`: gzip −127, **q5 −159**,
**q11 +5**. Using q5 to pick layout ships a q11 regression.
Using q5 to **reject** uniquify (+18 KB) is fine.

## 10. Reverse order beat source order once

LilScript gl-matrix Vite `fn-reverse`: **−68 Brotli**, +30 gzip.
Source order was a gzip local optimum. Brotli wanted the other
end first — or, more likely, wanted a different Huffman context
at a few function boundaries and reverse happened to provide it.
Do not “always reverse.” Do keep source order **and** one
scrambled control in the beam.

## 11. Bracketizing `.length` can win 12 bytes and lose 82 gzip

jquery-lil-raw `dot-length-bracket`: **−12 Brotli**, +82 gzip,
+1288 raw. The served codec matters. A gzip-first product should
not take this candidate; a Brotli-first product might, if 12
bytes are worth the semantics risk (some host objects trap
`["length"]` differently? usually not). Still a weak tactic.

## 12. `const` is expensive when it is the minority

`const-to-var` on jquery-lil-measured and glmatrix-lil-raw:
**−53 Brotli** each. Five letters, ROM via `constant` omit-3.
If the file is not already a `const` culture, do not introduce
it for “correctness theater.”

## 13. Function spelling is not “more dictionary”

`audit-function-spelling` (`function` instead of arrow): +3286
raw, +274 Brotli vs lean. More `function` tokens, worse stream.
The ROM word is a discount on the **first** `function`, not a
reason to rewrite arrows.

## 14. Monaco rejected the 100 KB winners

On the 210 KB IDE prefix, quote-flip, `!0`, and string pooling
all **lost**. Prefix-sort lost 1539 Brotli. [10](10-monaco.md).
Heuristics are corpus-sized.

## 15. Merging two hottest colors beat the alphabet pass

jquery.min `e`→`t` (illegal if they interfere): **−288 Brotli**.
jquery-lil-min `r`→`e`: **−548**. gl-matrix Vite `e`→`t`: **−186 /
−200**. The whole `alphabet-function-letters` rewrite was −67 to
−176. [15](15-color-merge.md).

## 16. Independent 32 KiB cuts tax 10%

Same LilScript jQuery raw: 33283 whole vs 36730 as independent
32 KB streams. Delivery object ≠ compiler object.
[13](13-window-chunks.md).
