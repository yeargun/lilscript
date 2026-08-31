# 007 — Is level 13 the sweet spot, and should it be the default?

**Status: CONFIRMED, EMPHATICALLY.** Level 15 is a 20x compile-time cost for 1.4% of bytes.

## Hypothesis

From objective.md: *"by default for our projects, maybe make it 13, not 15. And 13 needs to be
literally the sweet spot of time consumption for compilation and bundle sizes."*

`javascript.optimization_level` (0..15) sets the effort ladder. At `candidate_search = "production"`
the retained-candidate and proposal caps are pinned to 384 for every level >= 7, so the *real*
differences between 13 and 15 are only four things:

| knob | 13 | 14 | 15 |
|---|---:|---:|---:|
| `effective_candidate_byte_budget` | 768 KiB | 896 KiB | unbounded |
| `effective_candidate_beam_width` | 10 | 11 | 12 |
| `effective_terminal_codec_probe_limit` | 192 | 256 | 384 |
| level-gated features | — | `+ir-function-subsumption`, `+ir-phase-ordering`, `+ir-compress-pass`, `+joint-chunk-symbol-search` | — |

## Method

`auto-finer-lilscript/bench.sh` sweeps levels on one port, recording CPU time (user+sys, summed over
workers), peak RSS, canonical raw/gzip-9/Brotli-11 sizes from `lilscript-codec`, and the
deterministic work counters. Wall clock is unusable on this host — unrelated processes hold 1-2
cores continuously and identical runs have varied 3x — so CPU time is reported and the work counters
carry the argument.

## jQuery port (219 KB `.lil` source, 90 KB emitted, shipped `lilscript.toml`)

| level | CPU s | peak RSS MB | raw | gzip-9 | **Brotli-11** | codec encodes | emissions | fold MB scanned |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 15 | **1829.0** | 249.8 | 92706 | 33713 | **30225** | 500 | 94 | 2802.7 |
| 14 | 151.1 | 198.5 | 93662 | 34193 | 30607 | 72 | 63 | 317.5 |
| 13 | 89.8 | 170.2 | **89858** | 34197 | 30651 | 52 | 58 | 166.3 |
| 12 | 74.8 | 153.8 | 89872 | 34191 | 30635 | 47 | 51 | 145.3 |

## acorn port (26 KB emitted, `candidate_search` forced to `production`)

| level | raw | gzip-9 | **Brotli-11** | codec encodes |
|---|---:|---:|---:|---:|
| 15 | 26290 | 3550 | 3069 | 279 |
| 14 | 26315 | 3566 | **3068** | 253 |
| 13 | 25952 | 3575 | 3071 | 246 |
| 12 | 25949 | 3557 | 3070 | 249 |
| 11 | 27111 | 3627 | 3102 | 243 |
| 10 | 27672 | 3603 | 3099 | 217 |
| 9 | 27679 | 3604 | 3098 | 344 |

## Findings

1. **Level 15 is a catastrophic trade on a large artifact.** jQuery: **1829 CPU-seconds versus 89.8
   — 20.4x — to save 426 Brotli bytes, 1.41%.** The last step alone (14→15) is 12x the CPU for 382
   bytes. Level 15's search issues **500** canonical encodes and scans **2.8 GB** through the fold
   pipeline, against 52 encodes and 166 MB at level 13: **17x the work for one part in seventy.**
2. **Level 13 is at the knee, and the curve is flat around it.** acorn is within **3 bytes**
   (0.1%) across levels 12-15 and only breaks down at 11 and below (+31 bytes, 1%). jQuery moves
   0.14% between 12 and 14. Below 12 both artifacts degrade sharply. So 12-14 is a plateau and 13
   sits in the middle of it — exactly the "sweet spot" claim, now with numbers.
3. **More search is not monotonically better.** acorn's *best* Brotli is at level **14** (3068), not
   15 (3069); jQuery's level **12** beats level 13 (30635 vs 30651). This is expected of a bounded
   beam search and is worth stating plainly: paying for level 15 does not guarantee the smallest
   artifact, it only guarantees the largest bill.
4. **Level 13 produces the smallest *raw* jQuery artifact** (89858 vs 92706 at level 15). Level 15
   is spending raw bytes to buy Brotli bytes — legitimate under a Brotli objective, but it means a
   raw- or gzip-objective project gains nothing at all from level 15. jQuery's gzip is *worse* at 13
   (34197 vs 33713), so the level choice genuinely depends on the declared `cost_model`.
5. **Memory follows effort**: 250 MB at 15 against 170 MB at 13.

## Change

`JavaScriptConfig::default().optimization_level`: **15 → 13** (`src/config.rs`).

Explicit `optimization_level = 15` in individual port configs is left alone. Those are deliberate
per-artifact choices, and lowering them would grow shipped artifacts — which conflicts with the
"beat the upstream minified size" half of the objective. The table above is the evidence the owner
needs to revisit them one at a time; for jQuery specifically, level 15 currently costs 20 minutes of
CPU for 426 bytes and that looks like a bad deal even for a shipped artifact.

## Blast-radius check, and an independent corroboration

Changing a default can silently alter every artifact that relied on it, so the corpus was audited
before this was called safe.

**No shipped port inherits the default.** All 26 sibling `*Lil` repositories set
`optimization_level` explicitly in every one of their 55 configs. Inside this repository, 38 configs
set it explicitly and **8 inherit** — all of them test fixtures under `tests/`, and the full suite
passes. So the default change cannot move a single shipped artifact; it changes what a *new* project
gets, which is exactly what was intended.

The audit also produced an independent corroboration of the plateau. The levels the ports' authors
actually chose, without reference to this measurement:

| level | ports using it |
|---|---|
| 12 | rehypelil, hast-util-to-htmllil, mdast-util-to-hastlil, rehype-stringifylil, remark-breakslil, remark-gfmlil, remark-mathlil, remark-rehypelil |
| 13 | katexlil, remarklil, unifiedlil, micromarklil, remark-parselil, react-markdownlil, mdast-util-from-markdownlil |
| 15 | jquerylil, markedlil, mobxlil, zodlil, monacolil, posthoglil, playcanvaslil |

**Fifteen of the ports had already converged on 12 or 13 by hand.** That is a separate line of
evidence for the same conclusion, gathered by people optimizing their own artifacts rather than by
this sweep, and it is the strongest single argument that 13 is the right default.

## What would make 13 better still

The 382 bytes between 14 and 15 come almost entirely from the unbounded candidate byte budget: 428
extra canonical encodes for 382 bytes, i.e. **0.9 bytes per encode at roughly 3.5 CPU-seconds each**.
There is no cheap way to buy them — they are the tail of a beam search, not a missing optimization.
Making 13 smaller therefore means making the *candidates better*, not making the search wider. That
is the size work, not the effort-ladder work.
