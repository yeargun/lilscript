# Terser, Oxc, esbuild, and Vite

Parent: [research](README.md). Harness contract:
[baseline toolchains](../verification/baseline-toolchains.md).

These tools answer different questions. Treating them as interchangeable “minifiers”
hides whether a win came from parsing/compression, bundling/tree shaking, target
lowering, or application chunking.

## Roles in this repository

| Tool | Role | Current pinned popular-lab version |
|---|---|---:|
| Terser | mature single-artifact compressor/mangler | 5.43.1 |
| Oxc minifier through Rolldown | fast compressor/mangler with a different rewrite set | Rolldown 1.2.3 |
| esbuild | transform/minify and simple bundle baseline | 0.28.1 |
| Vite | production application graph/assets/chunks; selectable JS minifier | 8.2.1 |

Versions above are read from `benchmarks/popular/package.json` and its lockfile in
this checkout. They must be captured from the actual resolved install in reports.

The current micro runner invokes Terser, Oxc through Rolldown's minifier utility, and
esbuild, records their exact artifacts and semantic validity, and chooses a separate
minimum for every metric. Vite remains an application-graph baseline rather than a
single-file micro-suite lane.

## Terser lessons

The official [options reference](https://terser.org/docs/options/) exposes many useful
ablation axes: multiple compressor passes, top-level DCE/mangling, property mangling,
`pure_funcs`, `pure_getters`, unsafe built-in assumptions, comparison reversal,
function reduction, and output-format choices.

Translate them carefully:

- `pure_funcs` becomes a typed/inferred effect proof, not a string allowlist that can
  ignore a shadowed binding;
- `unsafe` built-in shortening becomes a known-host intrinsic only when mutation,
  exception, evaluation-order, and `NaN`/`valueOf` behavior are proved;
- `passes > 1` motivates fixed-point/phase-order cases, not an unconditional loop;
- property mangling needs ownership/escape/public-boundary proof;
- comparison flips must preserve operand evaluation order and numeric edge semantics.

## Oxc lessons

Oxc’s official [minifier guide](https://oxc.rs/docs/guide/usage/minifier.html) lists
dead-code elimination, syntax normalization for shorter/repetitive output, variable
mangling, and whitespace/comment removal, and separately calls out assumptions. It is
valuable because its fast implementation and rewrite mix can find a different
baseline minimum from Terser. “Fast” is a compile-time result; a LilScript candidate
must still win its exact selected raw, gzip, or Brotli metric and behavior gate.
Claiming all three requires three independently selected LilScript artifacts.

Use Oxc failures to add small grammar/control-flow/property cases. Do not silently
exclude Oxc when it beats Terser under one codec, and do not enable an assumption that
the case’s host/built-in contract does not justify.

## esbuild and Vite lessons

esbuild is useful as a fast, common transform/minify/bundle baseline. Vite owns a
larger production boundary: resolution, tree shaking, chunks, dynamic imports, CSS,
assets, preload behavior, and its selected minifier.

The current Vite [build options](https://vite.dev/config/build-options.html) make the
minifier explicit (`oxc`, `terser`, `esbuild`, or disabled); Vite 8’s documented
client default is Oxc. Always record that choice. A “Vite result” without minifier,
target, format, input graph, and asset/preload config is not reproducible.

## Harness matrix

- micro single-file: Terser, Oxc, esbuild; Closure only when closed-world eligible;
- static module graph: esbuild bundle, Vite production, Closure with matching entries;
- mixed app/assets: Vite/Lilpack paired builds;
- reusable library: matching ESM/public exports; watch Vite library-mode whitespace
  behavior;
- post-minified LilScript: ablation only, never the primary language/compiler row.
  Closed-world apps should not need Terser/Oxc to finish unused-import DCE, known
  DOM lowering, `document` spelling, exclusive-closure expressions, or captured
  arrow bindings; those belong in `lower_known_js_host_calls`,
  `prune_unused_foreign_imports`, and JS emission.

For every metric, choose the smallest eligible artifact independently. Keep compile
time beside size so LilScript can expose fast/default/maximum search tradeoffs rather
than hiding them.
