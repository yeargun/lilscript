# LilScript Engineering Roadmap

Mission and tradeoff triangle: [knowledge/mission.md](knowledge/mission.md). Full tree: [knowledge/README.md](knowledge/README.md).

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
  collections, array/record spread and destructuring/rest, callback-free
  `for...of`, binary memory, modules, and typed host declarations.
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
- Typed foreign JavaScript/TypeScript ESM edges plus Lilpack: a Lilscript-owned
  build graph and CLI with an integrated Vite engine for JS/TS/assets, production
  bundling, diagnostics, dependency watching, safe reload, and opt-in
  self-accepting hot updates.

### Platform, tooling, and evidence

- Direct zero-wrapper web host access plus JavaScript, portable C, and native
  executable output.
- LSP, VS Code extension, lossless formatter, import organizer, semantic rename
  and references, configurable lint providers, SARIF, Vite playground, and web
  documentation.
- Closure ADVANCED corpus, paired sources, package/library lanes, browser gates,
  native differential execution, and deterministic typed-program fuzzing.
- A generated filter/sort benchmark catalog with project drill-down pages,
  package links, real source previews, explicit mangling lanes, and a published
  progress/limits page.
- Root-owned `labs/solid-client` workspace plus a portable historical LSX
  evidence snapshot; no nested repository or sibling absolute path is required
  to build the site.

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

- [x] **Escape-owned property mangling.** Derive the ESM public aggregate
  field set across modules and mangle proven-owned properties without
  requiring `mangle.exports=true`. Preserve dynamic keys, extern classes, ESM
  public aggregates, reflection, and host ABI names. Size-first enables
  `property-mangling` by default while export names stay stable.
  The checked `property-ledger` boundary shows the available delta: `155/143/107`
  to `105/117/90` raw/gzip/Brotli.

- [x] **Bound private-function subsumption.** Reuse an existing broader
  direct-call implementation when typed scalar or known-function bindings prove
  exact normalized SSA/CFG equality. The isolated ablation improves
  `445/217/179` to `351/201/172` raw/gzip/Brotli.
- [x] **Generalized parameterized function merging.** Extend the proof to
  permuted parameters, then evaluate synthesized shared implementations for
  bodies differing by one operand. Preserve identity and retain every untouched
  artifact candidate. Gated by `parameterized-function-merging`.
- [x] **Repeated-region outlining.** Discover repeated pure/effect-equivalent
  statement regions across functions and outline them when helper calls win a
  bounded IR cost heuristic; codec search keeps untouched IR via
  `ir-compress-pass-variants`. Gated by `region-outlining`.
- [x] **Path- and context-sensitive propagation.** Sparse conditional constant
  propagation over executable blocks with Boolean branch facts. Field-sensitive
  memory SSA and relational facts remain future work. Gated by
  `path-sensitive-propagation`.
- [x] **Partial escape and allocation sinking for JavaScript.** Sink
  LocalOnly allocations into the single Branch arm that uses them. Joint
  named vs positional aggregate emission competes under
  `joint-representation-search`. Closure-environment splitting remains future
  work. Gated by `partial-escape-sinking`.
- [x] **Generalized typed pipeline fusion.** Fuse eligible same-block
  `map`→`map` chains into one callback without intermediate arrays. Keep
  unfused code when compress-pass variants win. Gated by `array-pipeline-fusion`.
- [x] **Joint representation selection.** Search named vs positional public
  aggregate spelling under the emission beam with engine-stable options.
  AoS/SoA and switch/table forms remain future work.
- [ ] **Joint mangling and layout.** Extend declaration clustering to string pools
  and individual chunks; optimize symbol assignment over interference,
  frequency, surrounding tokens, cache-stable public names, and codec cost.
  Use bounded local search, not an unbounded permutation search.
- [x] **Typed expression superoptimization.** Bounded pure Int/Bool rewrites
  (`x^x`, `!!x`, const-assoc add). Full equality-saturation with JS coercion
  rules remains future work. Gated by `expression-superoptimization`.
- [x] **Joint chunk and symbol search.** Score chunk plans against multiple
  function-layout and local-name-reserve emission options under deploy cost.
  Gated by `joint-chunk-symbol-search`.
- [x] **Package effect precision.** Persist typed side-effect/effect-summary
  metadata in library artifacts so consumers can tree-shake packages without
  reanalyzing source or trusting coarse handwritten `sideEffects` flags.
  Summaries live in `lilscript.effects.toml` beside the package root (ABI lock
  unchanged) and are produced from control-flow effect analysis.

### P2: compiler throughput and runtime quality

- [ ] Publish compile time, peak compiler memory, candidates emitted, and
  dominated-candidate counts beside output bytes. The complete robust-predicate
  topology demonstrates that a transfer win can still carry an unacceptable
  search-time cost.
- [ ] Cache equivalent optimizer IR hashes across mangling/output-policy lanes
  and stop codec search when a candidate is structurally dominated under every
  requested transport metric.

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

- [x] Add an exact JavaScript-only `Regex` type with construction, stateful
  `test`, metadata properties, native rejection, and conservative literal
  selection. Literal substitution is use-complete and release-gated by its
  selected Brotli objective plus runtime, retained-memory, and stateful-flag
  checks; raw and gzip are diagnostics for those Brotli builds.

- [ ] Generate a pinned, versioned browser declaration package from Web IDL,
  preserving overloads, inheritance, nullability, and stable external names.
- [ ] Add `DataView`, typed numeric arrays, `Atomics`, workers, and explicit shared
  memory semantics.
- [x] Add JavaScript-native `throw`, structured `try`/`catch`/`finally`, async
  functions, contextual `await`, typed `Task.resolve`/`reject`/`all` and task
  chains, native rejection, exception-safe mutable locals, and a sound unused
  catch-binding codec gate. Structured cancellation remains future work.
- [x] Add generic non-virtual single inheritance with flattened base-first
  layouts, checked subtype upcasts, direct inherited calls, and explicit sound
  constructor chaining. Overrides remain rejected instead of receiving false
  static dispatch or hidden per-instance vtables.
- [x] Add typed synchronous `Generator<T>`, generator functions and methods,
  `yield`, `yield*`, direct generator `for...of`, iterator-closing semantics,
  native rejection, and compressor-scored generator-star spelling.
- [ ] Keep portable C as the native reference backend until the ABI and semantic
  corpus justify a direct machine-code backend.

## Measure-first queue

These ideas are plausible, not commitments:

- [ ] Cross-function substring dictionaries and suffix/prefix factoring.
- [ ] Constant/function-operand merging that must synthesize a new shared body.
- [ ] Extend codec-window-aware function ordering within each emitted file to
  cross-module function placement inside large shared chunks; independently
  compressed lazy files remain hard history boundaries.
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
