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
- Module-level integer argument, return, and owned-field range analysis, plus
  allocation-root alias tracking for mutable built-in collections.
- Frequency-ranked identifier mangling, typed property dissolution, profitable
  string pooling, literal string-table packing, and configurable JavaScript
  size/performance policy.
- JavaScript, portable C, and native executable output with cross-backend tests.
- Typed web host boundaries, arrays, maps, sets, buffers, shared buffers, and
  byte views.
- Native LSP, VS Code extension, Vite playground, docs, and benchmark labs.

## Optimization program

### Semantics-aware JavaScript cost model

- [x] Separate JavaScript optimizer policy from native policy.
- [x] Add performance-first, realistic-performance-first, balanced, and
  size-first profiles with explicit inlining budgets.
- [x] Let size-first tolerate bounded temporary inline growth before rerunning
  constant folding and DCE; zero IR growth is not a final-byte cost model.
- [x] Add an exact allowlist for contested compression decisions.
- [x] Elide integer normalization only when range analysis proves signed i32
  behavior is unchanged; keep eager normalization available for numeric code.
- [x] Add loop induction-variable range analysis so bounded UI/application
  loops do not pay unnecessary `|0` costs.
- [x] Preserve source-written `Math.imul` while never introducing it for
  ordinary multiplication in the application-oriented profiles.
- [x] Add interprocedural argument/return ranges and field ranges.
- [ ] Model deoptimization-sensitive JavaScript shapes, allocation pressure,
  and monomorphic call sites in performance decisions.
- [ ] Add optional profile-guided hot-function and hot-loop data without making
  source annotations mandatory.

### Compression and entropy

- [x] Measure raw, gzip-9, and Brotli-11 output independently.
- [x] Rank short identifiers by whole-program use frequency.
- [x] Pool strings only when the local raw-byte model predicts a win.
- [x] Replace final-output raw-byte estimates with selectable raw/gzip/Brotli cost
  models that account for repeated token context.
- [x] Add deterministic bounded compressor-in-the-loop selection for contested
  final emission tactics.
- [x] Compare quote styles and emitted-character-ranked identifier alphabets
  with exact compressor scoring.
- [x] Compare compact and keyword boolean literals under the selected codec.
- [x] Compare nested structured closures, string-literal table packing, and
  ordinary array literals under the selected codec.
- [x] Compare tuple and scalar SSA parallel-copy layouts under the selected
  codec, reusing liveness-proven dead locals to break copy cycles.
- [x] Remove redundant expression parentheses at precedence-safe statement,
  assignment, argument, and return boundaries.
- [ ] Score optimizer-level IR variants, not only final emission variants, so
  inlining, specialization, loop shape, and SSA destruction can compete under
  the selected codec.
- [ ] Add a precedence-carrying JavaScript expression IR to remove redundant
  interior parentheses without parsing generated strings.
- [ ] Expand candidate search to declaration grouping, conditional/comma
  expressions, `while`/`do`/`for` loop layouts, switch lowering, and local
  mutation forms.
- [ ] Add entropy-aware cross-scope name reuse and property-name assignment.
- [ ] Add a parsed post-codegen peephole/superoptimizer whose every rewrite is
  differential-tested against optimized and disabled-optimizer executions.
- [ ] Track parse/compile cost and memory alongside transfer size so extreme
  compression choices do not silently damage startup behavior.

### Whole-program optimization

- [x] Fixed-point direct inlining and single-use multi-block CFG inlining.
- [x] Purity/effect inference, checked `pure`, and trusted host purity contracts.
- [x] Constant-parameter specialization plus unused direct-call parameter and
  return-value elimination.
- [x] Fold control flow exposed by literal closure captures during final
  JavaScript emission.
- [x] Struct/class scalar replacement and typed positional aggregate lowering.
- [x] Add allocation-root alias analysis for mutable arrays, maps, sets, and
  host calls so an unobserved local mutation graph is removed as a unit while
  an observed result, escape, capture, or boundary call preserves it.
- [ ] Add partial escape analysis and stack/region allocation for native output.
- [ ] Add specialization for generic and higher-order calls using call-site
  frequency and emitted-byte cost.
- [ ] Clone higher-order factories by constant capture signature so each
  returned closure reaches the normal constant-fold/DCE fixed point.
- [ ] Add interprocedural value sets, array lengths, return ranges, and nominal
  field constants.
- [x] Coalesce loop-header and conditional loop-carried phis with their dead
  incoming values so common mutations no longer require temporary copy chains.
- [ ] Complete SSA destruction across multi-exit loops, nested merges, parallel
  copy cycles, and deferred expressions using a byte-scored register allocator.
- [x] Add context-sensitive effect and alias summaries for arrays, maps, sets,
  known closures, and host calls. Parameter-mutation groups remove direct calls
  only when every affected allocation root is unobserved; inherent effects and
  unknown boundaries remain conservative.
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
- [ ] Score chunk candidates by full deploy cost: compressed bytes, request
  overhead, dependency depth, preload behavior, and long-term cache reuse.
- [ ] Add package resolution, lockfiles, reproducible dependency builds, and
  stable library ABI/versioning policy.

## Platform and native targets

- [x] Direct JavaScript host calls with stable external names and no wrappers.
- [x] Portable C lowering for core language values and binary memory.
- [ ] Generate a versioned browser declaration package for DOM, events, fetch,
  timers, workers, storage, streams, canvas, and Web APIs.
- [ ] Generate those declarations from pinned Web IDL, preserving overloads,
  inheritance, nullable contracts, and stable external property names.
- [ ] Add `DataView`, typed numeric arrays, `Atomics`, worker declarations, and
  explicit shared-memory concurrency semantics.
- [ ] Define exceptions, promises, async functions, and cancellation semantics.
- [ ] Add a direct machine-code backend after the C ABI and semantic corpus are
  stable; C remains the portable native reference backend.

## Tooling and quality

- [x] Diagnostics, completion, hover, symbols, syntax highlighting, and editor
  packaging.
- [x] Add a lossless syntax layer, formatter, import organizer, scope-aware
  rename/references, semantic tokens, and configuration-aware editor actions.
- [ ] Add incremental workspace analysis and persistent caches.
- [x] Add a configurable linter for correctness, allocation, boundary safety,
  bundle cost, and suspicious purity declarations.
- [x] Emit structured per-pass optimization explanations for single JavaScript
  builds.
- [ ] Emit source maps from source through SSA to JavaScript chunks.
- [ ] Add machine-readable optimization remarks and SARIF links from lint
  findings to the exact surviving IR allocation, call, or boundary operation.

## Evidence expansion

- [x] Compiler corpus against Closure ADVANCED with executable output checks.
- [x] Application lanes for reactive state, events, binary memory, and modules.
- [x] Build a real Motion package integration with Vite as context-only
  ecosystem evidence, isolated from comparable compiler totals.
- [x] Add a complete-root-entrypoint library lab for `@motionone/easing`,
  `clamp`, `lerp`, `string-hash`, `js-levenshtein`, `@emotion/hash`, and
  `murmurhash-js` with upstream and differential gates.
- [x] Add a separate SolidJS client-runtime lab: 2,355 lines of LilScript,
  109 adapted runtime behavior ports executed through optimized/unoptimized
  JavaScript, emitted C, and native output, plus the unchanged 469-test
  upstream reference suite.
- [ ] Port the remaining SolidJS client runtime behaviors before using the word
  compatible: stores, transitions, errors, promises/resources, complete DOM
  insertion/reconciliation, Suspense, hydration, and scheduling.
- [ ] Add real-browser interaction, scheduling, memory, and frame-time gates to
  the Solid client lab. TypeScript source compatibility and SSR are not goals;
  equivalent LilScript client behavior is the gate.
- [ ] Implement the audited Motion v13 package surface and pass applicable
  upstream unit/browser tests before publishing any compatibility comparison.
- [ ] Add router, validation, parser, state-machine, worker/buffer, and DOM
  application lanes without claiming complete library rewrites.
- [ ] Run browser benchmarks for parse, startup, animation-frame stability,
  steady-state throughput, memory, and transfer compression.
- [ ] Record JavaScript parse/compile time separately from execution and report
  confidence intervals rather than single-run timing winners.
- [x] Add a mechanically generated paired-source transfer-size gate and a
  Chromium steady-state runtime regression gate with confidence bounds.
- [ ] Add differential fuzzing against a reference interpreter and native C.
- [x] Publish every checked-in project, source scope, behavior contract, raw,
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
