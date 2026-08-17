# LilScript knowledge tree

This tree is the reasoning map for LilScript: why the language exists, how compilation behaves, and how `lilscript.toml` changes those behaviors.

Read top-down. Each page is written so a later change can be judged against the same goals without reloading the whole compiler. Canonical contracts stay in sibling `docs/` files; this tree explains **perspective** and **config-driven behavior**.

## How to use this tree

| If you need to… | Start here |
|---|---|
| Decide whether a language change is in-mission | [Mission](mission.md) → [Language](language/README.md) |
| Decide whether a compiler change seeks a global codec win | [Global optima](compilation/global-optima.md) → [Candidate search](compilation/candidate-search.md) |
| Change or add a TOML knob | [Config](config/README.md) → the matching child page |
| Reason about bundling, lazy loading, or progressive enhancement | [Delivery](delivery/README.md) → [Modules in the language](language/modules-lazy.md) |
| Judge a size regression against jQuery / Closure | [Evidence](evidence/README.md) |
| Add or review a paired compression case | [Verification](verification/README.md) → [Paired-case contract](verification/paired-case-contract.md) |
| Add a multi-function compression challenge | [Algorithm challenges](verification/algorithm-challenges.md) |
| Pick the next compression milestone | [Active migration](migration/README.md) |
| Import an idea from Closure/Terser/Oxc | [Research](research/README.md) |

## Tree

1. [Mission](mission.md) — compression-first web language; types instead of glue; tradeoff triangle
2. [Language](language/README.md)
   - [Types are not glue](language/types-not-glue.md)
   - [Numerics and value semantics](language/numerics-values.md)
   - [Functions, closures, and generics](language/functions-closures-generics.md)
   - [Control flow and exceptions](language/control-flow-errors.md)
   - [Collections and typed intrinsics](language/collections-intrinsics.md)
   - [Async, generators, and regex](language/async-generators-regex.md)
   - [Closed world](language/closed-world.md)
   - [Packages, exports, and ABI](language/packages-exports-abi.md)
   - [Boundaries and escape](language/boundaries-escape.md)
   - [Aggregates](language/aggregates.md)
   - [Effects and purity](language/effects-purity.md)
   - [Modules, lazy loading, progressive enhancement](language/modules-lazy.md)
   - [JavaScript vs native](language/js-vs-native.md)
3. [Compilation](compilation/README.md)
   - [Global optima](compilation/global-optima.md)
   - [Pipeline](compilation/pipeline.md)
   - [Frontend, linking, and lowering](compilation/frontend-linking-lowering.md)
   - [Analyses and proof invalidation](compilation/analyses.md)
   - [IR optimizer](compilation/ir-optimizer.md)
   - [DCE and tree shaking](compilation/dce-tree-shaking.md)
   - [Inlining, specialization, and sharing](compilation/inlining-specialization-sharing.md)
   - [Aggregate lowering](compilation/aggregate-lowering.md)
   - [Compress passes](compilation/compress-passes.md)
   - [JavaScript emission](compilation/javascript-emission.md)
   - [Mangling, layout, and pooling](compilation/mangling-layout-pooling.md)
   - [Candidate search](compilation/candidate-search.md)
   - [Parsed peephole](compilation/peephole.md)
   - [Chunk planning](compilation/chunk-planning.md)
   - [Native backend](compilation/native-backend.md)
   - [Correctness and fallbacks](compilation/correctness-fallbacks.md)
4. [Config](config/README.md)
   - [Discovery and precedence](config/discovery-precedence.md)
   - [`[package]`, dependencies, and lockfiles](config/package-dependencies.md)
   - [`[optimization]`](config/optimization.md)
   - [`javascript.priority`](config/javascript-priority.md)
   - [`javascript.compression`](config/compression-decisions.md)
   - [`javascript.optimizations` and levels](config/javascript-optimizations.md)
   - [Cost model and search budgets](config/cost-model.md)
   - [`[mangle]`](config/mangle.md)
   - [JavaScript shape and ABI](config/javascript-shape-abi.md)
   - [Startup and performance](config/startup-performance.md)
   - [`[bundle]`](config/bundle.md)
   - [`[profile]`](config/profile.md)
   - [`[native]`](config/native.md)
   - [`[lint]` / `[format]`](config/lint-format.md)
   - [Tradeoff matrix](config/tradeoffs.md)
   - [Behavior matrix](config/behavior-matrix.md)
   - [Build profiles](config/build-profiles.md)
5. [Delivery](delivery/README.md)
   - [Lilpack](delivery/lilpack.md)
   - [Progressive enhancement](delivery/progressive-enhancement.md)
   - [Reusable library vs closed app](delivery/library-vs-app.md)
   - [Manual bundling](delivery/manual-bundling.md)
   - [Typed lazy loading](delivery/lazy-loading.md)
   - [Chunk cost, cache, and preload](delivery/chunk-cache-preload.md)
6. [Evidence](evidence/README.md)
   - [Paired web micro suite](evidence/micro-suite.md)
   - [Structural algorithm suite](evidence/algorithm-suite.md)
   - [Corpora and lanes](evidence/corpora-and-lanes.md)
   - [Configuration ablations](evidence/config-ablations.md)
   - [Negative results](evidence/negative-results.md)
   - [Toolchain provenance](evidence/toolchain-provenance.md)
   - [jQuery port](evidence/jquery.md)
   - [Closure and corpus](evidence/closure-comparison.md)
7. [Verification](verification/README.md)
   - [Paired-case contract](verification/paired-case-contract.md)
   - [Case layout](verification/case-layout.md)
   - [Baseline toolchains](verification/baseline-toolchains.md)
   - [Codec measurement](verification/codec-measurement.md)
   - [Coverage matrix](verification/coverage-matrix.md)
   - [Algorithm challenges](verification/algorithm-challenges.md)
   - [Config matrix](verification/config-matrix.md)
   - [Browser and host cases](verification/browser-host-cases.md)
   - [Failure triage](verification/failure-triage.md)
   - [Release gates](verification/release-gates.md)
8. [Active migration](migration/README.md)
   - [00 — canonical folder runner](migration/00-canonical-runner.md)
   - [01 — scalars and folding](migration/01-scalars-folding.md)
   - [02 — control flow and functions](migration/02-control-functions.md)
   - [03 — aggregates and typed wins](migration/03-aggregates-wins.md)
   - [04 — collections and effects](migration/04-collections-effects.md)
   - [05 — modules, search, compiler bugs](migration/05-modules-search.md)
   - [06 — scale and release](migration/06-scale-release.md)
9. [Research](research/README.md)
   - [Closure ADVANCED](research/closure-advanced.md)
   - [Terser, Oxc, esbuild, and Vite](research/terser-oxc-vite.md)
   - [Gzip and Brotli](research/gzip-brotli.md)
10. [Old migration plans](old-migration/README.md) — catalog-era 00–09 and the 2026-08 compression queue

## Canonical contracts (not this tree)

These remain the source of truth for syntax, schema, and pass lists. The knowledge tree cites them rather than replacing them.

| Contract | Path |
|---|---|
| Language v0.1 | [`docs/language-v0.1.md`](../language-v0.1.md) |
| TOML schema | [`docs/configuration.md`](../configuration.md) |
| Modules and delivery | [`docs/modules-and-delivery.md`](../modules-and-delivery.md) |
| Closure mapping / pass schedule | [`docs/optimization-coverage.md`](../optimization-coverage.md) |
| Web host ABI | [`docs/web-platform.md`](../web-platform.md) |
| Roadmap / completion rule | [`docs/roadmap.md`](../roadmap.md) |
| Differential testing | [`docs/differential-testing.md`](../differential-testing.md) |
| Manifesto stub | [`why-lilscript.md`](../../why-lilscript.md) |

## Source map

| Area | Primary files |
|---|---|
| Config → behavior | `src/config.rs` |
| Pipeline, search, chunks | `src/compiler.rs` |
| IR optimization | `src/optimizer.rs`, `src/compress_passes.rs`, `src/value_analysis.rs` |
| JS emission | `src/codegen_ir_js.rs`, `src/js_peephole.rs` |
| Modules / lazy | `src/module.rs` |
| Types / escape | `src/semantic.rs`, `src/ir.rs` |
| Native | `src/codegen_native.rs` |
| App bundler | `src/bin/lilpack.rs`, `tooling/lilpack/vite-runtime.mjs` |
| Paired micro comparison | `comparison/cases/` |
| Structural algorithm comparison | `comparison/algorithms/` |
| Closure app comparison | `comparison/apps/`, `comparison/lib/` |
| Differential oracle | `src/bin/lilscript-differential.rs`, `src/interpreter.rs` |
| Browser/runtime evidence | `benchmarks/browser/`, `benchmarks/paired/` |
| Popular-library evidence | `benchmarks/popular/` |
