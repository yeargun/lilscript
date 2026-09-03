# 057 — Property mangling needs types, not declarations

**Status:** the compiler feature landed and is sound on the paths it was built
for. The aggressive policy is **measured unsafe on an untyped port**, with three
distinct failure classes, and the third cannot be fixed by declaration.

## Why this was worth trying

Closure ADVANCED on upstream katex is **257,166 raw / 61,345 Brotli**, against
Terser's 276,701 / 62,686 and ours at 275,444 / 64,907. Its single biggest lever
is property renaming: 245 of its 383 property names come out at two characters
or fewer, cutting property-name bytes from ~31 KB to ~17 KB.

Ours renamed **4 of 428**. Simulated on the finished artifact, doing what Closure
does is worth **−37,608 raw / −2,993 Brotli** at the ceiling, and **−23,992 raw /
−1,840 Brotli** with a real externs set. Our whole gap to Terser was 2,221, so
this one thing was the difference between losing and winning.

And it worked. With the policy on, katexlil compiled to **250,708 raw / 62,755
Brotli — 382 bytes better than Terser.** It also failed all 7 package tests.

## Why our default renames nothing

`[mangle] internal_properties` defaults to `"underscore-suffix"`: a
dynamically-keyed property is the program's to rename only if it is spelled
`name_`. katexlil has none, so nothing was eligible.

The reason for that convention is real. On an untyped `JsValue`,
`value.toUpperCase()` and `node.measuredDepth` are the same shape. There is
nothing in the IR to tell a member the platform owns from one the program
invented, so the safe default is to rename none of them.

## What was built

- **`src/js_externs.rs`** — 551 platform property names, the ECMAScript surface
  as the engine reports it plus the DOM members a browser port touches. Never
  renamed, whatever the policy. This is Closure's externs file, except it ships
  with the compiler instead of with the project.
- **`[mangle] internal_properties = "all"`** — every key the compiler cannot see
  the host read is the program's own.
- **`[mangle] preserve_properties`** — the surface only the port knows: options
  a caller sets, fields a callback reads off a context, members of a returned
  value.
- **Export names are property names.** A program keeping the export `render` is
  not renaming a property `render`, because a library's default export is
  usually an object mirroring its named exports.
- **Strings that escape a key position are preserved.** katex builds
  `{math:{}, text:{}}` and reads `symbols[mode]`; renaming one half is a wrong
  program. A string constant used only as a static key is safe, one that goes
  anywhere else is not.

The underscore convention keeps its exact old contract: it stays behind
`extern_fields = false` and every existing artifact is byte-identical.

## The three failure classes, in the order they appeared

1. **Platform members on untyped values.** `toUpperCase` became `r`. Fixed by
   `js_externs`.
2. **The API surface the compiler cannot see.** `katex.__defineMacro is not a
   function` — the default export object's keys, then the extension API a
   user's macro calls on the expander. Declarable: 174 names, two mechanical
   sources (every property the official suite touches that upstream katex also
   has, and every `X["prototype"]["name"]` the port defines).
3. **Keys computed from data.** `overrightarrow`, `xleftarrow`, `widehat1`,
   `sqrtImage` — SVG and stretchy-arrow tables read as `table[label]` where
   `label` comes from the parsed command name. **The key's text never appears as
   a string literal anywhere**, so no escaping-string rule can see it, and no
   declaration can enumerate it: the set is the LaTeX command vocabulary.

Class 3 is the wall. Deciding it needs to know which object a computed index
reaches, and on a program whose every value is `JsValue` there is nothing to
decide that with.

## Where this leaves it

| | raw | Brotli | tests |
|---|---|---|---|
| katexlil, as shipped | 275,444 | 64,907 | pass |
| with `"all"`, nothing declared | 250,708 | **62,755** | 0 / 7 |
| with `"all"`, 96 names declared | 254,858 | 63,072 | 1 / 7 |
| with `"all"`, 174 names declared | 259,226 | 63,605 | 3 / 7 |
| Terser | 276,701 | 63,137 | — |

Declaring more names costs back exactly what the mangling won, and the curve was
never going to reach 7 / 7 — class 3 does not respond to declarations.

katexlil is reverted to its safe configuration. The compiler feature stays: it
is sound for declared aggregates and for the underscore convention, and `"all"`
is opt-in with the contract written down.

**This is finer 055 again from another direction.** Property mangling is one
more optimization that untyped source switches off, alongside positional layout,
devirtualization and dead-field elimination. The route to the 2,993 bytes is to
give the compiler the types, not to give it a longer list of exceptions.
