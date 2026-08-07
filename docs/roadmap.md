# LilScript Engineering Roadmap

## North star

LilScript is a standalone, statically typed web language whose compiler should
minimize the complete deploy cost of an application: transferred bytes,
requests, parse/compile work, memory, and runtime shape. Raw JavaScript size is
an input to that objective, not the objective by itself.

The project does not claim that one compiler can beat hand-specialized code or
Closure ADVANCED on every program. Source semantics, public identity, host
boundaries, and workload knowledge can make a smaller equivalent artifact
unavailable. The durable target is stronger: find more whole-program
opportunities, measure every contested representation with the selected codec,
and never retain a heuristic alternative that loses its configured objective.

## Completion rule

A capability is complete only when:

1. Its language and boundary semantics are explicit.
2. Optimized and optimizer-disabled execution agree.
3. JavaScript, emitted C, and native output agree where the feature is portable.
4. Differential and regression tests cover the proof conditions and bailouts.
5. A checked-in ablation demonstrates a win or records a neutral result.
6. User-visible claims name the corpus, tool versions, codec, and scope.

## Shipped capability map

### Language and analysis

- Type-first syntax, generics, higher-order functions, structs, classes,
  collections, binary memory, modules, and typed host declarations.
- Arena parser, scope and symbol analysis, type checking, capture analysis, and
  typed CFG/SSA IR.
- Interprocedural integer ranges, bounded boolean/string/null sets, exact stable
  array lengths, nominal field summaries, effect summaries, and allocation-root
  alias tracking.
- Checked purity, conservative host effects, escape classification, scalar
  replacement, and native stack/region allocation for eligible aggregates.

### Whole-program optimization

- Fixed-point constant/branch folding, algebraic simplification, GVN, DCE, DSE,
  devirtualization, direct and CFG inlining, unused parameter/return removal,
  and constant-parameter specialization.
- Profile-guided direct/higher-order specialization and closure-factory cloning
  by constant capture signature.
- Late identical private-function folding after inlining and specialization.
  Exported, address-taken, method, constructor, closure, and incompatible
  escape contracts remain distinct.
- Proof-driven private-function subsumption for typed scalar and known-function
  bindings. A temporary specialization must exactly equal the narrower
  normalized SSA/CFG; explicit call arguments replace the removed body, while
  exported and address-taken identities remain distinct. Untouched IR remains
  a codec-scored candidate.
- Struct/class dissolution, typed positional lowering, collection mutation-root
  elimination, and conservative boundary invalidation.

### Compression-oriented JavaScript

- Frequency-ranked names, cross-scope color reuse, typed property mangling,
  codec-tested alphabets and quotes, profitable string pooling, and optional
  string-table packing.
- Precedence-carrying JavaScript expression IR, structured/state-machine
  alternatives, conditional/comma forms, loop and mutation spellings, phi
  affinity, and scalar/tuple SSA-copy alternatives.
- Bounded candidate search scored by exact raw, gzip-9, or Brotli-11 size, with
  deterministic startup and JavaScript-shape guards.
- Similarity-ranked function declaration layout. Up to 13 declarations use a
  maximum-weight dynamic program; larger groups use deterministic best
  insertion. Source order remains a full-codec candidate, so the heuristic
  cannot force a transfer regression.
- Parsed post-codegen peepholes that rewrite only validated generated-JavaScript
  AST shapes and are differential-tested.
- Optimization levels `0..15`, exact search-feature allowlists, semantic pass
  switches, size/performance priorities, and optional profile data.

### Modules and delivery

- Static graph resolution, private namespaces, live ESM exports, whole-graph
  tree shaking, preserve-module output, shared chunks, and lazy `import()`.
- Deterministic manifests, preload policy, runtime load failures, package
  resolution, lockfiles, reproducible dependency builds, and versioned ABI
  policy.
- Chunk search accounts for raw/gzip/Brotli bytes, request overhead, dependency
  depth, shared reachability, preload behavior, and cache reuse.

### Platform, tooling, and evidence

- Direct zero-wrapper web host access plus JavaScript, portable C, and native
  executable output.
- LSP, VS Code extension, lossless formatter, import organizer, semantic rename
  and references, configurable lint providers, SARIF, Vite playground, and web
  documentation.
- Closure ADVANCED corpus, paired sources, package/library lanes, browser gates,
  native differential execution, and deterministic typed-program fuzzing.

Detailed measurements live in [benchmark-results.md](benchmark-results.md), and
pass boundaries live in [optimization-coverage.md](optimization-coverage.md).

## Decision protocol

Every speculative size optimization follows the same pipeline:

1. Prove semantic eligibility using types, effects, aliases, identity, and
   escape information.
2. Produce explicit IR or JavaScript-IR alternatives; do not rewrite generated
   strings.
3. Remove locally dominated candidates, but retain structurally different
   families in a bounded beam.
4. Score the complete emitted artifact with the configured codec.
5. Apply startup, memory, and selected performance-policy constraints.
6. Differential-test the winner and the disabled pass.
7. Promote the pass only after paired applications or libraries confirm the
   isolated result.

This design reflects actual codec behavior. DEFLATE has a 32 KiB backward
window ([RFC 1951](https://www.rfc-editor.org/rfc/rfc1951.html)); Brotli combines
LZ matching with context modeling and a static dictionary
([RFC 7932](https://www.rfc-editor.org/rfc/rfc7932.html)). Therefore declaration
order, repeated token runs, and distance can matter even when raw bytes do not,
but an entropy proxy is never accepted in place of final codec measurement.
Closure likewise schedules repeated simplification, inlining, unused-code,
property, variable, and late peephole phases rather than relying on one pass
([DefaultPassConfig.java](https://github.com/google/closure-compiler/blob/master/src/com/google/javascript/jscomp/DefaultPassConfig.java)).

## Ranked optimizer program

### P0: semantic and evidence coverage

- [ ] Extend the independent evaluator and differential generator to nominal
  aggregates, classes, maps/sets, async modules, exceptions, and typed host
  boundaries. No aggressive pass is trustworthy outside the modeled corpus.
- [ ] Add source-to-SSA-to-chunk source maps and machine-readable optimization
  remarks. Every surprising retained allocation, boundary, or failed merge
  should identify the failed proof.
- [ ] Add browser parse/compile and peak-memory measurements to release artifacts;
  deterministic syntax proxies remain selection tools, not browser claims.

### P1: highest-probability bundle wins

- [x] **Bound private-function subsumption.** Reuse an existing broader
  direct-call implementation when typed scalar or known-function bindings prove
  exact normalized SSA/CFG equality. The isolated ablation improves
  `445/217/179` to `351/201/172` raw/gzip/Brotli.
- [ ] **Generalized parameterized function merging.** Extend the proof to
  permuted parameters, then evaluate synthesized shared implementations for
  bodies differing by one operand. Preserve identity and retain every untouched
  artifact candidate.
- [ ] **Repeated-region outlining.** Discover repeated pure/effect-equivalent
  statement regions across functions and outline them only when helper calls
  win the complete codec objective within call-shape limits.
- [ ] **Path- and context-sensitive propagation.** Add relational branch facts,
  sparse conditional constant propagation, bounded call contexts, and
  field-sensitive memory SSA. Widen deterministically at loops, recursion,
  mutation, dynamic calls, and host boundaries.
- [ ] **Partial escape and allocation sinking for JavaScript.** Split closure
  environments, scalarize branch-local aggregate regions, and sink allocations
  to the paths that escape. Compare scalar, tuple, and materialized-object
  representations with allocation and deoptimization guards.
- [ ] **Generalized typed pipeline fusion.** Extend the existing direct array
  callback lowering to eligible `map`/`filter`/`reduce` chains and small
  higher-order collection pipelines without intermediate arrays or callbacks.
  Preserve callback order, mutation, exceptions, and observable lengths. Keep
  unfused code when inlining or compression makes it smaller.
- [ ] **Joint representation selection.** Search aggregate scalar/tuple/object,
  closure environment, switch/table, and array-of-struct versus
  struct-of-arrays forms using escape facts, hotness, emitted bytes, and engine
  shape stability.
- [ ] **Joint mangling and layout.** Extend declaration clustering to string pools
  and individual chunks; optimize symbol assignment over interference,
  frequency, surrounding tokens, cache-stable public names, and codec cost.
  Use bounded local search, not an unbounded permutation search.
- [ ] **Typed expression superoptimization.** Add a bounded equality-saturation or
  enumerative search for pure integer, float, boolean, and string expressions.
  Rewrites must encode JavaScript coercion, signed-zero, NaN, overflow, and
  evaluation-order rules and pass differential execution.
- [ ] **Joint chunk and symbol search.** Score chunk boundaries, declaration order,
  name stability, preload, request count, and cache reuse together. Current
  chunk and single-file layout searches are sound but separately optimized.
- [ ] **Package effect precision.** Persist typed side-effect/effect-summary
  metadata in library artifacts so consumers can tree-shake packages without
  reanalyzing source or trusting coarse handwritten `sideEffects` flags.

### P2: compiler throughput and runtime quality

- [ ] Add incremental module analysis, content-addressed IR/pass caches, and
  dependency-aware invalidation for the LSP and repeated builds.
- [ ] Parallelize independent module analysis and expensive candidate emission while
  preserving deterministic output and bounded peak memory.
- [ ] Finish byte-scored SSA destruction for nested/multi-exit control flow and
  parallel-copy cycles.
- [ ] Add profile-controlled environment splitting and allocation sinking, then
  measure monomorphic call sites, GC pressure, and deoptimization in browsers.
- [ ] Add native LICM, strength reduction, guarded unrolling, and vectorization only
  where native profiles justify their compile-time and code-size costs.

### P3: web and native surface

- [ ] Generate a pinned, versioned browser declaration package from Web IDL,
  preserving overloads, inheritance, nullability, and stable external names.
- [ ] Add `DataView`, typed numeric arrays, `Atomics`, workers, and explicit shared
  memory semantics.
- [ ] Define exceptions, promises, async functions, structured cancellation, and
  their JavaScript/native boundary behavior.
- [ ] Keep portable C as the native reference backend until the ABI and semantic
  corpus justify a direct machine-code backend.

## Measure-first queue

These ideas are plausible, not commitments:

- [ ] Cross-function substring dictionaries and suffix/prefix factoring.
- [ ] Constant/function-operand merging that must synthesize a new shared body.
- [ ] Compressor-window-aware ordering across large chunks and lazy boundaries.
- [ ] Multi-result calling conventions and tuple-return dissolution.
- [ ] Route/island-aware progressive-enhancement entry graphs.
- [ ] Profile-trained candidate priors that change search order but never semantic
  eligibility or final exact scoring.

Each starts as an isolated ablation. It is removed or left opt-in when corpus
results are neutral, unstable, or paid back only by unrealistic source shapes.

## Deliberate non-defaults

- Do not introduce `Math.imul`, integer coercions, unrolling, or vectorization
  merely because they look lower-level. Explicit source operations stay exact.
- Do not force string pooling, comma expressions, state machines, function
  merging, or declaration reordering without whole-artifact evidence.
- Do not equate a specialized app slice with a complete npm-library rewrite.
- Do not promise universal superiority over hand-specialized JavaScript or
  Closure ADVANCED; publish paired wins, ties, and losses.
- Do not add optimization annotations when whole-program proof or optional
  profiles can recover the same fact.

## Near milestones

- [x] Scalar/known-function private-function subsumption with identity proofs,
  exact-codec retention, unit bailouts, and a checked ablation.
- [ ] Generalize function merging to parameter permutations and synthesized
  shared bodies, then run a multi-library ablation.
- [ ] Relational SCCP plus path-sensitive escape regions, followed by ordinary
  fold/DCE and representation search.
- [ ] Codec-local symbol assignment and per-chunk declaration layout with bounded
  compile-memory accounting.
- [ ] Typed collection-pipeline fusion on application and library workloads.
- [ ] Incremental analysis, source maps, and optimization remarks so agents and
  editors can act on retained-cost explanations.

## Release gates

1. Full semantic, differential, and backend matrices pass.
2. Every compared artifact passes the same behavior contract before measuring.
3. Every heuristic has a disabled candidate; every unconditional rewrite has a
   semantics-preserving proof.
4. Release benchmarks report raw, gzip-9, Brotli-11, startup, runtime, memory,
   compiler time, and confidence bounds where applicable.
5. Claims distinguish application specialization, partial API coverage, and
   complete compatibility.
6. Checked-in benchmark results, docs, and the website agree.
