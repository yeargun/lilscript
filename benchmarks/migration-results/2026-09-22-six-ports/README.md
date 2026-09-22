# Six-port goal: our Brotli and raw sizes against the original (2026-09-22)

Owner goal, 2026-09-22: katexlil, markedlil, motionlil, zodlil, posthoglil and jquerylil must each compile smaller than the original library. That means Brotli with the Brotli objective, and raw bytes with the raw objective. A loss may be fixed in the compiler or in the port's LilScript. Bottom-up, module-by-module analysis is the method for a losing port. Competitor sources are cloned under `~/competitors/` to learn from, not to copy: terser, rolldown, oxc, esbuild, closure-compiler, swc and UglifyJS.

## The bars

Each bar is the port's own declared baseline: the `baseline: true` row of its `site/results.json`, built from the pinned upstream by that port's harness. Where the port has none, the bar comes from `finer/tools/competitor-recipes.mjs`. The bar is always the smallest eligible competitor for the same public surface, compared like with like: license banners on both sides or on neither.

| Port | Brotli bar | Raw bar | Construction |
|---|---|---|---|
| katexlil | 63,044 | 267,050 | katex@0.16.22 npm graph, esbuild bundle, Terser mangle (Flow-source Terser is 61,758: the stretch bar) |
| markedlil | 10,092 | 37,022 | marked@18.0.10 parse-only sources (the port's surface), Vite 8 Oxc mangle |
| posthoglil | 5,622 | 16,123 | posthog-js@1.418.10 kernel modules, Vite 8 Oxc mangle |
| jquerylil | 27,445 | 87,151 | official `jquery.min.js` 3.7.1 for Brotli; Oxc on the ESM for raw (Terser 27,613 / 87,239; Oxc 27,751 / 87,151) |
| zodlil | open | open | the port publishes 2 names against zod/v4's 240: needs the restricted recipe (plan 013-T5) |
| motionlil | 41,032 | 137,455 | `motion` npm, esbuild bundle: Terser for Brotli, Oxc for raw (Oxc 41,246 Brotli). Surface 326 names against 312: to be matched |

## Standing (semantic route)

Brotli of the compiler output with the Brotli objective, raw bytes of the raw-objective build. Batches are listed in `docs/migration/index.md`.

| Port | Start (`577d472d`) | Now | Bar | Gap |
|---|---|---|---|---|
| katexlil (complete `katex.esm.js`) | 65,727 | 65,131 | 63,044 | +2,087 |
| markedlil (`marked.raw.js`) | 9,397 | 9,293 | 10,092 | **win −799** |
| posthoglil (`posthog.raw.js`) | 5,952 | 5,884 | 5,622 | +262 |
| jquerylil (`jquery.esm.js`, both with banners) | 33,593 | 32,558 | 27,445 | +5,113 |
| zodlil (`zod.core.js`) | 28,326 | 28,060 | open | – |
| motionlil (`full.js`) | does not build | 52,080 | 41,032 | +11,048 |

| Raw objective | Start | Now | Bar | Gap |
|---|---|---|---|---|
| markedlil (`marked.bytes.js`) | 39,687 | 37,071 | 37,022 | +49 |
| posthoglil (`posthog.bytes.js`) | 19,750 | 18,580 | 16,123 | +2,457 |
| katexlil, jquerylil, zodlil, motionlil | no raw configuration yet | | | |

The default route, for reference with the same binary: posthoglil 5,602 (it also loses once its `esm.js` banner is counted), katexlil 64,620–64,907, jquerylil 28,764. motionlil's committed `full.js` is 50,526.

## Known causes, by port

- **jquerylil:** 16,254 bytes of long binding names, against 1,113 on the default route. They come from the carried `js-host.ts` module, which is only whitespace-compacted (minifying it alone is worth −492 Brotli), and from escaping functions that keep their exact source `.name` (`fnLoad`, `queueHooks`). Statement-heavy spelling is another cause: 1,224 `if(` against 303.
- **motionlil:** 12 class names are declared in two modules (`JSAnimation`, `GroupAnimation`). The semantic checker keys classes and enums by bare name (`Type::Class(&str)`), which rejects that; the default route's linker qualifies them. Owed as a language fix; a rename patch unblocks measurement.
- **katexlil:** the port is an untyped transliteration (3,990 `JsValue`, 394 `toNum`, no `pure`). Terser still finds 1.7% in our output, mostly single-use functions.
- **posthoglil:** string arrays are not packed, small `JsValue` wrappers are not inlined, and the port declares builtins through `JsValue` (`JS.number(JS.invoke(Math,"trunc",x))`).
