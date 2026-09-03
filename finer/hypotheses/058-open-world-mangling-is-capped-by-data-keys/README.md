# 058 — Open-world mangling is capped by data keys, not by the API

**Status:** the rule that would make it sound is identified and simulated. It
does not pay on an untyped port, and the arithmetic says why.

## The intent

Open world: the DX API stays reachable, everything the program invented is
renamed. Closed world: the API goes too. That is Closure ADVANCED's contract and
it is the right shape. 057 built the machinery. This asks what it can actually
reach.

## The rule that makes it sound

Class 3 of 057 was keys computed from data — `katexImagesData[label]` where
`label` comes from parsed LaTeX. The shape in the source is decidable:

```lil
JsValue katexImagesData = undef();
JsValue o3 = JS.object("overrightarrow", ..., "xleftarrow", ...);  // 40 keys
katexImagesData = o3;
...
JsValue data = katexImagesData[label];        // label is not a constant
```

A module global assigned once from an object literal, then read with a
non-constant key. So the rule is:

> **An object literal that can reach a member access whose key is not a
> compile-time constant keeps every one of its keys.**

That is a backward taint from non-constant-key accesses to literal creation
sites, over SSA plus single-assignment globals — analyses the optimizer already
has.

## Simulated on the finished artifact

`scripts/_sim.mjs` (scratch, not kept) applied exactly that rule plus the
platform externs, the export names, and the escaping-string rule:

- 106 bindings are read with a non-constant key
- those taint **275 object literals**
- **1,381 names preserved, 179 renamed**
- −8,422 raw, **−690 Brotli**

And it still fails 212 of 653 official tests, because a babel pass over compiled
output cannot follow a literal that reaches a computed index through a function
parameter. That part is a harness limit, not a limit of the rule — the compiler
could follow it.

## Why it is not worth building

| | names renamed | Brotli |
|---|---|---|
| the sound rule | 179 | **−690** |
| a crude externs set | 397 | −1,840 |
| nothing preserved | 400+ | −2,993 *(wrong program)* |

Our gap to Terser is **1,770**. The sound rule is worth 690 of it. The other
1,100 is in names that are genuinely reachable from data — the LaTeX command
vocabulary is the key set of these tables — and no analysis can rename those,
because at run time the program looks a key up by a string the *document*
supplied.

275 of the artifact's object literals are tainted. In a program where every value
is `JsValue` and every table is a property bag, almost every literal is reachable
from a computed index, so the sound rule preserves almost everything. That is not
a weakness of the analysis. It is what the program is.

## What actually fixes it

The distinction the analysis is straining to recover is one the source can simply
state. These tables are **dictionaries** — `Map<string, T>` — and the compiler
never renames a Map key, because a Map key is a value, not a property. The
objects that are **records** get declared fields, and those rename freely and
completely.

That is the same conclusion as [[055]] and [[057]] from a third direction, and it
is now quantified: on untyped source, sound property mangling recovers 690 of
1,770; on typed source, `markedlil` shows the whole class of optimization
working — zero classes, zero `this`, and a win.

## Next

Take `stretchy.lil` and `buildCommon.lil`, whose tables are the clearest case
(`katexImagesData`, `svgData`), and spell them as `Map` rather than `JS.object`.
Measure that module pair alone. If the Map spelling costs nothing at run time and
frees its literal's keys, the same move applies to every table in the port and
the 1,100 becomes reachable.
