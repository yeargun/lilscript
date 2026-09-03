# 055 — The ports that win are the ports that are typed

**Status:** confirmed. This reframes what the remaining losses are.

## The question

Does a LilScript program come out flat? Closure ADVANCED collapses namespaces,
devirtualizes methods and drops what it can prove unused; the objective says we
exceed it. So: does the compiled artifact still look object-oriented?

## For markedlil, no — it is completely flat

`markedlil` against upstream `marked` minified by Terser:

| | ours | upstream |
|---|---|---|
| class declarations | **0** | 7 |
| class methods | **0** | 81 |
| `this` references | **0** | 259 |
| object literals | **14** | 73 |
| object literal properties | **71** | 270 |
| dotted property reads | 958 | 1450 |

Every class is gone. Every `this` is gone. Five sixths of the object literals
are gone. And markedlil **wins by 611 Brotli**.

## For katexlil, yes — it is a mirror of the original

| | ours | upstream |
|---|---|---|
| class declarations + expressions | 20 | 21 |
| class methods | 138 | 140 |
| constructors | 20 | 21 |
| `new` expressions | 256 | 278 |
| `this` references | 518 | 621 |
| object literals | 708 | 705 |

Nothing was flattened. We emit the same shape upstream does, so the only thing
left to compete on is spelling — which is the fight finer 053 measures us losing
by 2,113 bytes in the identifier stream alone.

## Why

The port sources, not the compiler:

| port | struct/class declarations | `JsValue` mentions | fleet |
|---|---|---|---|
| cnlil | 1 | **39** | beats upstream |
| markedlil | 4 | **84** | **WIN −611** |
| zodlil | 1 | 2,835 | win (dev build, see 054) |
| remarklil | 0 | 823 | LOSS +4,530 |
| mobxlil | 0 | 1,741 | LOSS +2,638 |
| micromarklil | 7 | 1,893 | LOSS +3,208 |
| katexlil | **0** | **3,974** | LOSS +1,770 |

katexlil has **zero** struct or class declarations and 3,974 `JsValue`s. It is a
transliteration of the JavaScript into dynamic property bags — `JS.object`,
`JS.invoke`, `self["classes"]`, `JS.method3`. Against that the compiler cannot
know a field's offset, so it cannot lay a value out positionally; cannot know a
receiver's type, so it cannot devirtualize; cannot prove a property set
complete, so it cannot drop one. Every optimization that makes LilScript more
than a minifier is switched off by the source, and what is left competes with
Terser on spelling.

The line is sharp and it is not about size or domain: the two ports written in
typed LilScript win, and every port written as an untyped transliteration
loses.

## What follows

The remaining large losses — katexlil +1,770, mobxlil +2,638, remarklil +4,530,
micromarklil +3,208 — are **not compiler deficiencies**. They are ports that
never gave the compiler anything to work with. Chasing them with peephole folds
is chasing a minifier fight we have no structural advantage in, which is exactly
what folders 050 and 053 record going nowhere.

Typing a port is a data-type change, which is inside what the owner allows for
port sources, and cnlil is the precedent: rewritten to module state and typed
handles, it went from losing to 9,433 against upstream's 9,783 (finer 048).

Next: take one katexlil module with a clear shape — `domTree` builds nodes with
a fixed field set — and give it declared types, then measure that module alone
both ways. If the flattening shows up there the way it does in markedlil, the
route for the whole port is settled.
