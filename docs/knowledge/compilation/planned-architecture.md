# Planned compiler architecture

Status: canonical target plan from the 2026-08-29 checkout. Implemented behavior
is described by [current architecture](current-architecture.md); durable choices
are in [design decisions](../decisions/README.md); execution order is the
[planned migration](../migration/planned-migration.md); live task state is the
[ledger](../migration/board/LEDGER.md).
Visual overview: [`docs/future-direction.html`](../../future-direction.html).

## Product intent

LilScript is a compression-first typed language for the web. It is not a
TypeScript cleanup layer and not a generic JavaScript source-to-source minifier.
Its durable advantage must come from source-level types and contracts that prove
representations legal before JavaScript is chosen. Ports should be able to state
reusable facts such as ownership, non-escape, constructor identity, callable ABI,
and hook-free data without compiler knowledge of a package name or
Terser-shaped source glue. See [mission](../mission.md) and
[compressor surface](../language/compressor-surface.md).

The engineering completion criterion is corpus-scoped: for every declared,
supported, semantically equivalent application or reusable-library boundary in
the maintained corpus, a `size-first` compile for each selected `raw`, `gzip`,
or `brotli` objective must be no larger in that metric than the best eligible
pinned JavaScript toolchain. This is not a theorem over arbitrary JavaScript and
must never be described as one. The comparison contract is
[paired-case contract](../verification/paired-case-contract.md); tool eligibility
is [baseline toolchains](../verification/baseline-toolchains.md).

Correctness, explicit source intent, declared host assumptions, and reusable
library ABI are constraints, not weighted objectives. Codec bytes cannot
legalize a behavior change. Raw, gzip, and Brotli may select different private
representations, but all objective builds for one compilation world must expose
the same declared API and semantics. Product claims measure LilScript's own
output; post-minifying it is diagnostic only.

## Authority and status

Use each authority only for the question it owns:

| Question | Authority |
|---|---|
| Language and public interface contract | [`docs/language-v0.1.md`](../../language-v0.1.md), [`docs/configuration.md`](../../configuration.md), and boundary-specific contracts |
| Product intent and non-goals | [Mission](../mission.md) and [design decisions](../decisions/README.md) |
| Implemented behavior | source and tests, especially [`src/compiler.rs`](../../../src/compiler.rs), [`src/decision_registry.rs`](../../../src/decision_registry.rs), [`src/compilation_contract.rs`](../../../src/compilation_contract.rs), [`src/ir.rs`](../../../src/ir.rs), [`src/lower.rs`](../../../src/lower.rs), [`src/optimizer.rs`](../../../src/optimizer.rs), and [`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs) |
| Current work status | [board ledger](../migration/board/LEDGER.md), not stale phase prose |
| Numerical result | tracked generated, fingerprinted artifacts such as [`comparison/large-libraries/results/seed.json`](../../../comparison/large-libraries/results/seed.json); ignored working summaries are diagnostic until promoted |
| Historical rationale | research pages, journal entries, and board notes; useful context, not current behavior or numerical authority |

No prose size table overrides its generated artifact. A working measurement
without compiler, source, config, artifact, scorer, and harness fingerprints is
a triage input, not publishable evidence. See
[codec measurement](../verification/codec-measurement.md) and
[toolchain provenance](../evidence/toolchain-provenance.md).

## Current implementation boundary

The following is implemented now, not proposed:

- `JavaScriptCompilationContract`, `JavaScriptOptimizationObjective`, and
  `JavaScriptAbiManifest` separate much of the world/ABI/unsafe/effect policy
  from ranking ([`src/compilation_contract.rs`](../../../src/compilation_contract.rs#L9-L94)).
  The compiler constructs and compares the manifest around selection
  ([`src/compiler.rs`](../../../src/compiler.rs#L337-L368)). This is not yet a
  complete final-byte ABI validator.
- Every lowered source instruction has source/generated provenance and an
  optional `NodeId`; a live source `value | 0` has
  `PreserveJavaScriptBitOrZero` ([`src/ir.rs`](../../../src/ir.rs#L16-L29),
  [`src/ir.rs`](../../../src/ir.rs#L423-L477),
  [`src/lower.rs`](../../../src/lower.rs#L2063-L2081)). The emitter rejects a
  malformed obligation and preserves the operation
  ([`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs#L12572-L12599)).
- The code registry classifies all 77 `IrJsOptions` fields and declares 48
  scored emission families. Those counts and the exclusion of ABI, unsafe, and
  illegal axes are enforced by
  `every_ir_js_options_field_is_classified_once` and
  `scored_emission_families_are_named_uniquely_and_skip_illegal_axes`
  ([`src/decision_registry.rs`](../../../src/decision_registry.rs#L1743-L1825)).
- Codec-conditioned packing and identifier-pooling priors are reversible, and
  scalar replacement has a scored `keep-object` IR clone
  ([`src/decision_registry.rs`](../../../src/decision_registry.rs#L556-L701),
  [`src/decision_registry.rs`](../../../src/decision_registry.rs#L1518-L1551),
  [`src/optimizer.rs`](../../../src/optimizer.rs#L14181-L14213)).
- Identity-observed constructors emit named classes, while identity-free
  classes still dissolve; `export constructor C [as PublicC]` supplies the
  public constructor-value proof without changing type-only `export class`
  ([`src/lower.rs`](../../../src/lower.rs#L124-L197),
  [`src/lower.rs`](../../../src/lower.rs#L304-L395),
  [`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs#L8165-L8280)).
- Expression `if`, scalar literal `match`, and ordinary-prototype `object{...}`
  lower into typed IR rather than being reconstructed from target text
  ([`src/lower.rs`](../../../src/lower.rs#L1854-L1880),
  [`src/lower.rs`](../../../src/lower.rs#L2106-L2112),
  [`src/lower.rs`](../../../src/lower.rs#L3604-L3699)).
- Owned field identity carries owner and slot through `FieldGet`/`FieldSet`;
  owner-scoped naming and immutable closure-capture snapshots are scored while
  mutable capture cells remain lexical
  ([`src/ir.rs`](../../../src/ir.rs#L624-L636),
  [`src/decision_registry.rs`](../../../src/decision_registry.rs#L826-L837),
  [`src/decision_registry.rs`](../../../src/decision_registry.rs#L1404-L1417),
  [`src/codegen_ir_js.rs`](../../../src/codegen_ir_js.rs#L13021-L13084)).
- Search retains configured IR/emission seeds, uses exact complete-artifact
  scoring, reserves work for selected late families, and reports starved
  families and `best-observed` stop reasons
  ([`src/compiler.rs`](../../../src/compiler.rs#L1669-L1701),
  [`src/compiler.rs`](../../../src/compiler.rs#L2160-L2316),
  [`src/compiler.rs`](../../../src/compiler.rs#L2875-L3229)).
- The target-side implementation is still string-producing code plus a parsed
  JavaScript optimizer. Canonical and search-off challengers are scored, but
  `repair_late_javascript_candidate` still mutates text and the peephole still
  reparses generated JavaScript
  ([`src/compiler.rs`](../../../src/compiler.rs#L7230-L7492),
  [`src/js_peephole/mod.rs`](../../../src/js_peephole/mod.rs#L1598-L1759)).

There is no complete `ChoiceGraph`, stable whole-program decision vector, or
hygienic target JavaScript AST. Split and preserve-module output also use
narrower, separate selection paths; see [pipeline](pipeline.md) and
[candidate search](candidate-search.md). These are gaps, not grounds to discard
the working compiler.

## Minimal target

The target is five explicit boundaries around the current implementation. It is
not a rewrite and does not require a universal choice graph, proof database, or
solver.

```text
source + normalized config
  -> immutable compilation contract + expected ABI
  -> typed CFG/SSA + reusable proof facts
  -> registered legal candidate recipes with one retained incumbent
  -> bounded deterministic search
  -> hygienic target-JS tree + printer alternatives
  -> pre-print legality + final-byte syntax/identity/ABI/obligation validation
  -> complete Artifact or ArtifactSet score with the pinned selected codec
  -> artifact + compact selection report
```

### 1. Contract boundary

Construct one immutable contract before any profitability transform. Keep three
independent axes explicit: compilation world, artifact format, and public
boundary roots. The contract owns:

- closed-application versus reusable-library world;
- script/module/chunk artifact format and compiler-owned internal linkage;
- the roots observable by unknown consumers;
- target syntax floor and language semantics;
- exported names, callable arity/kind/constructibility, constructor/prototype
  identity, public fields/descriptors/order, ESM behavior, and host names;
- explicit source lowering obligations;
- explicit unsafe assumptions and effect-removal policy.

The optimization objective separately owns selected codec, priority, enabled
families, guards, and work limits. Existing config keys map once into one side
or the other. Optimizer, emitter, target contractions, and bundle planning
consume normalized values; none reinterprets raw config independently.

The current contract and manifest are the seed. Freeze an `ExpectedAbi` from the
linked typed program before optimization. Derive an `ObservedAbi` witness from
every final artifact or module set and compare them. Cover emitted export names,
live binding/module identity, callable kind/default-sensitive arity and
constructibility, constructor/prototype topology, owner-qualified ordered public
fields, promised descriptors/order, foreign imports, and host names only where a
maintained boundary observes them. Do not design a universal reflection schema.

### 2. Proof boundary

Typed IR and conservative analyses answer legality. Reusable facts include
ownership, escape, aliasing, effects, range, allocation/constructor identity,
capture mutation, dynamic property access, and ABI visibility. Missing proof
removes an alternative. It never silently enables `pure_getters`, pristine
builtins, or another unsafe assumption.

Do not begin with a monolithic incremental `ProofDb`. Keep ownership, boundary
escape, aliasing, identity observation, capture mutability, property hooks,
representation exposure, and operation effects as orthogonal conservative facts.
Expose one operation-effects query instead of duplicating observability tables in
optimizer and emitter. Give existing analyses stable entity identities and small
proof/rejection records that candidate generation and `--explain` can consume.
Reanalyze a retained IR variant when a transform changes relevant facts. Add
incremental invalidation only after profiles show full reanalysis is material.

Language work should add proofs reusable across libraries. It must not add
package-name tests, source filename tests, or AST signatures for a port. The
current language inventory and remaining holes live in
[compressor surface](../language/compressor-surface.md).

### 3. Decision boundary

`src/decision_registry.rs` becomes the sole census and recipe owner for
profitability decisions.
Each registered family minimally states:

- stable name, scope, and class (`mandatory`, `ABI`, `explicit-lowering`,
  `unsafe-precondition`, or `scored`);
- proof query and rejection reason;
- coherent configured incumbent;
- all enabled proof-legal alternatives;
- materializer, validator, work class, and measured interactions.

Mandatory correctness is not represented as an optional alternative. A scored
choice without a retained incumbent does not land. `OptimizationOptions` and
`IrJsOptions` remain compact execution plans, but cease to be independent policy
registries.

A typed materializer returns either a candidate with a proof witness or a named
rejection. Avoid a generic proof-query DSL. A small stable candidate recipe is
sufficient initially: IR recipe, emission
options, target/printer choices, context identity, and parent lineage. Add
entity-scoped choices only for a measured case that a global option cannot
express. A general dynamically expanding `ChoiceGraph` is deferred until this
smaller model demonstrably cannot represent required alternatives or
interactions.

### 4. Search and evaluation boundary

Keep the current bounded deterministic portfolio. Its responsibilities are
only scheduling, budget accounting, caching, and incumbent retention. Cheap raw
deltas, token counts, codec predictors, and similarity scores may order work;
they cannot select a final gzip/Brotli winner or establish a bound.

Use one shared `validate_and_score(ArtifactSet)` acceptance primitive for IR
variants, emission plans, and terminal contractions. Candidate producers need
not become one framework, and chunk planning remains separate until a maintained
chunk workload defines its objective. The shared acceptance order is:

1. materialize the recipe;
2. validate typed IR legality, effects, identity, and contract obligations;
3. lower to target JS;
4. print complete bytes;
5. independently parse/resolve final bytes and validate syntax floor, every
   binding or declared external, property classification, module links, observed
   ABI, and live obligation witnesses;
6. score the complete artifact or declared artifact set with the pinned selected
   codec;
7. apply the configured transfer/runtime priority and guards;
8. compare against the retained, independently validated incumbent.

No exact codec call occurs before candidate validation. Final-byte checks do not
prove general semantic equivalence; typed legality plus independent behavioral
and API suites provide that evidence.

The report must serialize the complete selected recipe and name the incumbent,
active alternatives, evaluated and rejected
counts, work by family, unvisited/starved families, budget, stop reason, and
compiler/config/source/codec fingerprints. Normal production search reports
`best-observed`. `bounded-optimal` is allowed only for an explicitly enumerated
finite subdomain. `global-optimal` is forbidden.

Do not score every candidate under all three codecs by default. Each invocation
has one authoritative transfer metric; other metrics are optional diagnostics.
Three product objectives mean three compiles and one ABI/semantic contract.

### 5. Target-JS boundary

Introduce the smallest hygienic emission IR that covers JavaScript the compiler
can produce. It needs resolved binding/external/global references, owned/dynamic/
record/host property categories, function and call kind (including receiver
semantics), allocation-site and capture-cell identity, scope, precedence,
statement/expression kind, ordered effect/throw/suspend barriers, property
definition versus assignment, module edges, target syntax features, exports, and
lowering obligations. It does not model arbitrary user JavaScript or replace the
independent standards-grade final-byte parser.

Initially the new printer must reproduce an existing candidate byte for byte and
emit a witness mapping bindings, properties, ABI elements, and obligations to
target nodes or byte ranges.
Then migrate one parsed-text contraction family at a time. Renaming changes a
binding's spelling, never its identity; property coloring changes a legal owned
slot's spelling, never a public/host key; a fold must explicitly preserve or
reject attached obligations. The independent parser/resolver remains as a
final-byte check rather than as the source of semantic identity.

The parsed peephole remains a scored migration alternative until its useful
families move. Class identity should remain in typed IR emission. Text repair is
deleted family by family after equivalent target operations pass behavior and
objective gates.

## Invariants

1. Every emitted artifact is legal under one immutable contract before it is
   eligible for ranking.
2. Every profitability family retains a known-valid incumbent, and every
   registered alternative admitted by the config is reachable, rejected with a
   reason, or reported unvisited.
3. Mandatory normalization, type safety, liveness correctness, ABI, host
   assumptions, and explicit source intent are never optional search branches.
4. Final size decisions use complete artifact bytes from the pinned scorer.
   Local estimates only schedule proposals.
5. Raw, gzip, and Brotli builds may differ internally but preserve the same
   declared API and semantics.
6. Bounded search reports omitted work. Budget exhaustion and portfolio
   exhaustion never imply global optimality.
7. The selected recipe is replayable. Search retains a best-so-far archive.
   Budget-prefix monotonicity is required only after a path adopts an append-only
   logical schedule; until then the report states that it is not guaranteed.
8. `size-first` never accepts a selected-metric regression against its retained
   incumbent. Other priorities require explicit rank/guard evidence.
9. Compile time, peak memory, startup, and runtime are measured tradeoffs. More
   search, architecture, or configuration is not inherently better.
10. No product claim uses post-minified LilScript, a default-on unsafe property
    assumption, or a library/port-name matcher.
11. Every emitted identifier resolves to an intended binding or declared
    external, and every property access is classified as owned, record,
    public/ABI, extern/host, or dynamic.
12. Effectful, throwing, coercive, and suspending evaluations preserve order and
    cardinality; closure/object/function allocation multiplicity and mutable-cell
    sharing are preserved.
13. Ordinary host `extern` names remain exact. Coordinated foreign renaming, if
    retained, requires a separately declared ABI mapping rather than a spelling
    suffix or general extern-field switch.

## Keep, consolidate, delete

| Action | Components |
|---|---|
| Retain | typed parse/semantic/lowering pipeline; CFG/SSA optimizer; native path from the shared lowered IR; `JavaScriptCompilationContract` and ABI manifest; operation provenance; 77-field classification and 48-family registry; reversible priors; exact pinned scorers; configured incumbent; family reserves/starvation reporting; differential and final-byte validation |
| Consolidate | config normalization into contract versus objective; phase-order/compress IR probes into registry recipes; ordinary and terminal candidate acceptance into one validation/scoring primitive; source/entity explanation into the existing selection report; naming/property/obligation identity into the target tree |
| Delete after replacement gates | production text-mutating peephole families and `repair_late_javascript_candidate`; duplicate acceptance/ranking paths; imperative family definitions outside the registry; policy reads from raw config below normalization; legacy `src/codegen_js.rs` only after no supported API or test depends on it |
| Do not add yet | universal `ChoiceGraph`; persistent whole-program proof database; general SMT/ILP solver; always-on Pareto archive; distributed search; full ECMAScript frontend as target IR; plugin system for decision rows; unified chunk optimizer without a maintained workload; speculative layout/environment variants without a measured corpus need |

Deletion is not a goal by itself. A working path remains until its replacement is
behaviorally equivalent, no worse for the selected size objective, and within
the approved resource budget.

## Completion

This architecture is complete only when:

- every supported production candidate passes typed legality plus final-byte
  syntax, binding, property, ABI, module-link, and obligation validation;
- every declared profitability recipe is registry-owned with a retained
  incumbent and reachable/rejected/reported alternatives;
- no production result depends on reparsing generated text to recover identity;
- single, split, preserve-module, and terminal candidates that make product
  claims use the same validation and scoring authority;
- reports identify explored and unexplored work without global-optimum language;
- the maintained application/library corpus satisfies the metric-specific
  completion criterion against eligible pinned JS toolchains;
- compile-time, memory, and runtime budgets are recorded and accepted rather
  than hidden behind byte totals.

The current green gates and provisional large-library deltas are recorded once in
[`docs/current-status.md`](../../current-status.md), not duplicated here.
