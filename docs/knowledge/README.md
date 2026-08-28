# LilScript knowledge tree

Read **top-down**. Each child is more specific than its parent. Canonical syntax
and schema stay in sibling `docs/` contracts; this tree explains **why** and
**how compilation behaves**.

## How to use this tree

| If you need to… | Start here |
|---|---|
| Decide whether a language change is in-mission | [Mission](mission.md) → [Language](language/README.md) |
| Decide whether a compiler change seeks a global codec win | [Current architecture](compilation/current-architecture.md) → [Goal architecture](compilation/goal-architecture.md) → [Objectives](compilation/objectives.md) → [Decision registry](compilation/decision-registry.md) |
| Ask whether class/struct/inlining is searched or a heuristic | [Decision registry](compilation/decision-registry.md) |
| Ask why a forked library still loses to Terser | [Compressor surface](language/compressor-surface.md) |
| Change or add a TOML knob | [Config](config/README.md) |
| Reason about bundling or lazy loading | [Delivery](delivery/README.md) |
| Judge a size regression | [Evidence](evidence/README.md) |
| Add a paired compression case | [Verification](verification/README.md) |
| Pick the next milestone | [Migration](migration/README.md) → [board](migration/board/README.md) |
| Import an idea from Closure/Terser/Oxc | [Research](research/README.md) |

## Tree

1. [Mission](mission.md)
2. [Language](language/README.md)
   - Types: [not glue](language/types-not-glue.md), [compressor surface](language/compressor-surface.md), [numerics](language/numerics-values.md)
   - Programs: [functions](language/functions-closures-generics.md), [control](language/control-flow-errors.md), [effects](language/effects-purity.md)
   - Data: [aggregates](language/aggregates.md), [collections](language/collections-intrinsics.md), [async / regex](language/async-generators-regex.md)
   - World: [closed world](language/closed-world.md), [packages](language/packages-exports-abi.md), [boundaries](language/boundaries-escape.md), [modules](language/modules-lazy.md)
   - [JavaScript vs native](language/js-vs-native.md)
3. [Compilation](compilation/README.md)
   - Overview: [current architecture](compilation/current-architecture.md), [goal architecture](compilation/goal-architecture.md), [objectives](compilation/objectives.md), [decision registry](compilation/decision-registry.md), [global optima](compilation/global-optima.md), [pipeline](compilation/pipeline.md)
   - Frontend: [linking / lowering](compilation/frontend-linking-lowering.md), [analyses](compilation/analyses.md)
   - IR: [optimizer](compilation/ir-optimizer.md), [DCE](compilation/dce-tree-shaking.md), [inlining](compilation/inlining-specialization-sharing.md), [aggregates](compilation/aggregate-lowering.md), [class identity](compilation/class-identity.md), [compress passes](compilation/compress-passes.md)
   - JavaScript: [emission](compilation/javascript-emission.md), [mangling](compilation/mangling-layout-pooling.md), [search](compilation/candidate-search.md), [peephole](compilation/peephole.md), [chunks](compilation/chunk-planning.md)
   - [Native](compilation/native-backend.md), [correctness](compilation/correctness-fallbacks.md)
4. [Config](config/README.md)
   - Layers: [discovery](config/discovery-precedence.md), [package](config/package-dependencies.md), [optimization](config/optimization.md)
   - JS policy: [priority](config/javascript-priority.md), [compression](config/compression-decisions.md), [optimizations](config/javascript-optimizations.md), [cost model](config/cost-model.md)
   - ABI / shape: [mangle](config/mangle.md), [shape](config/javascript-shape-abi.md), [startup](config/startup-performance.md)
   - After optimize: [bundle](config/bundle.md), [profile](config/profile.md), [native](config/native.md), [lint](config/lint-format.md)
   - Matrices: [tradeoffs](config/tradeoffs.md), [behavior](config/behavior-matrix.md), [profiles](config/build-profiles.md)
5. [Delivery](delivery/README.md)
   - [Lilpack](delivery/lilpack.md), [library vs app](delivery/library-vs-app.md), [manual bundling](delivery/manual-bundling.md)
   - [Lazy loading](delivery/lazy-loading.md), [progressive enhancement](delivery/progressive-enhancement.md), [chunk cache](delivery/chunk-cache-preload.md)
6. [Evidence](evidence/README.md)
   - Suites: [micro](evidence/micro-suite.md), [algorithms](evidence/algorithm-suite.md), [corpora](evidence/corpora-and-lanes.md)
   - Method: [ablations](evidence/config-ablations.md), [negatives](evidence/negative-results.md), [provenance](evidence/toolchain-provenance.md)
   - Ports: [jQuery](evidence/jquery.md), [Closure](evidence/closure-comparison.md), [Motion](evidence/motion-compatibility.md)
   - Numbers: [benchmark results](evidence/benchmark-results.md), [post-minify audit](evidence/vite-closure-minification-audit.md)
7. [Verification](verification/README.md)
   - Contract: [paired cases](verification/paired-case-contract.md), [layout](verification/case-layout.md), [baselines](verification/baseline-toolchains.md), [codec](verification/codec-measurement.md)
   - Coverage: [matrix](verification/coverage-matrix.md), [algorithms](verification/algorithm-challenges.md), [config](verification/config-matrix.md), [host](verification/browser-host-cases.md)
   - Process: [triage](verification/failure-triage.md), [release](verification/release-gates.md)
8. [Migration](migration/README.md)
   - [Board](migration/board/README.md)
   - 00–06 standing evidence loop; [07 — global compressor](migration/07-global-compressor.md) is the current architecture plan (board `ident-05` / `arch-02`–`arch-07`)
9. [Research](research/README.md)
   - [Closure ADVANCED](research/closure-advanced.md), [Terser / Oxc / Vite](research/terser-oxc-vite.md), [gzip / Brotli](research/gzip-brotli.md)
   - Labs (subfolders): [Brotli machine](research/brotli-machine.html), [aligned mangling](research/aligned-mangling/README.md), [global mangle](research/brotli-global-mangle/README.md)

## Contracts (not this tree)

| Contract | Path |
|---|---|
| Language v0.1 | [`docs/language-v0.1.md`](../language-v0.1.md) |
| TOML schema | [`docs/configuration.md`](../configuration.md) |
| Modules and delivery | [`docs/modules-and-delivery.md`](../modules-and-delivery.md) |
| Closure mapping | [`docs/optimization-coverage.md`](../optimization-coverage.md) |
| Web host ABI | [`docs/web-platform.md`](../web-platform.md) |
| Roadmap | [`docs/roadmap.md`](../roadmap.md) |
| Differential testing | [`docs/differential-testing.md`](../differential-testing.md) |

## Source map

| Area | Primary files |
|---|---|
| Config → behavior | `src/config.rs` |
| Pipeline, search, chunks | `src/compiler.rs` |
| IR | `src/optimizer.rs`, `src/compress_passes.rs`, `src/value_analysis.rs` |
| JS emission | `src/codegen_ir_js.rs`, `src/js_peephole/` |
| Types / escape | `src/semantic.rs`, `src/ir.rs`, `src/lower.rs` |
| Native | `src/codegen_native.rs` |
| Paired cases | `comparison/cases/` |
| Algorithms / Closure apps | `comparison/algorithms/`, `comparison/apps/` |
