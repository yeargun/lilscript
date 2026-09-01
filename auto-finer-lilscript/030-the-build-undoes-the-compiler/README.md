# 030 — The port's build script was undoing the compiler's minification

**Status: FIXED on micromarklil. 4538 raw and 229 Brotli, and 1963/1963 tests still pass.**

## Where this came from

The project owner's redirect: *"the lilscript code we write might be not fine? that might also be
the reason why our lilscript code dont get smaller?"* — look at the port, not only the compiler. That
is the right instinct, and it pointed at the layer between them.

## The observation

micromarklil is the **only** port in the fleet that ships `true`/`false` instead of `!0`/`!1`:

| port | `!0` | `!1` | `true` | `false` |
|---|---:|---:|---:|---:|
| hast-util-to-htmllil | 32 | 69 | 0 | 0 |
| markedlil | 36 | 71 | 0 | 0 |
| mobxlil | 162 | 152 | 2 | 0 |
| mdast-util-to-hastlil | 15 | 7 | 0 | 0 |
| **micromarklil** | **0** | **0** | **81** | **50** |

The compiler is not at fault. Its own output is correct:

| file | `!0` | `true` |
|---|---:|---:|
| `dist/micromark.raw.js` — compiler output | **87** | 0 |
| `dist/micromark.esm.js` — after the port's esbuild step | **0** | 81 |

## The cause

micromarklil's ESM is not the compiler's file. The build re-bundles it:

```js
await esbuild({
  stdin: {contents: `export {compile,micromark,...} from "./${file}.raw.js"`, ...},
  outfile: resolve(dist, `${file}.esm.js`),
  minifyWhitespace: true,
  minifyIdentifiers: false,
  minifySyntax: false,          // <-- this
})
```

esbuild parses `!0`, understands it as the boolean `true`, and — with `minifySyntax` off — prints the
**canonical** form on the way out. Every compact spelling the compiler chose is discarded by the tool
that was only supposed to bundle. Thirteen other ports are unaffected because their measured ESM *is*
the compiler's file; micromark's is the one that round-trips.

## The fix and what it is worth

`minifySyntax: true` on that lane and the three that re-print it:

| | raw | Brotli |
|---|---:|---:|
| before | 100212 | 26930 |
| after | **95674** | **26701** |
| | **−4538** | **−229** |

`dist/micromark.cjs` 96898 → 96424 and `dist/micromark.umd.js` 96987 → 96513 from the same change.
**1963/1963 tests pass** on the result.

## Why this is worth generalising

This is the second time a port's *build* — not its source and not the compiler — has been the whole
story. [006](../006-markdown-stack-loss-diagnosis/README.md) found `minifyWhitespace` missing in
rehypelil, worth 2517 Brotli; [028](../028-unminified-lil-lane/README.md) found the measurement
harness itself bundling our lane unminified, worth 10634. Now a bundler flag silently reverting the
compiler's spelling choices.

The pattern is the same each time: **a tool between the compiler and the artifact re-prints the code
and normalises away what the compiler decided.** Nothing in the pipeline notices, because the output
is still correct — only bigger.

So `auto-finer-lilscript/shipped-vs-compiled.mjs` now makes that comparison standing: for every port
it reads the compiler's own `dist/*.raw.js` beside the artifact the port ships, and fails when a
compact spelling the compiler chose has been *completely* lost, or when the shipped file has gained
more than twenty `true`/`false`/`undefined` the compiler never emitted. A bundle legitimately grows
by pulling in dependencies, so partial changes are ignored; total loss of a spelling is
normalisation, not inlining. It would have caught this in one second, and it catches 006's class too.

Running it now flags one port: `rehype-katexlil`, whose ESM is the mid-refactor stub
[023](../023-unparseable-class-expressions/README.md) documents — its `!0`s are gone because the code
is gone. That is the check working.

## What it does not fix

micromark is still +3925 against its Terser baseline. Running Terser over our own output still finds
**16113 raw and 1578 Brotli** we are leaving behind — dominated by statement-to-sequence merging
(`;` −1498, `,` +932), branch-to-expression (`if(` −299, `?` +249) and declaration merging
(`var ` −344). Those are compiler work, and 582 `var` statements against Terser's 38 is where to
start.
