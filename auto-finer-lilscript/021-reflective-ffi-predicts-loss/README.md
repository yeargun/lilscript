# 021 — Reflective host-FFI density predicts the size gap

**Status: CORRELATION CONFIRMED, REMEDY FALSIFIED.** Reflective-FFI density orders the scoreboard
almost perfectly, and it is the answer to *"sometimes lilscript code might be the reason why we
compile into less optimized code"*. But the obvious fix — converting those calls to typed LilScript —
was tried two ways on a real file and **regressed both times**. Partial typing cannot pay: the port's
helper layer is `JsValue`-typed end to end, so it is all-or-nothing per data path.

## How this was found

[019](../019-one-mechanism/README.md) traced every remaining loss to one mechanism: dynamic receivers
make reads observable, observable values are `unstable`, and unstable values each take a name and a
statement. [020](../020-unstable-transitivity/README.md) then showed the compiler has no lever for it.

So the question became *why* those receivers are dynamic. Looking at the actual `.lil` source of the
worst family:

```lil
// micromarklil/src/util-decode-string.lil:38
return jsString(JS.call(JS.get(value, "replace"), value, characterEscapeOrReference, replacer));
```

That is not LilScript with some dynamic parts. **That is JavaScript, transliterated through a host
FFI.** Every property access is `JS.get(obj, "name")` — a runtime string-keyed lookup through an
extern. The compiler cannot mangle those names, prove the reads pure, flatten the objects, or type
anything downstream of them.

## The metric

Counting reflective host calls — `JS.get`, `JS.set`, `JS.call`, `JS.method` — per thousand lines of
port source, against each port's measured Brotli delta:

| port | lines | `JS.get` | `JS.set` | `JS.call` | `JS.method` | **reflective / kloc** | **Brotli vs upstream** |
|---|---:|---:|---:|---:|---:|---:|---:|
| **markedlil** | 2528 | **0** | **0** | **0** | **2** | **0.8** | **−579 WIN** |
| jquerylil | 12115 | 1 | 1 | 72 | 113 | 15.4 | +1825 |
| mobxlil | 8100 | 0 | 0 | 151 | 287 | 54.1 | +3577 |
| mdast-util-from-markdownlil | 7263 | 104 | 551 | 110 | 373 | 156.7 | +3175 |
| remark-parselil | 7289 | 105 | 554 | 110 | 375 | 156.9 | +3235 |
| micromarklil | 7264 | 111 | 707 | 85 | 391 | **178.1** | **+4154** |

**The only port written with near-zero reflective FFI is the only clean win.** The ordering is
near-monotone across two orders of magnitude of density, and the extremes are decisive: markedlil at
0.8/kloc beats upstream, micromarklil at 178/kloc loses by 18%.

It is a correlation across six ports, not a controlled experiment — mobxlil at 54 has a wider gap
than mdast-util-from-markdown at 157, so density is not the only term. But nothing else measured in
this workstream orders the scoreboard this well.

## Why it is causal, not coincidental

The chain is measured end to end, not inferred:

1. `JS.get(obj, "name")` is an extern call, so its result has no known coercion category.
2. `op_evaluation_is_observable_assuming` therefore reports it observable
   (`codegen_ir_js.rs`, the `IndexGet` and dynamic-operand arms).
3. `unstable_values` closes transitively over it, so everything computed from it is unstable —
   **54% of stored values on micromarkLil, 48% on jQueryLil**
   ([019](../019-one-mechanism/README.md)).
4. An unstable value cannot be fused into its consumer, so it takes a name and a statement —
   **+19.4% identifier occurrences and +42% semicolon density** against Terser on micromarkLil.
5. And the property names stay runtime strings, so property mangling has nothing to mangle:
   **370 untyped keys, 0 local-only, 0 typed.**

Each step has its own measurement in this log.

## The obvious remedy was tested, twice, and both times it regressed

021's correlation invites one action: convert the reflective calls to typed LilScript. That was tried
on `micromarklil/src/core/label-end.lil` in a scratch copy, two different ways, measuring the whole
port each time.

**Attempt 1 — construction.** Nine `JS.set(o, "k", v)` calls folded into three
`JS.object("type", …, "start", …, "end", …)` literals, the exact idiom `core/attention.lil` already
uses:

| | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| delta | −6 | −21 | **+18** |

**Attempt 2 — reads.** Six dynamic `objGet(events[i][1], "start")` reads replaced by a struct view
plus field access, again the idiom `attention.lil` uses:

| | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|
| delta | +129 | +44 | **+84** |

Both worse. The construction swap changes syntax the emitter already compiles to compact property
assignments; the read swap costs five extra local bindings to save six short reads.

## Why partial typing cannot pay here

The second attempt is the informative failure. It was first written with real field types —
`PointView start; PointView end;` — and did not compile: `copyPoint`, `objGet`, `call1` and the rest
of `host.lil` all take and return `JsValue`. To hand a typed field to any helper the helper has to be
typed too, and to type the helper its callers have to be typed, and so on.

**The port's helper layer is `JsValue`-typed end to end, so typing is all-or-nothing per data path.**
Retyping one file cannot pay, because every value it produces immediately re-enters an untyped
helper and loses whatever was proven about it. That is why the attempt above had to fall back to
`JsValue` fields — and with `JsValue` fields the typed view proves nothing, so it is pure overhead.

**So 021's correlation is a valid description and its obvious remedy is not a valid plan.** A
retyping effort has to convert a whole data path — token/event/point representation *and* every
helper that touches it — before any of it shows up in bytes. That is a real project, and this is the
measurement that says so rather than assuming it either way.

## What to do, in payoff order

This reorders every other recommendation in this workstream.

1. **micromarklil, remark-parselil, mdast-util-from-markdownlil** — 150–180 reflective calls per
   kloc, and they are one family sharing a core. They also feed `remark` and `react-markdown`, so
   **five of the eleven scoreboard losses trace to this one codebase.** Rewriting the tokenizer's
   token/event/context objects as structs and its `JS.method` state machine as typed function values
   is the single highest-payoff work available.
2. **mobxlil** — 54/kloc, all in `JS.call`/`JS.method` rather than property access, so the fix is
   typed closures rather than typed data.
3. **jquerylil** — only 15/kloc, and its committed artifact is already within **780 Brotli** of
   `jquery.min.js` while beating it on raw by 4489. The closest port to flipping, and the one where
   compiler work still plausibly matters.

And the standing caution: **markedlil proves the toolchain wins when the source is real LilScript.**
2528 lines, two `JS.method` calls, and it beats `marked.min.js` by 579 bytes. The compiler is not the
thing holding the other ports back.

## What this retires

Three earlier recommendations are now known to be smaller than this:

- typed sources for *property purity* alone: −540 jQueryLil, −327 micromarkLil
  ([019](../019-one-mechanism/README.md)) — real, but a fraction of the gap, because it addresses
  step 2 while the reflective calls also defeat steps 4 and 5;
- narrowing the `unstable` closure: **+69 net**, reverted ([020](../020-unstable-transitivity/README.md));
- porting competitor peephole passes: three implemented, all measured at zero
  ([017](../017-oxc-declaration-merge/README.md)).
