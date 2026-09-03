# 060 — The literal coercions were load-bearing

**Status:** tested and reverted. Splits the 202 coercions into two populations
that need opposite treatment, and rules out a compiler feature.

## The premise

Finer 059 counted **323 `unary +`** in our katexlil artifact against upstream's
**5**, traced to the port calling `toNum` / `toInt` 202 times. The reasoning was:
upstream writes `this.height + this.depth` with no cast, so we should not be
emitting `+a.height + +a.depth`. Remove the coercions and the noise goes.

## Why the port has them at all

Not transliteration sloppiness. `units.lil` builds the TeX unit table:

```lil
JS.object("mm", (toNum(7227) / toNum(2540)), ...)
```

Written as `7227 / 2540` those are **int literals and LilScript divides them as
integers** — the table would read `2`. `toNum` is how the port forces float
division. The coercion is doing real work.

## What removing them did

Rewritten as float literals — `7227.0 / 2540.0` — 18 coercions vanish from
`units.lil` and the constants fold, so the emitted form goes from

```js
g=7227,g/=2540,i=7227,i/=254,j=803,j/=800,k=12          // a temp per ratio
```

to a proper literal `{pt:1,mm:2.8452755905511813,cm:28.45275590551181,...}`.

**raw +12, Brotli +134.** Worse.

`2.8452755905511813` is nineteen bytes of unique digits. `7227/2540` is nine, and
its digits recur elsewhere in the program. Terser's artifact keeps
`mm:7227/2540` — its `evaluate` refuses a fold whose result prints longer than
the expression. We fold unconditionally, so the coercion was the only thing
keeping the short form, by accident.

## The compiler feature that would fix it, and why not to build it

Our IR folder is shared with the native backend, where folding is always right,
so the size test does not belong there. The JS-side answer is better than
Terser's: for any float literal, find the shortest rational `p/q` that reproduces
it exactly under f64 division and emit that when it is shorter. That recovers the
short form however the constant arose, which Terser cannot do — it can only
decline to fold.

Measured on the artifact: **7 literals qualify, 57 raw bytes total.** Not worth a
pass. Recorded so it is not proposed again.

## The two populations

| coercions on | count | treatment |
|---|---|---|
| a bare numeric literal | **62** | leave them. Removing them costs 134 Brotli for 57 raw of theoretical recovery. |
| a property read or call | **124** | the real cost, and only types remove them. |

The 124 are `toNum(options["sizeMultiplier"])`, `toNum(chars["length"])` — a
coercion because on a `JsValue` nothing proves the value is already a number.
Those are the ones upstream does not have and they are the ones worth removing.
They cannot be removed one call at a time: the value comes from another module's
object, so the type has to be declared where it is *created*, which is the
[[055]] work.

## What this changes

Finer 059's finding 2 stands, but its scope halves and its route is fixed: the
coercion noise is removed by declaring types on the data, not by editing call
sites. Anyone starting from "delete the `toNum` calls" will make the artifact
bigger, as this did.
