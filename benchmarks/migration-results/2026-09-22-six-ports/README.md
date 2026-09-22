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
| zodlil | 51,948 | 274,999 | zod@4 restricted to the 181 `z` members the port shares ([entry](zodlil/restricted-entry.mjs): each imported by name, so the bundler keeps only what they reach), esbuild bundle, Terser mangle. Unrestricted: 52,440 |
| motionlil | 41,032 | 137,455 | `motion` npm, esbuild bundle: Terser for Brotli, Oxc for raw (Oxc 41,246 Brotli). Surface 326 names against 312: to be matched |

## Standing (semantic route)

Brotli of the compiler output with the Brotli objective, raw bytes of the raw-objective build. Batches are listed in `docs/migration/index.md`.

| Port | Start (`577d472d`) | Now (batch 7) | Bar | Gap |
|---|---|---|---|---|
| katexlil (complete `katex.esm.js`) | 65,727 | 64,886 | 63,044 | +1,842 |
| markedlil (`marked.raw.js`) | 9,397 | 9,300 | 10,092 | **win −792** |
| posthoglil (`posthog.raw.js`) | 5,952 | 5,620 | 5,622 | level (−2) |
| jquerylil (`jquery.esm.js`, both with banners) | 33,593 | 31,373 | 27,445 | +3,928 |
| zodlil (complete package: `dist/index.js` bundled, hand-written JS unminified) | open | 45,720 | 51,948 | **win −6,228** |
| motionlil (`full.js`) | does not build | 52,080 (batch 4) | 41,032 | +11,048 |

| Raw objective | Start | Now (batch 7) | Bar | Gap |
|---|---|---|---|---|
| markedlil (`marked.bytes.js`) | 39,687 | 36,460 | 37,022 | **win −562** |
| posthoglil (`posthog.bytes.js`) | 19,750 | 18,350 | 16,123 | +2,227 |
| zodlil (complete package, built with the raw objective) | open | 250,283 | 274,999 | **win −24,716** |
| katexlil, jquerylil, motionlil | no raw configuration yet | | | |

Until batch 7 the zodlil raw row showed 252,517, which was the raw size of the Brotli-objective package. The row now uses a raw-objective build.

The default route, for reference with the same binary: posthoglil 5,602 (it also loses once its `esm.js` banner is counted), katexlil 64,620–64,907, jquerylil 28,764. motionlil's committed `full.js` is 50,526.

## Known causes, by port

- **jquerylil:** measured against the default route's 28,764 with the same binary, and estimated by editing our output (batch 7 in `docs/migration/index.md`):
  - The carried `js-host.ts` is 9.1 KB raw and 2,189 Brotli on its own. Only 41 of its 102 functions are used, and each call spells the long host name. Pruning and minifying the block is worth −1,204 Brotli.
  - The methods pass through `this` adapters at 117 sites (`function(a){return function(){return a(this,arguments)}}`), and their bodies read arguments by position.
  - The output is statement-heavy: 1,224 `if(` against 303, and 957 `let` against 28.
  - The 65 single-use struct encoders are gone in batch 7 (−794).
- **motionlil:** 12 class names are declared in two modules (`JSAnimation`, `GroupAnimation`). The semantic checker keys classes and enums by bare name (`Type::Class(&str)`), which rejects that; the default route's linker qualifies them. Owed as a language fix; a rename patch unblocks measurement.
- **katexlil:** the port is an untyped transliteration (3,990 `JsValue`, 394 `toNum`, no `pure`). Terser still finds 1.7% in our output, mostly single-use functions.
- **posthoglil:** string arrays are not packed, small `JsValue` wrappers are not inlined, and the port declares builtins through `JsValue` (`JS.number(JS.invoke(Math,"trunc",x))`).

## zodlil's boundary (013-T5)

The port's `z` has 191 members, against zod@4's 238, with the same 52 locales. It lacks the string-format classes (`ZodEmail`, `ZodURL` and the rest) and the check helpers (`gt`, `lte`, `length`, `includes`), and adds 10 of its own. The bar is therefore upstream restricted to the 181 shared members. Our side is everything `import { z } from "@itslil/zod"` loads, bundled by esbuild with nothing minified: the compiled `zod.core.js` plus the port's hand-written JavaScript (`compat.js`, `visit.js`, `async-api.js`, `official-json-schema.js`, `regexes.js`), and upstream's `zod/v4/locales`, which `compat.js` requires. Counting unminified hand-written code against us makes this bound conservative.
