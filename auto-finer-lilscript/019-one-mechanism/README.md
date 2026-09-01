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

**Fix B — the `unstable` + `cross_block` 79–90%.** This is the rest, and it is compiler work:
`cross_block` is LilScript reconstructing expressions from a CFG where definition and use sit in
different blocks, while Terser starts from an AST where they already share a tree. Note
`fallthrough_only` is **0–10**, so naive block merging is not the lever.

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
