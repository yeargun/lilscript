# Goal compiler architecture

Status: design target, not shipped behavior. Current implementation:
[current architecture](current-architecture.md). Migration sequence:
[phase 07](../migration/07-global-compressor.md). Live status:
[ledger](../migration/board/LEDGER.md).

LilScript's target is a proof-constrained, objective-aware, whole-program
compiler for ordinary JavaScript. It should expose more legal representations
than JavaScript minifiers can infer, search important interactions rather than
hard-wire them, and choose with the exact configured scorer.

The goal is not to claim an impossible minimum over every semantically
equivalent JavaScript program. Program equivalence is undecidable in general;
finite subproblems such as graph coloring, inlining, ordering, pooling, and
chunking are combinatorial; gzip and Brotli are non-additive objectives. The
strongest honest product is:

> For a fixed compiler, source graph, compilation contract, decision-domain
> fingerprint, objective, and deterministic budget, return the minimum-ranked
> validated artifact found, identify every exhaustively solved subdomain, and
> report an optimality gap only when an admissible bound proves one.

That is stronger and more useful than calling a large heuristic beam "global."

## Architectural invariants

1. Correctness, ABI, and explicit source intent constrain the search; scores
   never legalize a transform.
2. Every profitability decision has an incumbent and one or more explicit
   alternatives. The incumbent is always retained.
3. Raw, gzip, and Brotli are separate exact scoring objectives and may produce
   different JavaScript.
4. Heuristics order and prune work only when the pruning proof is sound. They do
   not silently select the final artifact.
5. Every candidate reaching a scorer has valid binding identity, property
   identity, target syntax, ABI, and lowering obligations.
6. A larger deterministic work budget evaluates a superset of the smaller
   budget's canonical schedule and cannot worsen the incumbent.
7. Search exhaustion is reported as unexplored work, not as evidence that the
   incumbent was optimal.
8. Exactness claims name their finite domain and codec implementation.
9. Library mode preserves only the declared external contract; private code is
   optimized as aggressively as closed application code.
10. Target-specific controls are typed operations with defined optimization
    envelopes, not raw text substitution.

## Guarantee vocabulary

These are independent report fields, not one mutually exclusive label. A build
can be semantically validated, exactly scored, best-observed globally, and
bounded-optimal for several small subdomains at the same time. Marketing
language must not invent a stronger claim.

| Label | Meaning |
|---|---|
| `SEMANTICALLY_VALIDATED` | Every finalist passed proof, target, binding, property, ABI, and explicit-obligation validation. |
| `EXACT_SCORE(codec)` | The bytes are measured by the named pinned encoder and parameters for this artifact. It does not mean the encoder produced the theoretically shortest compressed stream. |
| `BEST_OBSERVED(E)` | The winner has minimum configured rank among the fully validated and exactly scored evaluated set `E`. |
| `BOUNDED_OPTIMAL(D)` | Every assignment in a declared finite domain `D` was enumerated; invalid assignments were proved invalid and every valid artifact was exactly scored. The winner is exact for `D`. |
| `PARETO_OBSERVED(E)` | The report retains every nondominated objective vector among evaluated candidates. |
| `EPSILON_CERTIFIED(D, e)` | Admissible bounds prove no unexplored member of `D` improves the configured rank by more than `e`. |
| `GLOBAL_OPTIMAL` | Forbidden for unrestricted program optimization. |

A zero gap is meaningful only for the fingerprinted `ChoiceGraph`. It does not
cover representations the compiler has not implemented.

```text
L(C) = artifacts legal under compilation contract C
D    = finite decision-assignment domain declared by this compiler/configuration
S    = assignments in D whose artifacts were validated and exactly scored
E    = assignments in S whose artifacts also satisfy objective guards
A(d) = emitted artifact for assignment d

published winner = A(arg min rank_O(A(d))), d in E and A(d) in L(C)

if every d in D is proved infeasible or d is in S: BOUNDED_OPTIMAL(D)
otherwise: BEST_OBSERVED(E), unless admissible bounds prove EPSILON_CERTIFIED
```

For multiple objectives, “best tradeoff” means either the explicitly configured
total order or the nondominated frontier. Without one of those definitions,
there is no unique best artifact.

## Overall dataflow

```text
source files
  -> parse with stable NodeId
  -> normalize source/config PreContract
  -> typecheck and resolve SymbolId / TypeId / PropertyId
  -> finalize CompilationContract + AbiManifest + obligations
  -> build typed high-level IR and base ProofDb
  -> instantiate and incrementally extend proof-legal ChoiceGraph
  -> solve exact islands + run deterministic portfolio search
  -> materialize candidate IR recipes lazily
  -> lower to hygienic target JavaScript AST
  -> solve printer/name/order subproblems
  -> validate bindings, ABI, obligations, and target syntax
  -> score complete bytes with pinned raw/gzip/Brotli implementation
  -> update Pareto archive and configured-objective incumbent
  -> emit artifact + manifest + SearchCertificate
```

High-level identity survives until all relevant choices are registered. A
class, aggregate allocation, closure environment, property slot, or explicit
source operation must not be destructively lowered before its alternatives are
known.

## Semantic firewall

The compiler constructs immutable contract and objective values before any
profitability decision.

```rust
struct CompilationContract {
    language_version: LanguageVersion,
    world: CompilationWorld,
    target: EcmaScriptTarget,
    abi: AbiManifest,
    assumptions: UnsafeAssumptions,
    obligations: ObligationTable,
    fingerprint: ContractHash,
}

struct PreContract {
    language_version: LanguageVersion,
    world: CompilationWorld,
    target: EcmaScriptTarget,
    declared_abi_policy: AbiPolicy,
    assumptions: UnsafeAssumptions,
}

enum CompilationWorld {
    ClosedApplication,
    ReusableLibrary,
}

struct OptimizationObjective {
    transfer: TransferMetric,
    delivery: DeliveryObjective,
    diagnostic_metrics: MetricSet,
    domain: DecisionDomain,
    priority: PriorityPolicy,
    enabled_families: FamilySet,
    budget: SearchBudget,
    profile: Option<ProfileHash>,
    fingerprint: ObjectiveHash,
}

enum TransferMetric {
    Raw,
    Gzip { codec: CodecImplementation },
    Brotli { codec: CodecImplementation },
    ExplicitMixedCodec { weights: BundleCodecWeights, codecs: CodecManifest },
}

enum DeliveryObjective {
    SingleArtifact,
    Chunked {
        reachability: ReachabilityWeights,
        request_cost: u64,
        depth_cost: u64,
        cache_policy: CachePolicy,
    },
}

struct CodecImplementation {
    library: String,
    version: Version,
    source_hash: Digest,
    build_hash: Digest,
    wrapper: WrapperPolicy,
    mode: CodecMode,
    quality_or_level: u8,
    window: u32,
    strategy_and_block_settings: CodecSettings,
}
```

The initial canonical manifests preserve today's scorer contracts: UTF-8 byte
length for raw; bundled zlib 1.3.1 level 9 with deterministic gzip wrapper and
`mtime = 0`; bundled Google Brotli 1.1.0, generic mode, quality 11, `lgwin = 22`.
Source/build hashes and every non-default encoder parameter are part of the
manifest. `EXACT_SCORE` means exact output from that implementation, not the
shortest possible DEFLATE or Brotli stream.

`CompilationContract` answers what may be observed. `OptimizationObjective`
answers which legal artifact wins. The second may never mutate the first.

### Compilation worlds

Both worlds compile a closed `.lil` dependency graph. They differ only at the
root boundary.

| Observation | Closed application | Reusable library |
|---|---|---|
| Unused root export | removable | retained |
| Export spelling | internal/mangleable | stable manifest name |
| Private binding/property | mangleable | mangleable |
| Public aggregate field | internal if all consumers linked | stable name or declared opaque-handle ABI |
| Function arity/name/constructibility | preserve only if observed in the closed graph | preserve when manifest promises it |
| Constructor/prototype identity | preserve only if observed | preserve when exported as a constructor value |
| Public own-property order | preserve if observed | preserve declared creation/enumeration order where `Object.keys`, spread, or JSON can observe it |
| Host/foreign names | exact | exact |

Library mode is not optimization-off mode. The root `AbiManifest` is a set of
additional observable facts; everything outside it remains eligible for DCE,
inlining, specialization, mangling, scalar replacement, and layout search.

```rust
fn normalize_pre_contract(source: &SourceGraph, config: &Config) -> PreContract {
    PreContract {
        language_version: source.language_version(),
        world: world_from_target(config.target),
        target: config.javascript.ecmascript,
        declared_abi_policy: normalize_abi_policy(config),
        assumptions: normalize_unsafe_assumptions(config),
    }
}

fn finalize_contract(
    program: &TypedProgram,
    pre: PreContract,
) -> CompilationContract {
    let abi = build_typed_abi_manifest(program, pre.world, pre.declared_abi_policy);
    validate_abi_configuration(pre.world, &abi);

    CompilationContract {
        language_version: pre.language_version,
        world: pre.world,
        target: pre.target,
        abi,
        assumptions: pre.assumptions,
        obligations: collect_source_obligations(program),
        fingerprint: hash_normalized_contract(...),
    }
}
```

Public callable kind is separate from private spelling. A library ABI may
require an ordinary constructible function while private functions still
search `function` versus arrow. A legacy global `function_spelling` setting may
pin both only during config migration.

The boundary contract also covers ESM live bindings, initialization/TDZ and
cycle behavior, namespace export keys/order/descriptors, dynamic-import module
identity, and observable own-property insertion order. Layout and assignment
alternatives that would reorder public keys are illegal unless the ABI declares
the value opaque. `Function.prototype.toString()` is explicitly outside source-shape
compatibility: it may expose valid generated source, never the original body.
Engine-specific stack formatting is likewise not a stable language ABI. If a
consumer requires either textual observation, that value stays behind an
explicit host boundary rather than silently constraining all optimization.

### Explicit lowering intent

Operations need durable provenance:

```rust
struct NodeId(u64);
struct PassId(u32);

enum OperationOrigin {
    Source(NodeId),
    Generated { pass: PassId, parents: SmallVec<NodeId> },
}

enum LoweringObligation {
    Free,
    PreserveOperation(OperationContract),
    PreserveAbi(AbiId),
}
```

The first implemented operation contract is deliberately narrow: a source AST
`value | 0` whose right operand is the integer literal `0`, ignoring transparent
parentheses, requests a live JavaScript `|0`. `0 | value`, `value |= 0`, and
`value | ZERO` remain normal bitwise expressions. A later typed intrinsic may
offer a less syntactic spelling.

The obligation does not keep dead code alive. If the containing live value
survives, however, no optimizer or printer may erase `|0` or substitute `~~`.
Compiler-generated i32 normalization has generated origin and remains removable
after proof. Native executes the portable i32 semantics; a future JS-only
intrinsic without portable meaning is rejected on native.

Every future exact-lowering construct specifies:

- type and runtime semantics;
- exact target operation or permitted spelling set;
- transformations allowed around and through it;
- liveness behavior;
- unsupported-target behavior;
- ABI and optimization interactions.

There is no arbitrary JavaScript string injection and no file-wide "preserve"
mode.

## Stable identity and proof database

Source spans are diagnostics. They are not stable semantic identity. Linking,
specialization, outlining, and cloning use explicit IDs and provenance edges.

```rust
struct ProofDb {
    symbols: Map<SymbolId, SymbolFacts>,
    functions: Map<FunctionId, FunctionFacts>,
    values: Map<ValueId, ValueFacts>,
    allocations: Map<AllocationId, AllocationFacts>,
    closures: Map<ClosureId, ClosureFacts>,
    properties: Map<PropertyId, PropertyFacts>,
    effects: Map<FunctionId, EffectSummary>,
    ranges: Map<ValueId, IntegerRange>,
    aliases: AliasGraph,
    escapes: EscapeGraph,
    identity_uses: IdentityUseGraph,
}
```

Facts are sound and conservative. Missing proof removes an alternative; it does
not become a speculative assumption. Proof objects are immutable and shared by
candidates. Transform-specific derived facts are keyed by structural IR hash.

There is no single analysis snapshot for the entire search. Inlining can make an
allocation local, specialization can tighten ranges, and closure conversion can
change capture and escape facts. Each candidate references an immutable
`AnalysisSnapshot`; a transform invalidates named fact dependencies, recomputes
the affected region, and may instantiate new downstream choice variables.

```rust
struct CandidateState {
    decisions: DecisionVector,
    ir: IrStateHash,
    facts: AnalysisSnapshotHash,
    discovered_choices: ChoiceSetHash,
}

fn apply_recipe(state: CandidateState, recipe: Recipe) -> CandidateState {
    require(recipe.proof_receipt.matches(state.facts));
    let next_ir = materialize_transform(state.ir, recipe);
    let invalidated = dependency_index.invalidated_by(recipe);
    let next_facts = reanalyze_incrementally(next_ir, state.facts, invalidated);
    let choices = instantiate_new_legal_choices(next_ir, next_facts);
    CandidateState::new(next_ir, next_facts, choices)
}
```

A proof receipt is checked against the state it transforms, not merely against
the original program. Full reanalysis remains the conservative fallback.

## Decision registry

Every choice has one owner.

```rust
struct DecisionSpec {
    id: DecisionId,
    class: DecisionClass,
    family: FamilyId,
    scope: DecisionScope,
    legal_alternatives: fn(&AnalysisSnapshot, &CompilationContract, EntityId)
        -> Vec<Alternative>,
    incumbent: fn(&OptimizationObjective, EntityId) -> AlternativeId,
    constraints: Vec<ConstraintId>,
    interactions: Vec<InteractionId>,
    materializer: MaterializerId,
    validator: ValidatorId,
    budget_class: WorkClass,
}

enum DecisionClass {
    Mandatory,
    Abi,
    ExplicitLowering,
    UnsafePrecondition,
    Scored,
    HeuristicDebt,
}

enum DecisionScope {
    Program,
    Module,
    FunctionScc,
    CallSite,
    AllocationRegion,
    ClosureScc,
    PropertyComponent,
    ControlRegion,
    ChunkUnit,
}
```

The registry generates the configured incumbent, all enabled alternatives,
explain metadata, validators, and budget reservations. `IrJsOptions` becomes a
materialized emission plan, not a second policy registry.

## Choice graph

Instantiate registry rows only where proof permits a choice.

```rust
struct ChoiceGraph {
    baseline: DecisionVector,
    initial_variables: Vec<ChoiceVariable>,
    latent_generators: Vec<ChoiceGenerator>,
    hard_constraints: Vec<Constraint>,
    interaction_edges: Vec<HyperEdge>,
    exact_components: Vec<ComponentId>,
    fingerprint: ChoiceGraphHash,
}

struct DecisionVector {
    choices: PersistentMap<ChoiceVariableId, AlternativeId>,
    transform_recipe: Vec<OrderedTransform>,
    hash: DecisionHash,
}
```

`ChoiceGraph` is an immutable universe of registry rules, constraints, finite
domains, and generators. Candidate-local `AnalysisSnapshot` values determine
which variables are currently available. Discovering a new call site or
allocation updates that candidate state, not the global graph, its fingerprint,
or another candidate's budget. Family reservations are allocated from enabled
registry families up front, including families that become reachable only after
another transform.

Constraints encode semantic coupling, not estimated profitability:

- an identity-observed exported constructor requires class-like identity;
- mutable captures that alias require one shared cell;
- a property accessed dynamically cannot use an unrelated renamed spelling;
- an observable function value must remain independently materialized, although
  proven internal direct call sites may still inline its body;
- specialization depends on retained call sites;
- chunk placement constrains imports and name ownership;
- explicit operations survive every candidate containing their live result.

`baseline` is one known-valid whole-program plan produced from configured
behavior. It is not assembled from independently chosen per-row incumbents,
which could violate cross-decision constraints. Per-row incumbents guide
proposal order only.

Interaction hyperedges identify decisions that must be explored together.
Important components include:

```text
aggregate representation x scalar replacement x property naming
closure environment x inlining x specialization x outlining
control-flow spelling x SSA destruction x local coalescing
property assignment x pooling x declaration order
chunk partition x imports x symbol assignment x per-chunk layout
```

## Representation families

Representation is selected per proof-connected allocation or closure region,
not by one global layout flag.

| Entity | Alternatives when legal |
|---|---|
| Local identity-free aggregate | no allocation/SSA fields, scalar tuple, positional array, owned object |
| Typed escaping aggregate | positional array, mangled owned object, stable named object |
| Identity-observed constructor | named ES class, semantically equivalent constructor/prototype form |
| Enum | integer discriminant, specialized branch/table forms |
| Non-escaping closure | inline, lifted function with scalar captures, lexical closure |
| Repeated or escaping closure | lexical closure, positional environment, named environment |
| Return tuple | scalarized multiple values, positional aggregate, retained object |
| Closed record | retained null-prototype record, proof-legal observation projection |

Legality considers allocation identity, constructor identity, prototype and
descriptor observations, typed versus untyped escape, mutation, aliasing,
exception regions, reflection, and target support. "Primitive where possible"
means the primitive alternatives are available; it does not mean they win
without exact objective evidence.

## Property ownership and naming

Property search starts from semantic identities, never spelling conventions.

```rust
struct PropertyFacts {
    id: PropertyId,
    owner: PropertyOwnerId,
    visibility: Private | PublicAbi | HostExtern,
    reflected: bool,
    dynamic_access: DynamicAccess,
    frequency_by_context: ContextHistogram,
}
```

Rules:

- Public ABI, host/extern, reflected, `Record<T>`, and unknown dynamic keys are
  pinned unless a stronger typed contract proves ownership.
- Fields with one semantic identity keep one emitted identity across reads,
  writes, constructors, inheritance, and chunks.
- Public own-property insertion/enumeration order is pinned when the ABI permits
  `Object.keys`, spread, or JSON observation.
- Distinct fields may reuse one spelling only when no reachable object shape or
  dynamic access can distinguish the collision.
- Closure capture slots are compiler-owned properties only when an explicit
  environment object representation is selected.
- Source suffixes such as trailing `_` carry no semantic authority.

The resulting optimization contains graph coloring and compressed-context name
assignment:

```text
property facts
  -> pinned names + renameable PropertyIds
  -> collision/interference graph
  -> exact-color small connected components
  -> deterministic heuristic color large components
  -> assign identifier strings to colors
  -> jointly perturb color reuse, spelling, declaration order, and pools
  -> emit complete artifact
  -> exact configured-codec score
```

For raw size with fixed contexts, small color/name assignment components can use
branch-and-bound or min-cost matching. For gzip/Brotli, token frequency is only
a proposal heuristic because global dictionary context makes costs non-additive.
Small components are exhaustively emitted and codec-scored; large components use
deterministic swaps, recolors, and large-neighborhood re-solves.

Every exact claim uses a finite declared name domain: pinned names plus a
deterministically generated set of legal identifiers with a stated alphabet and
maximum length. Longer dictionary-friendly names can be heuristic challengers,
but an unbounded identifier language cannot be called exhaustively searched.
Compressed-context components are exact only with all surrounding target bytes
fixed and every assignment in that conditional domain emitted and scored. Local
codec winners from separate components cannot be composed as if their costs were
additive.

```rust
fn property_family(
    component: PropertyComponent,
    state: &CandidateState,
    domain: &DecisionDomain,
) -> Vec<Plan> {
    if component.domain_size() <= domain.exact_property_cardinality {
        return enumerate_legal_colorings_and_names(component)
            .map(|choice| state.with(choice))
            .collect();
    }

    let seed = deterministic_dsatur(component);
    stable_union(
        [seed, frequency_seed(component), reuse_seed(component)],
        bounded_recolors(seed),
        bounded_name_swaps(seed),
        solve_selected_neighborhoods(seed),
    )
}
```

## Closures and call graph

Inlining is not a local yes/no pass. It interacts with closure environments,
specialization, outlining, function merging, naming, and compression repetition.

Legality excludes transformations that violate:

- callable identity or address-taking;
- public `name`, `length`, prototype, or constructibility;
- recursion and dynamic dispatch constraints;
- mutable shared capture-cell semantics;
- argument/default evaluation order;
- exceptions, `async`, generators, and `this` binding;
- explicit operation obligations.

For each call-graph SCC, generate a small family of coherent strategies:

```text
keep shared implementation
inline proven call sites
clone by constants/capture signature
lift closure and pass scalar captures
retain lexical closure
materialize positional or named environment
merge equivalent/permuted implementations
outline repeated regions
```

Instruction counts, call frequency, and compressed-body similarity order these
strategies. Only complete-artifact rank chooses among them.

```rust
fn call_graph_alternatives(scc: FunctionScc, facts: &AnalysisSnapshot) -> Vec<Recipe> {
    let mut out = vec![Recipe::incumbent(scc)];
    out.extend(legal_inline_recipes(scc, facts));
    out.extend(legal_specializations(scc, facts));
    out.extend(legal_closure_layouts(scc, facts));
    out.extend(legal_merge_and_outline_recipes(scc, facts));
    deduplicate_by_structural_recipe(out)
}
```

## Hygienic target JavaScript AST

The production emitter returns a target AST, not a string to be reparsed.

```rust
struct JsProgram {
    scopes: ScopeTree,
    bindings: Map<BindingId, JsBinding>,
    properties: Map<PropertyId, JsProperty>,
    nodes: Arena<JsNode>,
    obligations: Map<JsNodeId, OperationContract>,
}
```

All identifier references point to `BindingId`; all owned property references
point to `PropertyId`. Renaming changes spelling, not identity. Target folds
state which IDs and obligations they preserve. String rendering is the final
step. The printer is verified by construction and also emits a resolution
witness. An independent final-byte parser/resolver checks that witness, ABI,
and operation obligations before scoring. Syntax validity alone is insufficient:
valid text can still bind an identifier to the wrong nearer declaration.

```rust
struct PrintWitness {
    binding_tokens: Vec<(ByteRange, BindingId)>,
    property_tokens: Vec<(ByteRange, PropertyId)>,
    obligation_tokens: Vec<(ByteRange, ObligationId)>,
    export_tokens: Vec<(ByteRange, AbiId)>,
}
```

The independent validator resolves final bytes, compares every witnessed use to
its intended identity, checks pinned property spellings and ESM behavior, and
confirms every live obligation. Only then may codec scoring run.

Printer choices remain searchable:

- parentheses and precedence-safe grouping;
- ASI and semicolons;
- quotes and numeric literal forms;
- declaration and function spelling where ABI permits;
- conditional/comma/control-flow forms already represented in the target AST.

During migration, parsed-JS folds are scored alternatives. A fold moves to the
target AST with binding-aware tests, then its text implementation is deleted.
No search-off path applies an unranked second optimizer.

## Candidate evaluation

There is one evaluation authority for IR, emission, terminal, and chunk
challengers.

```rust
fn evaluate_complete(
    decisions: &DecisionVector,
    contract: &CompilationContract,
    objective: &OptimizationObjective,
    proofs: &ProofDb,
    cache: &mut ArtifactCache,
) -> Result<EvaluationBatch, EvaluationFailure> {
    let state = cache.materialize_candidate(decisions, proofs)?;
    validate_ir(&state.ir, contract, &state.facts)?;

    // One artifact set covers single-file and chunked output.
    let target = cache.lower_to_js_artifact_set(&state, decisions.delivery_recipe())?;
    for js in target.programs() {
        validate_binding_ids(js)?;
        validate_property_ids(js)?;
        validate_abi_ast(js, &contract.abi)?;
        validate_lowering_obligations_ast(js, &contract.obligations)?;
        validate_target_floor(js, contract.target)?;
    }

    let mut evaluated = EvaluationBatch::new();

    for printed in exact_printer_family(&target, decisions, objective.budget) {
        // Reparse final bytes and compare resolved identities with the printer's
        // witness. This catches valid-but-wrong binding and property output.
        validate_printed_artifact(&printed, contract)?;

        let scores = exact_required_metric_scores(
            &printed,
            objective.required_metrics(),
            objective.codec_manifest(),
        );
        let delivery = exact_delivery_score(
            &printed,
            &scores,
            objective.transfer,
            objective.delivery,
        );
        let performance = performance_model(&state.ir, &printed, objective.profile)?;
        let startup = startup_model(&printed);

        let candidate = EvaluatedArtifact::new(
            printed, scores, delivery, performance, startup
        );
        match objective.evaluate_guards(&candidate) {
            GuardDecision::Accept => evaluated.admissible.push(candidate),
            GuardDecision::Reject(report) => {
                evaluated.guard_rejections.push((candidate.summary(), report));
            }
        }
    }

    evaluated.finish(objective)
}

enum EvaluationFailure {
    ProvenInfeasible(ProofReceipt),
    CompilerFailure(CompilerDiagnostic),
}
```

`apply_selected_canonical_peephole`, search-off cleanup, and repair paths do not
receive special comparison rules. If they survive during migration, they call
this evaluator and compete with the untouched incumbent.

`EvaluationBatch` returns every validated, exactly scored, guard-admissible
printer alternative, not only the configured-rank winner. Guard-rejected scored
artifacts belong to `S` but not `E` and are retained as aggregate report records,
not Pareto candidates. The Pareto archive keeps compact metadata for every
nondominated point in `E` and stores artifact bytes in the content-addressed
cache. A ticket reserves summary and artifact capacity before evaluation; if it
cannot, search stops before scoring that candidate. Therefore
`PARETO_OBSERVED(E)` never silently drops a measured nondominated point.

`ProvenInfeasible` means a registry constraint rejected an assignment with a
proof receipt. `GuardRejected` means the declared objective excludes an
otherwise legal artifact. A materialization, printer, binding, ABI, or obligation
failure for an assignment declared legal is `CompilerFailure`: compilation or
certification stops. It is never counted as a proved-invalid domain member.

## Objective model and Pareto archive

The configured metric is always exact. Other metrics are measured only when the
objective or an explicit diagnostic/Pareto budget requests them. The archive is
therefore labeled with its measured dimensions rather than pretending every
candidate paid for every codec:

```rust
struct ObjectiveVector {
    raw_bytes: u64,
    gzip_bytes: Option<u64>,
    brotli_bytes: Option<u64>,
    delivery_cost: Option<u128>,
    performance_proxy: u64,
    startup_proxy: u64,
    retained_memory_proxy: Option<u64>,
}

struct ParetoArchive {
    measured_dimensions: ObjectiveSet,
    candidates: Vec<EvaluatedArtifact>,
}
```

The configured policy is explicit:

```rust
fn rank(
    v: ObjectiveVector,
    baseline: ObjectiveVector,
    objective: OptimizationObjective,
) -> Rank {
    // OptimizationObjective.transfer is the sole selected transfer metric.
    // DeliveryObjective only composes per-file costs and request/cache terms.
    let transfer = objective.selected_artifact_or_delivery_cost(v);
    match objective.priority {
        SizeFirst => lex(transfer, v.performance_proxy),
        Balanced(weights, guards) =>
            guarded_weighted_rank(v, baseline, transfer, weights, guards),
        RealisticPerformanceFirst(limit) =>
            regression_bucket_then_transfer(v, baseline, transfer, limit),
        PerformanceFirst =>
            lex(v.performance_proxy, transfer),
    }
}
```

Phase 07 prioritizes `SizeFirst`. Other priorities reuse the same legal set and
validators; they do not weaken semantics or ABI. The compiler maintains the
nondominated frontier over evaluated candidates for diagnostics and future
interactions. Objective-stratified sampling is not called a Pareto frontier
unless dominance is actually checked.

## Why the complete search is hard

| Decision | Embedded hard problem |
|---|---|
| Binding/property coalescing | graph coloring |
| Compressed-context name assignment | quadratic assignment |
| Declaration/function ordering | Hamiltonian path/TSP-like ordering |
| Inlining and specialization | knapsack and facility-location variants |
| Overlapping outlining candidates | set packing |
| String/literal pool selection | set cover/facility location |
| Chunk assignment | constrained graph partitioning |
| Joint representation choices | weighted constraint satisfaction over interacting SCCs |

Gzip and Brotli make even distant decisions interact through history and context.
Raw deltas, entropy, n-grams, similarity, and learned predictions can prioritize
work. None can certify the final compressed winner.

## Exact islands

Use exact algorithms where the finite domain and context are explicit.

| Subproblem | Exact method and boundary |
|---|---|
| Legality, ABI, obligations | conservative proof/validation; not profitability |
| Small spelling family | enumerate every permitted spelling and exact-score |
| Raw printer decisions | min-plus dynamic program over precedence/token/ASI states |
| Small property interference component | DSATUR branch-and-bound coloring |
| Fixed-color raw name assignment | min-cost matching when costs are additive |
| Small topologically constrained declaration set | subset dynamic programming |
| Small gzip/Brotli order set | enumerate legal permutations and exact-score complete artifacts |
| Small representation/closure SCC | enumerate the constrained Cartesian domain |
| Fixed-symbol small chunk partition | subset/partition branch-and-bound |
| Single-interval, non-overlapping raw outlining choices with additive precomputed helper cost | weighted interval scheduling; general multi-occurrence outlining remains set packing |

Current Held-Karp function layout is exact for its similarity surrogate, not for
gzip/Brotli. The goal report names that distinction. Exact-domain cutoffs and
finite candidate alphabets are declared in the objective fingerprint before
search; changing them creates a different domain, while increasing only the
logical work budget extends the same schedule. A compressed exact island is conditional on fixed
surrounding bytes: every assignment in the island is emitted in that context and
scored as a complete artifact. Winners of separate gzip/Brotli islands are not
composed as though codec cost were additive. An exact island contributes
whole-artifact candidates and a context-qualified certificate; it does not
permanently commit its local winner for later contexts.

## Deterministic portfolio search

The general solver combines exact islands with bounded global exploration.

```rust
fn optimize_whole_program(
    source: SourceGraph,
    config: ProjectConfig,
) -> Result<SearchResult, CompileError> {
    let pre_contract = normalize_pre_contract(&source, &config)?;
    let typed = typecheck(source, &pre_contract)?;
    let contract = finalize_contract(&typed, pre_contract)?;
    let objective = normalize_objective(&config)?;
    let typed_ir = lower_to_high_level_ir(typed, &contract)?;
    let base_facts = analyze(&typed_ir, &contract)?;
    let graph = DECISION_REGISTRY.instantiate_universe(
        &typed_ir, &base_facts, &contract, &objective
    );

    // This is one coherent known-valid plan, not the product of independent
    // per-row defaults.
    let incumbent_plan = graph.configured_baseline();
    let incumbent_batch = evaluate_complete(
        &incumbent_plan, &contract, &objective, &base_facts, &mut cache
    )?;
    let incumbent = incumbent_batch.best_by(objective);

    let mut archive = ParetoArchive::new(objective.required_metrics());
    archive.insert_batch(incumbent_batch);
    let mut best = incumbent;
    let mut queue = canonical_initial_schedule(&graph, &best);
    let mut ledger = WorkLedger::new(objective.budget, graph.active_families());
    let mut certificate = SearchCertificateBuilder::new(&graph, &contract, &objective);

    ledger.reserve_one_challenger_per_family();

    for island in graph.small_exact_components() {
        queue.insert_exact_enumeration(island);
    }

    while let Some(batch) = ledger.reserve_next_prefix_batch(&queue) {
        let regions = queue.remove_batch(&batch);

        let results = parallel_map_indexed(regions, |ticket, region| {
            if let Some(bound) = admissible_rank_bound(&region, &graph, &objective) {
                if bound.cannot_beat(&best.rank) {
                    return SearchStep::BoundPruned(region.id, bound);
                }
            }

            evaluate_or_refine(
                region,
                &contract,
                &objective,
                &base_facts,
                cache.shard(ticket),
                ticket.reserved_cost,
            )
        });

        // Parallel completion order never changes search history.
        for result in results.in_ticket_order() {
            ledger.commit_reserved_ticket(result.ticket());
            match result {
                ProvenInfeasible(proof) => certificate.record_infeasible(proof),
                CompilerFailure(error) => return Err(error),
                BoundPruned(id, bound) => certificate.record_bound_prune(id, bound),
                Partial(next) if next.bound_may_beat(&best.rank) => {
                    queue.insert(next);
                }
                Complete(batch) => {
                    certificate.record_guard_rejections(&batch.guard_rejections);
                    for artifact in batch.admissible() {
                        archive.insert_if_nondominated(artifact.clone());
                        if objective.precedes(&artifact, &best) {
                            best = artifact;
                            certificate.checkpoint(&best, &ledger);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let stop = classify_stop_reason(&graph, &queue, &ledger);
    certificate.finish(stop, &best, &archive, &queue, &ledger);
    Ok(SearchResult { best, archive, certificate })
}
```

The initial portfolio contains:

1. the mandatory configured incumbent;
2. at least one challenger from every enabled family;
3. exact solutions for small connected components;
4. declared joint families for measured non-monotone interactions;
5. pairwise covering samples across remaining interaction edges;
6. deterministic best-first branch-and-bound over elite domains;
7. deterministic large-neighborhood moves that re-solve whole interaction
   components;
8. target printer, naming, pooling, and order neighborhoods.

This is not a full cross-product. It is an anytime portfolio whose omissions are
visible. Emptying the sampled portfolio is not exhausting a declared domain.

## Bounds and stopping

The best validated artifact is an exact upper bound from the start because the
configured incumbent is mandatory. Pruning requires an admissible lower bound.
If a transform may reveal new legal choices, the region's bound covers that
transitive discovery closure or is `None`; a bound over only currently visible
variables is not safe.

Useful raw bounds may combine a mandatory token skeleton, exact minimum costs of
independent unresolved fragments, token-boundary DP, and relaxed name costs.
For gzip/Brotli, a raw bound is not a compressed bound. Until the pinned encoder
has a verified clonable-state model, the general compressed lower bound is weak
and the global gap is usually unknown.

Never call these admissible bounds:

- entropy or token-frequency estimates;
- n-gram or declaration-similarity scores;
- a learned codec predictor;
- local compressed deltas;
- scores from a different codec.

They are proposal-order signals only.

```rust
enum StopReason {
    ExactDomainExhausted,
    PortfolioExhausted,
    EpsilonCertified,
    WorkBudgetExhausted,
    CodecBudgetExhausted,
    MemoryBudgetExhausted,
    DeterministicEpochDeadline,
    Stagnation,
    Cancelled,
}
```

`ExactDomainExhausted` is available only when queued regions formed a proven
partition of a declared finite domain, every dynamically discoverable choice was
closed under that domain definition, and every assignment was classified; it
proves `BOUNDED_OPTIMAL(D)`. Exhausting a sampled portfolio reports
`PortfolioExhausted` and only `BEST_OBSERVED(E)`. `EpsilonCertified` requires an
admissible bound for every open region. Budget, deadline, and stagnation stops
also report `BEST_OBSERVED(E)` with unknown global gap. Cancellation returns the
last fully validated checkpoint.

For a scalar size objective:

```text
gap_bytes = best_validated_transfer - minimum_admissible_open_bound
```

For mixed priorities, report the rank interval and separate transfer/performance
intervals. Do not manufacture a byte gap from a weighted rank.

## Work budgets and determinism

```rust
struct SearchBudget {
    ir_transforms: u64,
    emitted_input_bytes: u64,
    validations: u64,
    raw_probes: u64,
    gzip_input_bytes: u64,
    brotli_input_bytes: u64,
    retained_artifact_bytes: u64,
    peak_memory_bytes: u64,
}

enum WorkClass {
    Representation,
    CallGraph,
    ControlFlow,
    Naming,
    PoolingAndLayout,
    Chunks,
    TerminalContraction,
}
```

Rules:

- baseline construction and validation are mandatory;
- every enabled family receives a minimum reserve before shared work;
- unused reserves roll forward in stable family order;
- each ticket reserves a conservative upper bound in every affected budget
  dimension before parallel execution; a worker aborts before crossing an
  unreserved streaming/memory limit;
- workers receive immutable numbered tickets and results commit in ticket order;
- cache hits charge the same logical work as misses;
- if `B2` is at least `B1` in every logical budget dimension, `B2` consumes a
  superset of `B1`'s canonical ticket prefix and cannot worsen its incumbent,
  provided both builds have the same contract, objective domain, cutoffs, and
  family reservations;
- wall-clock deadlines are an explicitly nondeterministic convenience mode and
  cannot produce a reproducible-search or monotone-budget certificate;
- artifact-size scaling may reduce shared work but never the family minimum;
- reports name unvisited and starved families.

Release and comparison builds use logical limits, not wall time. These rules make
their search reproducible and quality-monotone under componentwise larger
budgets. Peak memory is enforced through retained-byte limits plus per-ticket
worker reservations, not measured only after allocation.

## Caching and structural sharing

Candidate state is persistent and sparse. Do not clone and optimize the entire
module for every boolean.

| Cache | Key |
|---|---|
| Proof facts | contract hash + typed high-level IR hash |
| Transform result | input state hash + ordered transform recipe |
| Optimized IR | source graph + contract + optimizer recipe |
| Target AST | optimized IR hash + contract/target hash + structural decision projection |
| Naming solution | complete target-context hash + binding/property topology + finite name domain + pinned names + objective |
| Printed bytes | target AST hash + printer decision projection |
| Codec score | bytes SHA-256 + exact encoder/version/parameters |
| Performance score | IR hash + printed artifact/printer-decision hash + profile + weights |
| Chunk score | chunk-byte hashes + delivery objective fingerprint |

```text
PlanId = H(
  compiler version,
  contract hash,
  choice-graph hash,
  canonical decision vector,
  ordered non-commuting transform recipe
)
```

Never canonicalize transforms whose order can change output. Inlining before
outlining and outlining before inlining are distinct recipes.

## Bundle and chunk optimization

Chunking joins the same decision system and the same `evaluate_complete`
authority. A candidate materializes an `ArtifactSet` containing one or more JS
programs plus its manifest; validation and ranking consume that set rather than
calling a separate bundle-only selector.

```text
mandatory lazy boundaries
  + movable function/data ownership
  + import/export constraints
  + shared property/name decisions
  + per-chunk selected-codec bytes
  + request/depth/cache costs
  + aggregate performance/startup policy
```

For a normal objective, define:

```text
transfer(plan, metric) =
  sum(load_weight(chunk) * encoded_size(chunk, metric))
  + request_cost(plan)
  + depth_cost(plan)
  - cache_credit(plan)
```

An explicitly configured mixed-codec deployment objective normalizes to the
single authoritative `TransferMetric::ExplicitMixedCodec` variant. It is named
and reported as mixed; `DeliveryObjective` contributes only topology costs and
never carries a second codec selector that could silently override
`javascript.cost_model`.

For a fixed partition, re-solve affected symbols, pools, property names, and
declaration layout. Large-neighborhood moves include moving one SCC, merging two
chunks, splitting one candidate shared SCC, and swapping ownership. Unaffected
chunks are content-addressed cache hits. `preserve-modules` fixes partition
identity but still admits legal per-module and cross-module symbol decisions.

## Search certificate

Every production/release result emits machine-readable evidence:

```rust
struct SearchCertificate {
    validation: ValidationCertificate,
    scoring: ScoreCertificate,
    search_guarantee: SearchGuarantee,
    pareto_guarantee: Option<ParetoGuarantee>,
    stop_reason: StopReason,
    compiler_commit: CommitId,
    compiler_binary_sha256: Digest,
    source_graph_hash: Digest,
    contract_hash: Digest,
    abi_manifest_hash: Digest,
    objective_hash: Digest,
    registry_version: RegistryVersion,
    choice_graph_hash: Digest,
    schedule_hash: Digest,
    codec_implementations: CodecManifest,

    effective_budget: SearchBudget,
    work_by_family: Map<FamilyId, WorkSummary>,
    generated: u64,
    validated: u64,
    proven_infeasible: u64,
    guard_rejected: u64,
    compiler_failures: u64, // must be zero for a successful certificate
    exact_scored_by_metric: Map<TransferMetric, u64>,
    deduplicated: u64,
    retained: u64,
    unvisited_families: Vec<FamilyId>,
    starved_families: Vec<FamilyId>,

    baseline_hash: Digest,
    winner_hash: Digest,
    winner_vector: ObjectiveVector,
    pareto_frontier: Vec<ArtifactSummary>,
    exact_subdomains: Vec<DomainCertificate>,
    lower_bound: Option<RankBound>,
    certified_gap: Option<RankGap>,
    unknown_gap_reason: Option<String>,
}
```

Nondeterministic telemetry such as elapsed time is stored outside the hashed
certificate. The report distinguishes generated, validated, scored, retained,
and finally ranked candidates. It also records candidate lineage and every
failed proof. Validation, exact scoring, search completeness, and Pareto scope
are separate facts; no single label hides a weaker dimension.

## Source feedback loop

The language source can be the compression blocker. The compiler must make that
visible instead of growing a target peephole.

`--explain` attributes retained cost and missing alternatives to source entities:

```text
allocation NodeId 412:
  legal: positional-array, named-object
  unavailable: scalarize (escapes through JsValue at NodeId 455)
  selected: positional-array
  objective delta vs incumbent: -17 Brotli bytes

property PropertyId 19:
  stable: exported field in AbiManifest

call FunctionId 8 -> FunctionId 14:
  inline rejected: this use takes the observable function value

operation NodeId 92:
  fixed: explicit js-i32-normalization obligation
```

When a library loses, classify the cause before compiler work:

1. compiler correctness bug;
2. legal alternative missing from the choice graph;
3. language cannot express the needed proof;
4. source is still a `JsValue`/host-shaped transliteration;
5. genuinely dynamic public behavior;
6. search budget left a known family unexplored.

Only cases 1, 2, and 6 are optimizer work. Case 3 is a language RFC. Case 4 is a
source redesign. Case 5 is measured honestly.

## Migration components

The target is architecturally new but lands incrementally around the current
compiler:

| Component | Responsibility |
|---|---|
| `compilation_contract` | world, ABI, assumptions, source obligations |
| `optimization_objective` | transfer metric, priority, guards, enabled families, budgets |
| `decision_registry` | one declaration of every mandatory/ABI/scored choice |
| `choice_graph` | entity-scoped variables, hard constraints, interaction components |
| `decision_vector` | sparse immutable choices and stable plan identity |
| `search::ledger` | deterministic family reserves and logical work accounting |
| `search::frontier` | configured incumbent, Pareto archive, bounds, certificates |
| `search::exact` | exact small-domain coloring, assignment, ordering, and enumeration |
| `artifact_cache` | structural and codec caches |
| `target_js` | hygienic AST, target contractions, printer families |
| `legacy_plan` | temporary projection into current `OptimizationOptions` and `IrJsOptions` |
| `search_report` | lineage, proof failures, starvation, bounds, and guarantee label |

Migration order and acceptance gates remain authoritative in
[phase 07](../migration/07-global-compressor.md). The key compatibility technique
is shadow mode: first reproduce current output from the new model, then migrate
one decision family at a time, and delete each legacy decision site only after
semantic, objective, resource, and library gates pass.

## Non-goals

- No claim of a global minimum over all equivalent JavaScript.
- No post-minification by Terser/Oxc/Closure.
- No library-name or source-shape special cases.
- No raw-JavaScript string macros as an optimization API.
- No codec proxy accepted as the final score.
- No public ABI change caused by raw/gzip/Brotli or priority.
- No unsafe getter/prototype/builtin assumptions inferred from size.
- No exhaustive global Cartesian product marketed as practical production
  compilation.
- No performance claim based only on the static model; browser evidence remains
  required.

## Completion criteria

The goal architecture is in production only when:

1. every production candidate is semantically and identity validated;
2. every decision comes from the registry or is explicitly mandatory/ABI;
3. every compactness prior has a reachable opposite where both are legal;
4. source obligations and library ABI pass across all objectives;
5. aggregate, class, closure, property, call-graph, control-flow, naming,
   pooling, layout, and chunk families all participate without silent starvation;
6. terminal transforms use the same evaluator as every other candidate;
7. the current text reparser and duplicate policy paths are no longer required;
8. reports separate semantic validation, exact-score dimensions, Pareto scope,
   and `BOUNDED_OPTIMAL`, `EPSILON_CERTIFIED`, or `BEST_OBSERVED` search status,
   with no unqualified global-optimum language;
9. size-first raw/gzip/Brotli corpora and designated library wins do not regress;
10. production compile-time and memory budgets remain enforced and published.
