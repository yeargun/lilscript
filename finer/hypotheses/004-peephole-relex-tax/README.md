# 004 — The peephole re-derivation tax

**Status: PARTIALLY LANDED (lexer cache in). Main finding located, fix in 005.**

## Hypothesis

[001](../001-where-does-compile-time-go/README.md) profiled the acorn port, where the codec is 75% of
the compile. But acorn emits only 26 KB. The artifact that actually motivated this workstream is the
jQuery port — 93 KB out, **404 s at level 15**. Assuming the two behave alike would be a mistake, so
the pipeline was instrumented end-to-end and jQuery measured directly.

## Instrument

`src/timing.rs` was generalized from two counters into named `Bucket`s, each recording
`(wall nanos, calls, bytes)` and reported as one JSON line under `LILSCRIPT_TIMING`. Buckets:
`codec`, `analyze`, `emit`, `peephole`, `cleanup`, `optimize`, `lex`, `closers`, `regions`, `scopes`,
`bindings`, `idle_fold`, `active_fold`, plus iteration counts for the two SSA fixed points.

## Measurement — jQuery port, level 13, 4 pinned cores

CPU columns sum across Rayon workers, so percentages exceed 100%. The host is shared, so absolute
wall clock varies run to run; **call counts and byte totals are deterministic** and carry the
argument.

| bucket | ms | % wall | calls | per call |
|---|---:|---:|---:|---:|
| emit (IR→JS + 6 text folds) | 112601 | 71.6% | 58 | 1941 ms |
| peephole (135-fold pipeline) | 45026 | 28.6% | 8 | 5628 ms |
| codec (Brotli-11) | 13685 | 8.7% | 52 | 263 ms |
| optimize (SSA pipeline) | 12698 | 8.1% | 8 | 1587 ms |
| analyze | 2598 | 1.7% | 47 | 55 ms |
| — of which lex | 3385 | 2.2% | 3824 | 0.9 ms |
| — of which bindings index | 4028 | 2.6% | 240 | 16.8 ms |
| — of which closers | 1398 | 0.9% | 4015 | 0.3 ms |
| — of which regions | 512 | 0.3% | 79 | 6.5 ms |
| — of which scopes | 454 | 0.3% | 91 | 5.0 ms |
| **idle folds (rewrote nothing)** | **23736** | **15.1%** | **1980** | **12.0 ms** |
| active folds (rewrote something) | 10927 | 7.0% | 432 | 25.3 ms |
| scalar fixpoint | 8371 | 5.3% | 46 runs | max 2 iters |
| inline fixpoint | 1096 | 0.7% | 12 runs | max 12 iters |

## Findings

1. **jQuery and acorn have completely different bottlenecks.** acorn (26 KB) is 75% codec; jQuery
   (93 KB) is 6-11% codec and ~95% emission + peephole. Any conclusion drawn from one artifact size
   does not transfer. This retired [003](../003-cheap-codec-screening/README.md) as the lead fix.
2. **The competitor study's "uncapped fixed point" lead is real but not the cause.**
   `optimize_scalar_fixed_point` converges in **at most 2** iterations and
   `optimize_inlining_fixed_point` in at most 12, together under 6% of wall. The missing cap is a
   latent robustness gap worth closing on its own merits, not a compile-time fix. Recorded as a
   negative result so it is not re-chased.
3. **82% of peephole fold invocations rewrite nothing** — 1980 idle of 2412 measured — and those
   idle folds consume **23.7 s, 15% of the whole compile and 68% of all fold time**. Each idle fold
   spends ~12 ms re-deriving whole-program structure over ~86 KB only to conclude its enabling
   syntax is absent. This is the single largest addressable waste found so far.
4. **Shared index construction is not the problem.** lex, closers, regions, scopes, and bindings
   together are ~6% of wall. The 12 ms an idle fold spends is overwhelmingly in the fold's *own*
   scanning loops, not in the indexes it builds. So the fix is not "cache the indexes" — it is
   "don't enter the fold at all".
5. The pipeline runs `optimize_generated_javascript_pass` **twice** per call whenever the first pass
   rewrote anything and a constructor table remains, doubling everything above.

## Change landed here: per-thread lexer cache

`src/js_peephole/token.rs`. Every fold takes `&str` and re-tokenizes from scratch; a fold that
rewrites nothing hands the *identical* string to the next fold. `Token` carries `text` as a borrow,
but that borrow is redundant with the `start`/`end` offsets beside it — so a tokenization can be
stored as owned `(kind, start, end)` triples and *replayed* against any string with the same bytes.

Four per-thread slots, matched by exact byte equality (length first, so a mismatch is normally
O(1)). Not a hash: a hash could in principle return another program's tokens, and this sits directly
on the correctness path.

**Result: `lex` calls 7640 → 3825, bytes scanned 665 MB → 333 MB, and jQuery output byte-identical
to the pre-cache baseline.** Worth having, but only ~7% of wall — it is not the fix, and saying so
matters more than the win itself.

## Consequence

→ [005](../005-idle-fold-guards/README.md): give each fold a cheap necessary-condition guard so an
idle fold costs microseconds instead of 12 ms.
