# 019 — The remaining losses are one mechanism, measured on two independent families

**Status: CONFIRMED and PRICED.** Eleven scoreboard losses reduce to a single cause, verified on two
unrelated port families, with the two available fixes priced against it.

## The question

The fleet stands at 9 wins / 11 losses. The losses look like eleven separate problems. This asks
whether they are.

Two families dominate: **jQueryLil**, and the **micromark family** (micromark,
mdast-util-from-markdown, remark-parse, and through them remark and react-markdown — five of the
eleven). They share no code, no authors' conventions, and no upstream lineage.

## The same signature, twice

Both measured against their own pinned Terser baseline with the canonical codec:

| metric | jQueryLil | its Terser | | micromarkLil | its Terser |
|---|---:|---:|---|---:|---:|
| raw | 83044 | 87533 | **−5%** | 89252 | 81566 | **+9%** |
| Brotli-11 | 28225 | 27445 | **+3%** | 26157 | 22776 | **+15%** |
| distinct 8-grams / total | **0.703** | 0.643 | | **0.644** | 0.610 | |
| byte entropy | 5.258 | 5.263 | | 5.653 | 5.601 | |
| **identifier occurrences** | **17990** | 16719 | **+7.6%** | **14153** | 11855 | **+19.4%** |
| **`;` share of file** | **1.53%** | 0.72% | **+113%** | **0.88%** | 0.62% | **+42%** |

jQueryLil emits **fewer raw bytes than Terser and still loses on Brotli**. micromarkLil emits 9% more
raw and loses 15% on Brotli. In both cases **the Brotli gap is larger than the raw gap**, and in both
cases the two metrics that move hardest are identifier occurrences and statement terminators.

Byte entropy is already matched — the entropy-aware identifier alphabet is doing its job. What is not
matched is **repetition**: more distinct short fragments means fewer and shorter LZ matches.

## The same cause, twice

`LILSCRIPT_STORE_CENSUS` reports why SSA destruction gave a value its own statement:

| bucket | jQueryLil | micromarkLil |
|---|---:|---:|
| `unstable` (evaluation observable, or depends on something that is) | 48% | **54%** |
| `cross_block` | 42% | 25% |
| `use_count > 1` | 10% | 16% |
| `single_use` | **1%** | **4%** |

And the property-escape census is identical in shape:

| | jQueryLil | micromarkLil |
|---|---|---|
| local-only keys | 0 | 0 |
| typed keys | 0 | 0 |
| **untyped keys** | **414 / 9657 B** | **370 / 7585 B** |
| key-opaque receivers | 544 | 332 |

**Every property-bearing receiver in both ports escapes to an untyped boundary**, because both are
written against `JsValue` bags where `o[k]` may be a getter. That makes their values `unstable`,
which forces each into its own named statement — and that is the identifier and semicolon excess
above.

One mechanism, two families, same numbers.

## Both fixes, priced

**Fix A — make property reads provably pure** (the `.lil` source work, or `assume_pure_property_reads`
as its unsound proxy). Measured directly:

| port | unstable values | raw | gzip-9 | Brotli-11 |
|---|---:|---:|---:|---:|
| jQueryLil | 1681 → 1276 | −1343 | −599 | **−540** |
| micromarkLil | 1139 → 955 | −615 | −393 | **−327** |

Real, and **not enough**: 540 against jQueryLil's 780 gap is 69%, but 327 against micromark's 3381 is
**under 10%**. Typing the sources would flip jQueryLil and would not come close on the micromark
family.

Note also that the flag itself is **not a legitimate win** — it is Terser's `pure_getters`, which the
baselines also leave off ([013](../013-statement-density/README.md)). Only a *type* the compiler can
prove counts.

**Fix B — the `unstable` + `cross_block` 79–90%.** Attempted and reverted in
[020](../020-unstable-transitivity/README.md): narrowing the `unstable` transitive closure to fusible
operands is sound and cuts the unstable count 12%, but nets **+69 Brotli** across the shipped ports.
**Reducing the unstable count is not a proxy for reducing bytes** — fusing a value into its consumer
substitutes an expression where a short name used to stand, and on the two largest ports that loses.
This mechanism is real but has no lever that can be pulled from the census; it has to be scored per
candidate against the real codec.
 This is the rest, and it is compiler work:
`cross_block` is LilScript reconstructing expressions from a CFG where definition and use sit in
different blocks, while Terser starts from an AST where they already share a tree. Note
`fallthrough_only` is **0–10**, so naive block merging is not the lever.

## Minimal reproduction of the mechanism

Reduced to something a port author can pattern-match against. Same config, `candidate_search = "off"`
so no search noise:

```lil
// dynamic — the shape both losing families are written in
export JsValue walk(JsValue token, JsValue out) {
  JsValue a = token["start"];  JsValue b = token["end"];  JsValue c = token["type"];
  out["s"] = a;  out["e"] = b;  out["t"] = c;  return out;
}

// typed
export struct TokenView { int start; int end; int type; }
export struct OutView { int s; int e; int t; }
export OutView walk(TokenView token, OutView out) {
  int a = token.start;  int b = token.end;  int c = token.type;
  out.s = a;  out.e = b;  out.t = c;  return out;
}
```

emits:

```js
// dynamic: 91 bytes, 3 unstable values, three named temporaries
function q(b,a){let c=b.start,d=b.end,e=b.type;a.s=c,a.e=d,a.t=e;return a}

// typed:   87 bytes, 2 unstable values — one read fuses straight into its use
function q(b,a){let d=b.end,e=b.type;a.s=b.start,a.e=d,a.t=e;return a}
```

The typed version fuses `token.start` directly into `out.s` instead of giving it a name. That is the
whole mechanism in six lines: **a dynamic receiver makes the read observable, an observable read is
`unstable`, and an unstable value cannot be fused into its consumer, so it takes a name and a
statement.**

**Do not extrapolate the 4 bytes.** This case is far too small to scale from, and the directly
measured whole-port numbers above (−540 jQueryLil, −327 micromarkLil) are the real prize. What the
reduction establishes is the *mechanism*, reproducibly, so a port author can recognize the shape.

## What was tried and did not reproduce it

Worth recording, because it narrows where the cost is. Typing a *comparison* changes nothing:

```lil
export JsValue classify(JsValue code, JsValue acc) { if (code == 62) { acc = 1; } ... }
export int      classify(int code, int acc)        { if (code == 62) { acc = 1; } ... }
```

Both emit the identical 90 bytes with **zero** unstable values.
`binary_evaluation_can_invoke_coercion` (`codegen_ir_js.rs:25309`) does return `true` for any dynamic
operand, so the comparison *is* nominally observable — but with a constant on one side the value
analysis resolves the category anyway and the whole chain folds to a ternary.

So micromarkLil's 248 `JsValue code` declarations are **not** the lever, despite being the single most
common annotation in that source. The cost is in **property reads on dynamic receivers**, not in
arithmetic or comparison on dynamic scalars. That distinction is what a retyping effort should be
aimed at, and guessing it the other way round would have wasted the effort.

## What this changes about the scoreboard

The eleven losses are not eleven problems. Subtract the two located regressions
([016](../016-marked-size-regression/README.md) fixed, [018](../018-mobx-admission-regression/README.md)
open and worth 253 Brotli), the scope-mismatched rows, and mobxlil's deliberate
`realistic-performance-first` choice, and what remains is **one mechanism** — priced at roughly 10%
source-side and 90% compiler-side.

It also explains why every competitor technique ported in this workstream measured at zero
([017](../017-oxc-declaration-merge/README.md), and the `Math.pow` and quote-style findings): those
passes repair *shapes a human wrote*. LilScript's problem is upstream of any peephole — it is which
values get a name at all.
