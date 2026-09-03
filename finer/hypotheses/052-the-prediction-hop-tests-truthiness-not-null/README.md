# 052 — the prediction hop tests truthiness where upstream tests null

**Status: CONFIRMED on the artifact, NOT REACHABLE from source. Replacing the single truthiness
test on the sequence-prediction hop with a null test takes every component lane down —
single 1.048 → 1.034, loop 1.058 → 1.026 — and it is 3 bytes. Typing the field so the compiler
emits that test produces a different whole-function emission that loses the win.**

Lane: port + compiler. Objective: runtime ≤ upstream on every lane (objective §3). Ports: cnlil.
Opened: 2026-09-03. Measured on an idle 16-core pool worker, Node 22, 9-11 interleaved rounds.

## Where the port actually stands

| lane | ours ns | upstream ns | ratio | range |
|---|---:|---:|---:|---|
| merge:short | 4.7 | 4.8 | **0.95** | 0.76-1.19 |
| merge:arb | 750.3 | 775.7 | **0.96** | 0.93-0.98 |
| merge:ssr | 346.4 | 351.2 | **0.99** | 0.98-1.00 |
| merge:long | 5.2 | 5.3 | **0.99** | 0.94-1.04 |
| merge:repeat | 6.1 | 6.1 | 1.00 | 0.99-1.03 |
| merge:workset | 10.9 | 10.9 | 1.02 | 0.98-1.33 |
| component:single | 7.5 | 7.2 | 1.04 | 1.04-1.05 |
| component:loop | 8.0 | 7.5 | 1.06 | 1.05-1.06 |
| component:dup-loop | 22.7 | 21.4 | 1.06 | 1.02-1.15 |

The merge algorithm meets the gate. The `cn()` variadic wrapper does not, and its ranges are tight
enough that the 4-6% is real, not noise.

## Prior art

- Upstream's wrapper (`engine.js:910`) is the shape the port already copies, `(nArgs|1)===3` trick
  included. Its prediction hop is `const pred = lh.n; if (pred !== null && match3(...))`.
- Ours emits `var e=a.n; if(e){var i=e; if(S2(i,...))}` — a truthiness test, because the port types
  the field `JsValue`, and a `JsValue` genuinely can be `""` or `0`. `truthy_nullable_checks=false`
  does not apply: that switch is for nullable *class* types.
- Profiling cannot see this. Both implementations inline the whole call into the benchmark loop
  (82% and 80% of samples in `pass`), so tick attribution returns nothing. Every result here comes
  from patching the shipped artifact and measuring interleaved.

## Result — hand patches on the shipped artifact, three runs

| variant | single | loop | dup-loop |
|---|---:|---:|---:|
| v0 shipped | 1.045 / 1.048 / 1.050 | 1.057 / 1.058 / 1.063 | 1.042 / 1.055 / 1.086 |
| **v2 null test at the hop** | **1.034 / 1.035** | **1.026 / 1.028** | 1.051 / 1.058 |
| v1 stop reusing the `arguments.length` binding | 1.047 | — | — |
| v4 lazy field select in the verify | 1.065 | 1.049 | 1.071 |
| v5 = v2 + v4 | 1.058 / 1.065 | **1.014 / 1.017** | 1.041 / 1.045 |

**v2 is the win**: one test, three bytes, better on every lane. **v1 is falsified** — reusing the
binding that held `arguments.length` for the first argument, which mixes a Smi and a string in one
slot, costs nothing. **v4 is a genuine trade**: our verify spells `var n=e.a1;0==a&&(n=e.a0)`
where upstream spells `k===0?e.a0:e.a1`, so ours loads a field it then discards. Removing that
eager load helps `loop` (1.058 → 1.014 with v2) and *hurts* `single` (1.048 → 1.065). It is also
20 bytes smaller. Worth a knob, not a default.

## The part that blocks it: the source route does not reach the spelling

Typing the field `ArgumentEntry? n` instead of `JsValue` is the obvious way to make the compiler
emit the null test. It does — and loses anyway:

| build | single | loop | dup-loop | brotli |
|---|---:|---:|---:|---:|
| shipped (`JsValue n`) | 1.045 | 1.063 | 1.042 | **9456** |
| typed (`ArgumentEntry? n`) | 1.070 | 1.045 | 1.057 | 9507 |
| typed, `??null` stripped by hand | 1.107 | 1.005 | 1.055 | 9477 |

Two things go wrong. The compiler inserts `a.n??null` at five sites to normalize an `extern class`
field read from `undefined` to `null` before the test — redundant, since `undefined != null` and
`null != null` are both false, and worth 30 raw bytes. And the whole function's name coalescing
changes (`var t=arguments.length; var r=b; t=c;`), which moves `single` the wrong way by more than
the null test wins. All suites stay green throughout.

So the port cannot express this today: the good artifact is three bytes from the shipped one, and
no source spelling reaches it.

## Next

Two independent compiler items, both general:

1. **Elide `?? null` when the read's only use is a loose null comparison.** Five sites here, 30
   raw bytes, and it removes an operation from the hot path. This is the
   `null_tested_host_field_reads` analysis already started in the `wt-048` worktree.
2. **Do not materialize the unselected operand of a field select.** `k===0?e.a0:e.a1` loads one
   field; our `var n=e.a1;0==a&&(n=e.a0)` loads two. Smaller *and* faster on two of three lanes,
   so it wants the codec scorer and a fleet A/B rather than an unconditional flip.

Neither is cnlil-specific. Both need the 22-port fleet pass before landing.
