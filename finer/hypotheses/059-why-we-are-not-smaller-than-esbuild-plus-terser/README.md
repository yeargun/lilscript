# 059 — Why we are not smaller than esbuild + Terser

**Status:** answered, against the right baseline, with the mechanism named and
counted.

## Pick the baseline first

Three different numbers all get called "Terser on katex", and the question is
meaningless until one is chosen:

| lane | raw | Brotli |
|---|---|---|
| what `import "katex"` actually serves (`dist/katex.mjs`, unminified) | 610,145 | **119,059** |
| `dist/katex.min.js`, reachable by CDN or deep import only | 276,701 | 62,686 |
| the site's baseline: esbuild bundle of the npm graph, Terser 3 passes, mangle on | 264,213 | **63,044** |
| the same from the Flow sources | 263,654 | 61,758 |
| **ours** (`katex.esm.js`, never post-minified) | 273,486 | **64,907** |

Against what npm serves we are **45% smaller**. Against a bundler that minifies
their source we are **+1,863**. Both are true; the second is the engineering bar
and is what follows.

## Where the 1,863 is

`scripts/artifact-anatomy.mjs` on `_site/katex.js` against `_site/official.js`:

| stream | raw Δ | Brotli Δ |
|---|---|---|
| string literals | **+10,400** | **−298** |
| number literals | −843 | −102 |
| identifier occurrences | +653 | **+1,418** |
| punctuation + keywords | **+76** | **+1,186** |

Our data wins: 10 KB more raw string bytes that compress *better*, because the
content is more repetitive. Both losing streams are **the same size and compress
worse** — 653 bytes more identifiers costing 1,418, and 76 bytes more punctuation
costing 1,186. Nothing here is a size problem. It is a variety problem.

## What the variety is

Counting constructs in both artifacts:

| construct | ours | theirs | ratio |
|---|---|---|---|
| `unary +` | **323** | **5** | **64.6×** |
| `assign =` | **2,218** | **993** | **2.23×** |
| `if` / `else` | 216 | 79 | 2.73× |
| arrow function | 279 | 139 | 2.01× |
| comma sequence | 649 | 482 | 1.35× |
| `if` | 415 | 356 | 1.17× |
| ternary | 348 | 299 | 1.16× |
| `var` declaration | 461 | 721 | 0.64× |
| `function` declaration | 2 | 31 | 0.06× |
| `switch` | 0 | 16 | — |
| `unary !` | 397 | 725 | 0.55× |
| `void` | 30 | 135 | 0.22× |

Three findings, in order of size:

**1. 1,225 more assignments.** SSA lowering materialises intermediate values as
named assignments where Terser leaves them nested inside the expression that
consumes them. This is exactly `collapse_vars`, the one Terser option with real
content behind it in finer 053 (−629 raw): `t=[],q=[t]` against `q=[t=[]]`. It is
a compiler gap, it accounts for most of the identifier-stream loss, and it is not
about the port.

**2. 323 numeric coercions against their 5.** The port calls `toNum` / `toInt`
**272 times**, because on a `JsValue` the compiler cannot know a value is already
a number, so arithmetic gets a `+` to force one. Each is one byte, but they are
scattered single characters inserted mid-expression, which is the worst shape for
Brotli: they break token sequences that would otherwise repeat. A typed port does
not emit them at all.

**3. More varied control flow.** 2.7× the `if/else`, 2× the arrows, 1.35× the
comma sequences, while they keep 31 function declarations and 16 switches we
lower away. Every one of our choices is locally shorter. Collectively they make
the artifact less self-similar, and Brotli charges for that.

## So, why

Because on this port we compile *the same program* — finer 055 measured that:
katexlil has zero struct or class declarations and 3,974 `JsValue`s, so nothing
gets flattened and our artifact mirrors upstream's shape. Having no structural
advantage, the contest reduces to spelling, and there we make more, shorter,
more varied choices than Terser's uniform rules do.

Finding 2 is a port fix and it is the same fix as 055: declare the types and the
coercions vanish. Finding 1 is a genuine compiler gap worth closing on its own,
because it applies to every port, typed or not.

## Next

`collapse_vars`: sink a single-use intermediate into its one reader so the binding
dies. We have SSA where Terser has only `reduce_vars`, so we can decide it with
better information than it has. Worth ~1,200 assignments on this port.
