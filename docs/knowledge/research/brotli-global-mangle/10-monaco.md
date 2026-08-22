# Monaco: does it survive a megabyte?

Parent: [index](README.md).

Artifact: `benchmarks/popular/apps/monaco/lil/ide.js` — LilScript
Monaco IDE emit, **2,371,075** raw bytes.

| Slice | Raw | gzip-9 | br5 | br11 | br11 / raw |
|---|---:|---:|---:|---:|---:|
| full file | 2371075 | 539173 | — | **423674** | 17.9% |
| prefix (last `;\n` before 400k) | 210467 | 63606 | 59650 | 54426 | 25.9% |
| jquery-min (scale ref) | 87533 | 30342 | 29763 | 27445 | 31.4% |

The prefix is **less** compressible than the whole file. Later
regions copy from earlier ones. A 210 KB window that never sees the
rest pays a 26% ratio; the complete 2.4 MB stream pays 18%. That is
the same lesson as independent chunks in
[13](13-window-chunks.md), at megabyte scale.

The harness originally sliced at a raw 400,000-byte cut and died
inside a string literal. Complete-artifact scoring needs a token or
statement boundary. “First N kilobytes” is not a Brotli sample of
the file.

## Prefix mutations (210 KB)

| Mutation | Δ gzip | Δ br5 | Δ br11 |
|---|---:|---:|---:|
| quotes-single | +79 | +37 | +64 |
| bool-minify | −3 | +5 | +59 |
| rotate-short-1 | +176 | +163 | +80 |
| locals-as-length-index-value | +1869 | +2571 | **+1868** |
| pool-strings-4x6 | +136 | +178 | +64 |
| fn-by-prefix | +4863 | +2310 | **+1539** |
| bait-javascript-type | +19 | +15 | +1 |

Nothing here won. The 100 KB jQuery / gl-matrix wins (quote flip,
`!0`, function-letter alphabet) **did not transfer** to this
prefix.

Why the prefix is a different animal:

- Monaco is editor + AMD + nls + DOM. Identifier mix is wider than
  jQuery’s `e`/`t`/`n` culture.
- `bool-minify` **loses 59 Brotli** while gzip is almost flat (−3).
  The prefix already has a lot of `0` / `1` / `!` from flags.
- `fn-by-prefix` is a **1.5 KB Brotli disaster** (and 4.8 KB gzip).
  Reordering a 210 KB AMD-ish emit wrecks whatever locality the
  compiler already had. Distant copies at `lgwin=22` already work;
  prefix-clustering just permutes Huffman context at every function
  boundary.
- ROM locals still lose (+1868), same as [04](04-dictionary-as-names.md).
- Dictionary bait is a 1-byte wash.

## What this means for search budgets

`entropy-aware-mangling` already scales permutation search down as
artifacts grow, because q11 probes are expensive. Monaco says that
is correct for a second reason: **the profitable mutations shrink
relative to the file**, and several 100 KB winners become losers.

A compiler that spends its beam on alphabet permutation of the
hottest 1-char locals is still right (rotate-short-1 lost only 80
on 54 KB Brotli — 0.15%). A compiler that spends it on function
prefix sort or ROM-word locals is wasting the budget.

Full-file mutations were not run. q11 on 2.4 MB is seconds per
candidate. The prefix is a **warning**, not a substitute for a
full-file score of a finalist you actually intend to ship.

## Heuristic

1. Score the **complete** artifact you will serve.
2. Use prefixes only to **kill** bad ideas (ROM locals, prefix
   sort), not to accept good ones.
3. Keep reuse. Do not uniquify at megabyte scale either — the
   18% whole-file ratio is reuse plus window, not clever letters.
4. Do not port a 100 KB quote / `!0` winner onto Monaco without
   measuring. This prefix rejected both.
