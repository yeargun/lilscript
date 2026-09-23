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

| Port | Start (`577d472d`) | Now (batch 16) | Bar | Gap |
|---|---|---|---|---|
| katexlil (complete `katex.esm.js`; without our 76-byte banner, which the bar lacks: 63,232) | 65,727 | 63,239 | 63,044 | +195 (like for like +188) |
| markedlil (`marked.raw.js`) | 9,397 | 9,258 | 10,092 | **win −834** |
| posthoglil (`posthog.raw.js`) | 5,952 | 5,593 | 5,622 | **win −29** |
| jquerylil (`jquery.esm.js`, both with banners) | 33,593 | 29,340 | 27,445 | +1,895 |
| zodlil (complete package: `dist/index.js` bundled, hand-written JS unminified) | open | 45,639 | 51,948 | **win −6,309** |
| motionlil (`full.js`) | does not build | 51,321 | 41,032 | +10,289 |

| Raw objective | Start | Now (batch 16) | Bar | Gap |
|---|---|---|---|---|
| katexlil (`katex.esm.js`) | first built in batch 9: 272,347 | 251,769 | 267,050 | **win −15,281** |
| markedlil (`marked.bytes.js`) | 39,687 | 35,393 | 37,022 | **win −1,629** |
| posthoglil (`posthog.bytes.js`) | 19,750 | 16,351 | 16,123 | +228 |
| jquerylil (`jquery.esm.js`) | first built in batch 9: 91,134 | 86,618 | 87,151 | **win −533** |
| zodlil (complete package, built with the raw objective) | open | 244,615 | 274,999 | **win −30,384** |
| motionlil | no raw configuration yet | | | |

katexlil and jquerylil have no raw configuration of their own. Their rows build the port with `cost_model = "raw"` in every configuration, which is how the raw-objective suites run too.

**zodlil's package, measured two ways.** The package is esbuild's bundle of `dist/index.js`, which reprints every file. Without minification it also pretty-prints our minified core. That is the conservative figure in both tables (45,639 Brotli, 244,615 raw; the pretty-printing inflates nested code, so the raw figure grew when batch 13 nested tails in `else` blocks). With `--minify-whitespace`, our core keeps its spelling and the hand-written glue only loses its whitespace: 41,671 Brotli (Brotli objective) and 181,521 raw (raw objective).

Until batch 7 the zodlil raw row showed 252,517, which was the raw size of the Brotli-objective package. The row now uses a raw-objective build.

The default route, for reference with the same binary: posthoglil 5,602 (it also loses once its `esm.js` banner is counted), katexlil 64,620–64,907, jquerylil 28,095 (batch 9 binary; 28,764 with the older binary). motionlil's committed `full.js` is 50,526. Batch-4-era binaries give 53,077 for motionlil's `full.js` with the committed patch; the 52,080 reported at batch 4 was measured differently.

## Known causes, by port

- **jquerylil:** measured against the default route's 28,764 with the same binary, and estimated by editing our output (batches 7 and 8 in `docs/migration/index.md`):
  - Batch 8 lowers the carried `js-host.ts` into the program (−1,818). Its unused functions go, and its wrappers inline.
  - The methods pass through `this` adapters at 117 sites (`function(a){return function(){return a(this,arguments)}}`). Dissolving them into `function(){…this…}` measured +42 Brotli, since `this` is longer than the parameter it replaces, so they stay.
  - The output is statement-heavy: 1,224 `if(` against 303, and 957 `let` against 28. Terser's full compression over our output finds only −246, so spelling is not the main cause.
  - The 65 single-use struct encoders are gone in batch 7 (−794).
- **motionlil:** feature by feature against upstream bundles with the same exports, the small entries carry large fixed costs: viewport 717 against 345, mini 10,980 against 4,446. Classes are constructed as a null-filled literal plus an `init` call, so every value type is a module-level effect nothing can prune, and it keeps color parsing and the frame loop in every entry (batch 8 in the plan). Duplicate module variants are only 4.5 KB raw of a 230 KB core. The port's `full.js` is esbuild plus Terser over our output, and that reprint costs about 1.1 KB Brotli over our own core (51,757 against 52,893).
- **katexlil:** the port is an untyped transliteration (3,990 `JsValue`, 394 `toNum`, no `pure`). Terser still finds 1.7% in our output, mostly single-use functions.
- **posthoglil:** string arrays are not packed, small `JsValue` wrappers are not inlined, and the port declares builtins through `JsValue` (`JS.number(JS.invoke(Math,"trunc",x))`).

## zodlil's boundary (013-T5)

The port's `z` has 191 members, against zod@4's 238, with the same 52 locales. It lacks the string-format classes (`ZodEmail`, `ZodURL` and the rest) and the check helpers (`gt`, `lte`, `length`, `includes`), and adds 10 of its own. The bar is therefore upstream restricted to the 181 shared members. Our side is everything `import { z } from "@itslil/zod"` loads, bundled by esbuild with nothing minified: the compiled `zod.core.js` plus the port's hand-written JavaScript (`compat.js`, `visit.js`, `async-api.js`, `official-json-schema.js`, `regexes.js`), and upstream's `zod/v4/locales`, which `compat.js` requires. Counting unminified hand-written code against us makes this bound conservative.
