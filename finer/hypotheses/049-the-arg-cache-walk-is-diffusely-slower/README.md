# 049 — the argument-cache walk is one shape, not a diffuse sum

**Status: CONFIRMED as one shape, not a diffuse sum. On an idle 16-core machine, where the lane's
range is ±0.03 instead of ±0.15, the `|| ""` hole guard on a `string[]` element read is the whole
remaining gap: removing it alone takes `dup-loop` from 1.09 to 1.04, and the other three shapes
tested with it move nothing. The compiler now drops that guard when every use of the read is a
strict equality against a value a dominating branch proves truthy — the same fact upstream gets
from writing `if (v && v !== ea[k++])`. cnlil: dup-loop 1.09 → 1.04 and −52 Brotli.**

Lane: port. Objective: brotli, with runtime ≤ upstream on every lane of the port's own harness
(objective §3). Ports: cnlil. Opened: 2026-09-02.

## Prior art

- **V8**: a keyed load of a `string[]` element behind `||""` is a load plus a truthiness branch;
  measured here at ~0.007 of the lane, so the guard is not the gap (048 `g3-noguard`).
- **Upstream cn** writes the same walk with a labelled `continue outer` instead of a boolean flag,
  hoists `const ea = e.a`, and never allocates a bucket it does not use (`engine.ts:859-905`); the
  port now matches the last two.
- **LilScript** has no loop-invariant code motion pass (`optimizer.rs` has no LICM; the pipeline is
  scalar/inlining fixed points, escape analysis, scalar replacement, compress passes), so an
  invariant member read inside a loop is one property load per iteration. That is a real absence
  and the first candidate this folder should price on a benchmark that isolates it.

## Claim

The residual on `dup-loop` is not one missing rewrite but the sum of per-operation costs in the
same walk, and it is measurable: with a profile that attributes ticks per line for both artifacts
on the same corpus, the sum of the per-line differences accounts for the lane's ratio within ±0.02.
Confirms if the accounted differences add up and name a specific compiler or port change worth
≥ 0.03 on the lane; falsifies if the ticks are distributed with no line above 5% of the gap, in
which case the lane is at the limit of what this shape of code can do under the current emission
and the gate needs a different lever (a different cache structure in the port, or a compiler-level
representation change).

## Read

- `finer/out/048/results.md` (the h0-h6 and g0-g3 tables), `finer/out/048/prof/`
- `~/cnlil/src/default.lil` `resolveArguments`, `~/cnlil/vendor/cn/packages/cn/src/engine.ts:822-905`
- `src/optimizer.rs` pass list (no LICM), `src/codegen_ir_js.rs:12907` (the `string[]` hole guard)

## May touch

- `~/cnlil/src/*.lil`; `src/optimizer.rs`, `src/codegen_ir_js.rs`; this folder; `finer/out/049/`

## Method

`node --cpu-prof` on the pretty-printed artifact so lines are meaningful, both implementations on
the same `dup-loop` corpus, ticks aggregated per line (`finer/out/048/prof/lines.mjs`); each
candidate shape isolated by hand on the compiled artifact, checked with `diff-check.mjs`, and
benchmarked interleaved (seven rounds) before any compiler work.

## Result

Measured on an idle 16-core pool worker, seven interleaved rounds per lane, the ranges in
brackets. The local host is shared with other sessions and its ranges are four to five times
wider, which is why the earlier hand tests read as noise.

| variant, cumulative | dup-loop | range |
|---|---:|---|
| shipped port | 1.092 | 1.05-1.19 |
| drop the hole guard in the walk | **1.036** | 1.02-1.14 |
| + upstream's labelled `continue` instead of the boolean flag | 1.036 | 1.01-1.09 |
| + `bucket === void 0` instead of the null normalization | 1.033 | 1.01-1.11 |
| + strict identity in the chain update | 1.018 | 0.98-1.03 |

The guard is the step that matters; the other three are worth about 0.02 together. Compiled with
the elision (no hand edits): dup-loop 1.041, single 1.049, loop 1.056, and the artifact is
9400 Brotli against upstream's 9783.

An explicit `if (valueIndex >= entryCount) break` in the port does **not** buy the elision:
`string_index_in_bounds` does not learn from a dominating comparison, and even with the range the
array's density is unprovable for an `extern class` field. The fact that works is about the *other*
operand, not the index.

### Earlier, on the shared host (kept for the record)

| variant (hand, on the shipped shape) | dup-loop | note |
|---|---:|---|
| base | 1.185 | |
| split the polymorphic coalesced binding | 1.153 | noise alone |
| drop the per-call empty-array allocation | 1.119 | noise alone; **now fixed in the port** |
| drop the string hole guard | 1.160 | noise alone |
| all three | 1.078 | |
| + hoist the invariant `entry.a` | 1.066 | **now fixed in the port** |
| compiled port after both port fixes (g2) | 1.093-1.101 | |
| the same, hole guard stripped by hand (g3) | 1.086 | the guard is ~0.007 |

## Verdict

Confirmed and landed on the compiler side (`b6da284`): the hole guard was one shape, not a
diffuse sum, and the fact that removes it is a dominating branch on the compared value. What
status.md carries: (1) a lane whose range on the shared host is ±0.15 cannot be read there at all
— the pool's idle workers resolve 2% steps; (2) `string_index_in_bounds` is an index-range
question and the guard is often an *operand* question, which is where the proof was; (3) the port
kept its own two shapes (the bucket walk allocates nothing on a hit, the entry's values are read
once per candidate) because they are what upstream writes.

## Next

`ssr` at 1.09 is now the widest lane and is untouched by this: it is all cold `mergeUncached`, so
the next attribution belongs there, on the same idle-worker method. Loop-invariant member motion
stays unpriced — the port hoisted its one hot case by hand.
