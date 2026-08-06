# LilScript Engineering Roadmap

This file is the durable project direction. It separates implemented behavior
from measured next work so optimization claims remain testable.

## Definition of complete

A feature is complete only when its language semantics are documented, its
JavaScript and native behavior agree where both targets apply, regression tests
cover success and failure cases, and size/runtime claims are reproduced by a
checked-in benchmark workload. Passing a synthetic example alone is not enough.

## Current foundation

- Standalone type-first language, parser, semantic analysis, typed CFG/SSA IR.
- Static module linking, cross-file optimization, tree shaking, static chunks.
- Constant propagation, algebraic simplification, GVN, inlining,
  devirtualization, escape analysis, scalar replacement, DSE, and DCE.
- Frequency-ranked identifier mangling, typed property dissolution, profitable
  string pooling, and configurable JavaScript size/performance policy.
- JavaScript, portable C, and native executable output with cross-backend tests.
- Typed web host boundaries, arrays, maps, sets, buffers, shared buffers, and
  byte views.
- Native LSP, VS Code extension, Vite playground, docs, and benchmark labs.

## Optimization program

### Semantics-aware JavaScript cost model

- [x] Separate JavaScript optimizer policy from native policy.
- [x] Add performance-first, realistic-performance-first, balanced, and
  size-first profiles with explicit inlining budgets.
- [x] Add an exact allowlist for contested compression decisions.
- [x] Elide integer normalization only when range analysis proves signed i32
  behavior is unchanged; keep eager normalization available for numeric code.
- [ ] Add loop induction-variable range analysis so bounded UI/application
  loops do not pay unnecessary `|0` costs.
- [ ] Add interprocedural argument/return ranges and field ranges.
- [ ] Model deoptimization-sensitive JavaScript shapes, allocation pressure,
  and monomorphic call sites in performance decisions.
- [ ] Add optional profile-guided hot-function and hot-loop data without making
  source annotations mandatory.

### Compression and entropy

- [x] Measure raw, gzip-9, and Brotli-11 output independently.
- [x] Rank short identifiers by whole-program use frequency.
- [x] Pool strings only when the local raw-byte model predicts a win.
- [ ] Replace local raw-byte estimates with selectable raw/gzip/Brotli cost
  models that account for repeated token context.
- [ ] Search equivalent expression forms, declaration layouts, quote styles,
  and identifier assignments using a bounded compressor-in-the-loop pass.
- [ ] Add entropy-aware cross-scope name reuse and property-name assignment.
- [ ] Add post-codegen superoptimization with semantic differential tests.
- [ ] Track parse/compile cost and memory alongside transfer size so extreme
  compression choices do not silently damage startup behavior.

### Whole-program optimization

- [x] Fixed-point direct inlining and single-use multi-block CFG inlining.
- [x] Purity/effect inference, checked `pure`, and trusted host purity contracts.
- [x] Struct/class scalar replacement and typed positional aggregate lowering.
- [ ] Add richer alias analysis for mutable arrays, maps, sets, and host calls.
- [ ] Add partial escape analysis and stack/region allocation for native output.
- [ ] Add specialization for generic and higher-order calls using call-site
  frequency and emitted-byte cost.
- [ ] Add loop-invariant code motion, strength reduction, unrolling policy, and
  vectorization candidates for native backends.
- [ ] Add profile-controlled allocation sinking and closure environment
  splitting.

## Modules and delivery

- [x] Static graph resolution, private namespaces, live ESM exports, tree
  shaking, preserve-module chunks, and shared static chunks.
- [ ] Define dynamic `import()` syntax and typed asynchronous module values.
- [ ] Emit lazy chunks with deterministic manifests, preload policy, and runtime
  failure handling.
- [ ] Add chunk graph optimization using request count, minimum bytes, shared
  reachability, cache stability, and gzip/Brotli costs.
- [ ] Add package resolution, lockfiles, reproducible dependency builds, and
  stable library ABI/versioning policy.

## Platform and native targets

- [x] Direct JavaScript host calls with stable external names and no wrappers.
- [x] Portable C lowering for core language values and binary memory.
- [ ] Generate a versioned browser declaration package for DOM, events, fetch,
  timers, workers, storage, streams, canvas, and Web APIs.
- [ ] Add `DataView`, typed numeric arrays, `Atomics`, worker declarations, and
  explicit shared-memory concurrency semantics.
- [ ] Define exceptions, promises, async functions, and cancellation semantics.
- [ ] Add a direct machine-code backend after the C ABI and semantic corpus are
  stable; C remains the portable native reference backend.

## Tooling and quality

- [x] Diagnostics, completion, hover, symbols, syntax highlighting, and editor
  packaging.
- [ ] Add formatter, import organizer, rename/references, semantic tokens, and
  incremental workspace analysis.
- [ ] Add a configurable linter for correctness, allocation, boundary safety,
  bundle cost, and suspicious purity declarations.
- [ ] Emit source maps and optimization explanations from source through SSA to
  JavaScript chunks.
- [ ] Add incremental compilation and persistent module caches for Vite-class
  reload latency.

## Evidence expansion

- [x] Compiler corpus against Closure ADVANCED with executable output checks.
- [x] Application lanes for reactive state, events, binary memory, and modules.
- [ ] Motion/spring sampling lane using the official Motion package as an
  ecosystem baseline and a behavior-equivalent LilScript implementation.
- [ ] Add router, validation, parser, state-machine, worker/buffer, and DOM
  application lanes without claiming complete library rewrites.
- [ ] Run browser benchmarks for parse, startup, animation-frame stability,
  steady-state throughput, memory, and transfer compression.
- [ ] Add differential fuzzing against a reference interpreter and native C.
- [ ] Publish every checked-in project, source scope, behavior contract, raw,
  gzip, Brotli, and runtime result in the Vite documentation site.

## Release gates

1. No backend behavior mismatch in the full verification matrix.
2. No benchmark is measured before all compared artifacts pass one contract.
3. Every optimization has an off mode or a semantics-preserving justification.
4. Public size claims identify the corpus, tool versions, and compression mode.
5. External-library comparisons distinguish app-level specialization from a
   complete compatible library implementation.
6. The repository docs and website are regenerated when checked-in benchmark
   results change.
