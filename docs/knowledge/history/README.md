# History: the deleted compiler route

Parent: [knowledge tree](../README.md). The compiler today:
[current architecture](../compilation/current-architecture.md), the architecture
[future-architecture.md](../../future-architecture.md) and the plan
[migration/index.md](../../migration/index.md).

Until 2026-09-23 two compilers shared one binary behind `--backend`. The first
one (a typed CFG/SSA IR, an IR optimizer, a string emitter, a text peephole over
its own output and a search that re-emitted the whole program) was deleted in
plan phase M1, with its configuration keys. The pages below describe how it
worked. They are kept as prior art: plan rule 7 asks each batch to read the old
route's implementation of what it rebuilds, and the plan's "Old-route
optimizations: disposition" table says what replaces each pass. Nothing here
describes the compiler that exists now. The old sources remain in git history
before commit `6f6db845`.

## Compiler

| Page | What it recorded |
|---|---|
| [Architecture router](compilation/architecture.md) | Reading order for the pages below |
| [Current architecture, 2026-08-29](compilation/current-architecture.md) | The old pipeline: typed CFG/SSA, decision registry, search, text peephole |
| [Pipeline](compilation/pipeline.md) | Stage order for single, split and native builds |
| [Frontend, linking and lowering](compilation/frontend-linking-lowering.md) | The module linker and lowering to a CFG |
| [Analyses](compilation/analyses.md) | Effects, escape, ranges, alias and call facts |
| [IR optimizer](compilation/ir-optimizer.md) | Pass order and `[optimization]` gates |
| [DCE and tree shaking](compilation/dce-tree-shaking.md) | Dead-code passes |
| [Inlining, specialization, sharing](compilation/inlining-specialization-sharing.md) | Inlining fixed point, specialization, function folding |
| [Aggregate lowering](compilation/aggregate-lowering.md) | Scalar, positional and named layouts |
| [Class identity](compilation/class-identity.md) | When a constructor stayed an ES `class` |
| [Compress passes](compilation/compress-passes.md) | Fusion, sinking, outlining |
| [JavaScript emission](compilation/javascript-emission.md) | The string emitter and `IrJsOptions` |
| [Mangling, layout, pooling](compilation/mangling-layout-pooling.md) | Names, function order, literal pools |
| [Candidate search](compilation/candidate-search.md) | The two-level search, beam and budgets |
| [Decision registry](compilation/decision-registry.md) | The census of emission options and scored families |
| [Parsed peephole](compilation/peephole.md) | The text peephole over generated JavaScript |
| [Chunk planning](compilation/chunk-planning.md) | Split and preserve-modules planning |
| [Native backend](compilation/native-backend.md) | The first C emitter and `[native]` storage |
| [Correctness and fallbacks](compilation/correctness-fallbacks.md) | Fallback hierarchy and determinism |
| [Objectives](compilation/objectives.md) | `priority` × `cost_model` ranking |
| [Global optima](compilation/global-optima.md) | Why a locally smaller spelling can lose under a codec; candidate admission and ranking |
| [Optimization coverage](optimization-coverage.md) | Closure ADVANCED responsibilities mapped to the old passes |

## Configuration

These keys are retired: each now warns "no effect in this compiler" or is
refused ([configuration.md](../../configuration.md#retired-keys)).

| Page | Keys |
|---|---|
| [Discovery and precedence](config/discovery-precedence.md) | How the old option objects were derived |
| [`[optimization]`](config/optimization.md) | Per-pass switches |
| [`javascript.priority`](config/javascript-priority.md) | The four priorities and their inline budgets |
| [`javascript.compression`](config/compression-decisions.md) | The decision list |
| [`javascript.optimizations`](config/javascript-optimizations.md) | The search-family list and levels |
| [Cost model and search budgets](config/cost-model.md) | Candidate budgets and raw-growth admission |
| [`[mangle]`](config/mangle.md) | Export, property and extern-field mangling |
| [JavaScript shape and ABI](config/javascript-shape-abi.md) | Function spelling, layouts, naming |
| [Startup and performance](config/startup-performance.md) | Runtime-cost guards |
| [`[profile]`](config/profile.md) | Profile-guided specialization |
| [`[native]`](config/native.md) | C storage placement |
| [Tradeoff matrix](config/tradeoffs.md), [behavior matrix](config/behavior-matrix.md), [build profiles](config/build-profiles.md) | Knob recipes |

## Status

- [Implementation status, 2026-09-20](status-2026-09-20.md): the status page
  while both routes shipped.
- The milestone record 001–013 is [migration/record-2026-09.md](../../migration/record-2026-09.md).
