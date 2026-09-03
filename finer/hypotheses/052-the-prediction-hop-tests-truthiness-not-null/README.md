# 052 — the prediction hop tests truthiness where upstream tests null

**Status: RESOLVED in the compiler. The winning spelling is now what the typed source compiles
to, and it is both smaller and faster than the shipped build: single 1.048 → 1.036, loop
1.056 → 1.029, dup-loop 1.042 → 1.030, and 9456 → 9443 Brotli. Two compiler facts were missing,
not one; the second only showed up after the first was fixed.**

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


## Resolution

The port could not reach the winning artifact because the compiler was missing two facts. Fixing
the first exposed the second, and each on its own is a *worse* build than shipping — which is why
the earlier measurements looked contradictory.

**One: a `??null` nobody can observe.** A nullable `extern class` field read lowers to a plain
member access, so its absent case is already `undefined`; the normalization only exists to spell
that `null`. It is dead when nothing downstream separates the two, and three kinds of use cannot:
a loose `==`/`!=` (JavaScript's loose equality equates null and undefined, so no operand tells
them apart), the narrowing intrinsics, and any read in a block dominated by a branch that already
proved the value non-nullish — the same dominance proof the string hole guard uses (049).

**The bug that taught the shape of it.** The first version also re-spelled the test `!==void 0`,
copying the record-read path it sits beside. That crashed cnlil on the first call:

    TypeError: Cannot read properties of null (reading 'a0')

A record's absent key is `undefined` and nothing else, so `===void 0` is a complete test there. A
host field can hold a `null` the *program* stored, so `!==void 0` is true for it and the guarded
call runs on null. Dropping the normalization is safe; re-spelling the test is not. They are now
two analyses, and the weaker one carries that reason in its doc comment.

**Two: a strict compare blocked the elision, and the loose one costs 7%.** `JS.strictEqual` does
separate `undefined` from `null`, so it kept the normalization alive — leaving the port to choose
between the elision and the faster compare. Writing the compare loosely instead (`last !=
prediction`) reads better and lowers to abstract equality, which V8 runs instead of a pointer
compare: **1.098 against 1.029 on component:single**, measured. The fix is that a strict compare
against an operand whose *type* says it is neither null nor undefined answers `false` either way.
The type check matters over the dominance proof here: the optimizer narrows the value after a null
test, so by the compare the dominance proof names the pre-narrowing value and misses.

| build | single | loop | dup-loop | brotli |
|---|---:|---:|---:|---:|
| shipped, `JsValue n` | 1.048 | 1.056 | 1.042 | 9456 |
| typed + elision, loose compare | 1.098 | **1.014** | 1.052 | **9434** |
| typed + strict compare, no elision | 1.029 | 1.066 | 1.061 | 9475 |
| **typed + elision + strict compare** | **1.036** | **1.029** | **1.030** | **9443** |

Neither half dominates the shipped build. Both together win every lane and 13 bytes.

## What this says about predictability

The owner's framing was that a LilScript author should know what a source line compiles to. Two
places failed that here, and both are now either fixed or worth stating plainly:

- `field != null` on a nullable host field compiled to `(field??null)!=null`. The normalization was
  invisible in the source and cost bytes and an operation. **Fixed.**
- `a != b` on two references compiles to loose `!=` and is measurably slower than `!==`. The
  compiler does not yet promote it even when it can prove the two spell the same question. That is
  a real predictability gap and the next candidate: promote `Eq`/`NotEq` to strict when at most one
  operand can be nullish. It costs one byte per site and is worth about 7% where it is hot.

## Still open

`component:single` 1.036, `loop` 1.029, `dup-loop` 1.030 — better on every lane and smaller, but
not yet at parity. The `merge:workset` bimodality from 049 is untouched. The lazy field select
(v4 above) remains a knob-shaped trade: 20 bytes smaller, better on `loop`, worse on `single`.
