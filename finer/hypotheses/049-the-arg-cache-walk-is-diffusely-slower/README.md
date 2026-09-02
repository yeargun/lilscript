# 049 — the argument-cache walk is diffusely slower

**Status: OPEN — after 048 cnlil is at parity or better on eight of nine lanes (0.998-1.042) and
1.09-1.10 on `component:dup-loop`, the lane whose calls miss the sequence prediction and walk the
argument-cache bucket. A line profile puts 110 ticks in our `resolveArguments` against 86 in
upstream's, spread over the whole body rather than in one shape: the four shapes hand-tested there
(a polymorphic coalesced binding, a per-call empty-array allocation, a re-read invariant property,
a string hole guard) are worth 1.19 → 1.07 together and nothing apart, and two of them are already
fixed in the port. What remains is a per-operation gap in the same code, and this folder is where
it gets measured instead of guessed.**
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

Open.

## Next

Attribute the 110-against-86 ticks line by line on the current artifact (the table above was taken
on the pre-fix shape), and price loop-invariant member motion on a benchmark that isolates it
before proposing a pass.
