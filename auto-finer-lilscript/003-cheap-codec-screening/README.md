# 003 — Can a cheaper encoder rank candidates the way Brotli-11 does?

**Status: MEASURED, PARKED.** The technique works, but [004](../004-peephole-relex-tax/README.md)
showed the codec is not the dominant cost on the artifacts that hurt most. Revisit for
small-artifact/wide-search configurations.

## Hypothesis

Every scored candidate costs a canonical Brotli **quality 11** encode of the whole artifact
(`canonical_brotli_size`, `src/compiler.rs`). On the acorn port that is ~75% of the compile.

Quality 11 is expensive because it runs Brotli's optimal-parsing ("z-opt") backend. Lower qualities
use greedy/hash-chain matching and are dramatically cheaper. But **search does not need the exact
size — it needs the exact *ordering***. If a cheap quality orders candidates the way q11 does, the
search can screen with the cheap encoder and spend exact q11 encodes only on a small verified beam.
The finally selected artifact would still be measured exactly, so the reported number never changes.

## Method

Added `LILSCRIPT_DUMP_CANDIDATES=<dir>` to `src/compiler.rs`, which writes every **distinctly
scored** artifact as `<canonical-q11-size>-<digest>.js` (it hangs off the 002 memo, so it captures
exactly the population the search actually paid for). Ran the acorn port at level 15 with
`candidate_search = "production"`: **279 distinct candidates**, 26 KB each, canonical q11 sizes
spanning 3069–5119 bytes.

Then re-encoded all 279 at other settings and asked the only question that matters:
*if the search screened with proxy P, how deep would the exact-verify beam have to go to still find
the artifact q11 actually prefers?*

## Results (279 real candidates, acorn port)

Per-candidate encode cost, same host, same buffers:

| encoder | ms/candidate | vs q11 |
|---|---:|---:|
| gzip-9 | 0.69 | **164x cheaper** |
| Brotli q5 | 0.65 | 174x cheaper |
| Brotli q9 | 5.74 | **20x cheaper** |
| Brotli q10 | 12.19 | 9x cheaper |
| Brotli q11 (canonical) | 113.37 | 1x |

Ranking fidelity against the canonical q11 ordering:

| proxy | rank it gives the true q11 optimum | K to cover true top-5 | K to cover true top-10 | Spearman ρ |
|---|---:|---:|---:|---:|
| Brotli q9 | **1** | 6 | 19 | 0.830 |
| Brotli q10 | **1** | 7 | 37 | 0.824 |
| gzip-9 | 2 | 6 | 25 | 0.830 |
| Brotli q5 | 9 | 18 | 20 | 0.828 |
| raw bytes | 22 | 32 | 32 | 0.608 |

## Findings

1. **Brotli q9 is an excellent screen.** It puts the true q11 winner **first**, and an exact-verify
   beam of 6 recovers the entire true top-5. Projected cost for this workload:
   `279 x 5.74 ms + 6 x 113 ms = 2.3 s` versus `279 x 113 ms = 31.6 s` — a **14x reduction** with
   the same answer.
2. **Raw bytes are a bad proxy and gzip is a surprisingly good one.** ρ = 0.61 for raw vs 0.83 for
   gzip-9, and gzip-9 costs the same as Brotli q5 while ranking far better. This matters
   independently: any place in the compiler that screens on raw length is leaving ordering accuracy
   on the table for no saving.
3. **Spearman ρ is the wrong summary statistic here** and is reported only for completeness. All
   non-raw proxies sit at ρ ≈ 0.83, yet they differ enormously in the metric that actually matters
   (q5 puts the winner 9th; q9 puts it 1st). Global rank agreement is dominated by the uninteresting
   tail; what a beam search needs is agreement at the *head* of the distribution.
4. **Node's Brotli 1.1.0 disagrees with the pinned `BrotliEncoderCompress` on 96 of 279 artifacts at
   q11**, even with `SIZE_HINT` set to the input length. The one-shot C entry point and Node's
   streaming wrapper evidently differ in some internal parameter. Ground truth throughout this study
   is therefore the compiler's own dumped size, never Node's. **This is a standing warning for the
   whole project: any lane that scores with Node's zlib/brotli instead of `lilscript-codec` is not
   measuring the canonical objective.**

## Why this is parked rather than landed

[004](../004-peephole-relex-tax/README.md) measured the jQuery port — the artifact whose compile
time actually motivated this workstream — and found the codec is only **6.7%** of it. The 51% is the
parsed peephole and 43% is emission. Screening would buy at most ~6% there while adding a real
"the search explored a slightly different frontier" risk to a byte-exact objective.

Conditions under which to revisit:
- small artifacts (<32 KB) with a wide candidate search, where the codec *is* the bottleneck
  (acorn L15 is exactly this: 75% codec);
- after 004/005 land, if the codec's share has grown back to dominance.

If revived, the design constraint is fixed: **the artifact that is finally selected must always
carry an exact canonical q11 measurement.** The screen may only order exploration, never report.
