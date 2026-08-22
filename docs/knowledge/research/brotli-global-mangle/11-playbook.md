# Playbook: heuristics for a Brotli-first mangler

Parent: [index](README.md). This is candidate-**generation**
advice. Ranking stays with the configured codec on the complete
legal artifact. Nothing here is a reason to skip
`lilscript-codec`. Compiler source is not changed by this folder.

The compiler already has most of these knobs
(`local_name_reserve`, `entropy-aware-mangling`,
`quote-style-selection`, `function-layout-variants`,
`compact-boolean-literals`, `cost_model`). The measurements say
which ones deserve beam budget, and which folklore to stop
generating.

## Always generate

1. **Cross-scope short-name reuse.** Reserve the first N one-char
   spellings so similar functions share `a`/`e`/`t`. Never emit
   “fresh alphabet per function” as a finalist. Uniquify is
   +14–28 KB Brotli on minified jQuery. `local_name_reserve=0`
   already costs +275 on the lean audit.
2. **At least three alphabets** for the hottest short locals:
   current, letters from `function`/`return`/`length`
   (`e n i o t a r s …`), and one hostile or rotated control.
   They are not interchangeable. Score the **product** with the
   declaration keyword, not each alone. Independently measured
   deltas do not add ([15](15-color-merge.md)).
2b. **Fewer colors, biased to `e`/`t`/`n`.** If two locals do
   not interfere, they should share a name even when a fresh
   letter is free. Merging only the hottest name into `t` or
   `e` (often illegal) saved 186–548 Brotli bytes — more than
   the whole alphabet rewrite. The legal version is an
   interference-aware color collapse, not `a,b,c` greed.
3. **Both quote styles**, whole-file, never mixed. gzip and
   Brotli disagree (jquery.min +13 gzip / −16 Brotli).
4. **Declaration-keyword majority**: `var` vs `let` (and `const`
   only if the file already is `const`). LilScript jQuery wants
   `let` (−44 to −92). jquery.min wants `var` (+17 to flip).
5. **Boolean family**, not a single rewrite: keep `true`,
   `!0`, and a shared `undefined` binding as alternatives.
   `!0` lost 58 Brotli on raw JS gl-matrix; expanding `!0`
   won 12 on measured LilScript jQuery.
6. **Source function order** as the layout baseline. Add at most
   one scramble (reverse or similarity) as a control. Do not
   accept a gzip-winning reorder without a q11 score.

## Generate only with a cheap filter, then score

7. **String pool** for long, medium-frequency strings. Reject
   if raw drops and gzip rises until q11 agrees (jquery.min
   pool: −658 raw, +24 Brotli). gl-matrix has nothing to pool.
8. **Host-name alias** (`var d=document`) only inside a hot
   function, only to a letter already in the chosen alphabet.
9. **`local_name_reserve` sweep** {0, 8, 16, 32, 48} — the
   compiler already does this in production search. Keep it.
   The 0 point is the ablation, not a candidate you want to win.

## Do not generate

10. **ROM words as locals** (`length`, `function`, `return` as
    the name table). +1.3–3.6 KB Brotli on every minified
    corpus, +1.9 KB on the Monaco prefix. The dictionary is
    for rare literals and host names you must emit anyway.
11. **Dictionary warmup preambles.** Exact ROM phrases can
    cost more than they add (jquery-lil-min +148 Brotli for
    34 raw bytes). Unique padding “wins” are first-block
    noise.
12. **Bracketize `.length` to feed `"length"`.** Usually a
    loss; the two tiny Brotli wins fight gzip.
13. **Rewrite arrows to `function` to “use the dictionary.”**
    audit-function-spelling: +274 Brotli.
14. **q5 or gzip as the final ranker.** They are filters.
    Layout is the family that flips most often (q5 −159 /
    q11 +5 on one gl-matrix reorder).
15. **Prefix / first-N-KB scores as acceptors.** Use them to
    kill ROM-locals and prefix-sort. Accept only on the
    served object (file or HTTP chunk).

## How to spend a limited beam

For a ~100 KB size-first Brotli artifact, in order:

| Budget slot | Family | Why |
|---|---|---|
| 1 | configured emit (reuse on) | baseline |
| 2 | function-letter alphabet × majority `var`/`let` | −30 to −190; product ≠ sum |
| 2b | legal color-collapse toward `e`/`t` | illegal merge probe was −186 to −548 |
| 3 | quote flip | ±20, codec-disagreement |
| 4 | boolean family | −123 to +58, file-dependent |
| 5 | one layout scramble | usually lose; once −68 |
| 6 | string pool on / off | −59 to +24 |
| last | `reserve=0` / readable | ablation, not a winner |

For a ~2 MB artifact, keep 1–2 and 3. Drop ROM tricks and
aggressive layout. q11 probes are expensive; Monaco’s prefix
already rejected the small-file literal winners.

For gzip-first products, **swap the layout prior**: 32 KiB
clustering and function-by-length become real (jquery-lil-raw
−1188 gzip). Do not reuse that ranking on Brotli.

## Illegal probes (lab only)

`collapse-to-e` and `uniquify-short` are gravity meters. If a
legal candidate moves toward uniquify, it is probably wrong.
If a legal candidate moves toward shared `e`/`t`/`n` without
colliding bindings, it is probably in the right neighborhood.

## Scoring rules this folder will not relax

- Complete artifact, statement-aligned (do not cut mid-string).
- Report raw, gzip-9, Brotli-11. Keep losing columns.
- Node zlib numbers here are **diagnostic**. Gates stay on
  `lilscript-codec` (bundled Google C 1.1.0).
- Semantic legality is not optional. A 7 KB `e`-collapse is
  not a candidate.

## What “clever” actually meant

The global optimum was not a secret dictionary word. It was:

- the same short names in every similar function
- those names spelled with letters the file already breathes
- keywords (`var`/`let`, `"`/`'`, `!0`/`true`) that match the
  file’s majority
- layout left close to source unless the **served** codec
  says otherwise
- representation changes that **delete names** (positional
  aggregates) when ABI allows

Weird hacks (unique bait, reverse functions, rare letters)
exist to keep the beam honest, not to become the default pass.
