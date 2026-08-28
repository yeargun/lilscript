# Phase 07 — architecture for a global compressor

Parent: [migration](README.md). Current state:
[current architecture](../compilation/current-architecture.md). Target:
[goal architecture](../compilation/goal-architecture.md),
[objectives](../compilation/objectives.md),
[decision registry](../compilation/decision-registry.md).
Language leverage: [compressor surface](../language/compressor-surface.md).
Board: 07.1 is `ident-05`; 07.2–07.7 are `arch-02`–`arch-07`. Mission: [mission](../mission.md).

Phases 00–06 are the standing evidence loop that the **existing** compiler does
not lose to JS minifiers. This phase changes the **decision system** so later wins come
from typed proof + complete-artifact search, not from another peephole shape or
port TOML. No compiler code is specified here as a patch; the order is the
product.

The north star is unchanged: smallest **correct** artifact under the configured
`raw` / gzip-9 / Brotli-11 objective, with compile time and runtime as explicit
tradeoffs, Closure ADVANCED’s closed-world discipline and beyond it — because
LilScript types exist before JavaScript is spelled.

“OptimizedJS” in this plan means ordinary valid ECMAScript selected from legal
lowerings. It is not a second language and it does not weaken LilScript
semantics. A sidecar decision/proof report may explain the artifact, but no
runtime metadata is required.

This phase is for **libraries under `priority = size-first`**. A cleaner
`compiler.rs` that still emits `defineProperty` tables, starves 18 KiB modules,
or hides size-first families behind the root TOML subset has not landed.

Authority for every claim below:
[objectives](../compilation/objectives.md),
[current architecture](../compilation/current-architecture.md),
[goal architecture](../compilation/goal-architecture.md),
[decision registry](../compilation/decision-registry.md),
[compressor surface](../language/compressor-surface.md).

## Why this phase exists

The knowledge tree already says “do not decide contested spellings with local
heuristics when a complete-artifact score is available.” The implementation
still does, in named ways:

- struct/class → SSA / array / object / `class` is mostly proof + config, not a
  searched family ([registry](../compilation/decision-registry.md#aggregates-class-struct-object-record));
  keep-object versus scalar-replace is searched for size-first libraries (07.3);
- `src/compiler.rs` still owns phase-order and compress-pass IR probes;
- parsed peephole is a second optimizer, including class identity;
- several ports cannot state proofs Terser guesses, so they grow `assume_*` and
  `JS.method*` tables ([compressor surface](../language/compressor-surface.md)).

Improving compression while that list stands produces more glue. This phase
removes the list.

## Invariant

Every new or moved tactic must state all four:

1. **Proof** — which type/escape/effect/identity fact makes it legal.
2. **Incumbent** — configured or codec-prior spelling, always retained.
3. **Family** — which complete artifacts compete, and which budget slice they
   consume.
4. **Refusal** — what the tactic must not become (library special case,
   post-minify, unscored search-off rewrite, entropy proxy as winner).

A tactic that cannot fill (3) is either mandatory correctness or it does not
land as a size optimization. A tactic that fills (3) only for one port is glue.

## Semantic firewall

The optimizer does not own every choice. Before candidate generation, one
normalized compilation contract freezes four authorities:

| Authority | Examples | May raw/gzip/Brotli search change it? |
|---|---|---|
| Language semantics | evaluation order, i32 wrap, exceptions, identity | No |
| Public/host ABI | exports, host names, field keys, arity, constructibility, descriptors | No |
| Explicit lowering obligations | a source-requested live JS `\|0`, a future typed target intrinsic | No |
| Unsafe assumptions | pristine builtins, no getters/proxies | No; explicit, fingerprinted preconditions only |

Only after that contract is fixed may the objective choose layout, inlining,
names, pooling, declaration order, or another legal spelling. In particular,
`cost_model` and `priority` may change the artifact but may not change its API or
erase an explicit lowering obligation. Configuration that changes observable
behavior belongs to the contract layer, not to the optimizer layer:

- `public_aggregate_abi`, export-name policy, function constructibility, and
  host/property names are ABI;
- `assume_pristine_builtins` and `assume_pure_property_reads` are unsafe
  preconditions until typed proofs replace them;
- `strip_console` is an explicit effect-removal build policy, not an ordinary
  size optimization;
- `cost_model`, `priority`, search effort, and representation priors are
  optimization policy.

The current implementation cannot fully enforce this firewall. A source
`x | 0` initially lowers to `IrBinaryOp::BitOr`, while normalization required by
other `int` operations is introduced during JS emission. Neither carries an
end-to-end obligation into terminal rewrites, where both are only generated JS
text. The migration therefore adds durable node identity and operation origin
before widening search:

```text
NodeId                         # stable identity, not a synthetic Span
OperationOrigin = Source(NodeId) | Generated(PassId, parent NodeIds)
LoweringObligation = Free | PreserveOperation | PreserveAbi(AbiId)
```

Transforms must carry or discharge obligations explicitly. An optimization
remark records every discharge. Spans remain diagnostics, not semantic identity.

### The `|0` rule

The first implementation is deliberately narrow: the source AST pattern
`value | 0`, where the right operand is the literal integer token `0` after
transparent parentheses:

- source-written live `x | 0` lowers as an explicit i32-normalization
  obligation and must still contain the JS `|0` operation under raw, gzip, and
  Brotli and under every priority;
- `0 | x`, `x |= 0`, and `x | ZERO` remain ordinary bitwise language operations,
  not exact-spelling requests; a future typed intrinsic is the unambiguous form
  when broader control is needed;
- a normalization introduced by lowering ordinary `int` arithmetic is marked
  generated and may be removed when range/type proof makes it redundant, or
  retained by a performance policy;
- unreachable code and an entirely unused pure computation may still disappear;
  preserving an operation does not make the surrounding program live;
- the emitter may rename or inline around the operation, but may not replace a
  live explicit `|0` with an identity, `~~`, or another spelling.

The IR now carries this first obligation and affected candidates conservatively
skip terminal text rewriting. Stable source `NodeId` provenance and target-AST
tracking remain migration work; this coarse safety path is not 07.5 completion.
Future JS-specific controls follow the same pattern: typed, versioned intrinsics
with a precise optimization envelope, never raw source strings and never a
generic “do not optimize this file” escape.

## Size-first library contract

`size-first` is an objective, not a feeling. Rank is exact transfer *T* for
the configured `cost_model` ([objectives](../compilation/objectives.md)).
A library compile is a reusable `js-module` / ESM root (marked, gl-matrix,
monaco extract, PostHog packs, `comparison/apps`), not a 20-line scalar case.

After this phase, a library compiled with

```toml
[javascript]
priority = "size-first"
cost_model = "brotli"   # or raw / gzip — one winner per invocation
candidate_search = "production"  # `always` for release measurement
```

is gated on the declared release corpus against pinned Terser / Oxc / Closure
ADVANCED configurations. It must be **no larger** than the smallest eligible
baseline for each equivalent supported corpus member. Named strict-win libraries
must remain strictly smaller, and the result must be no larger than today's
LilScript output wherever this phase only adds a legal competitor. Semantic
mismatch is red first. Beating every possible JavaScript program remains the
north star, not a theorem implied by bounded search.

### Compilation worlds

Both supported worlds still optimize a closed `.lil` graph. The difference is
which boundary observations are legal:

| World | Root boundary | What may still change |
|---|---|---|
| Closed application (`js`/executable) | Every LilScript consumer is linked; exports are source accessibility, not unknown runtime roots | all non-host names, owned fields, layouts, function boundaries, and dead exports |
| Reusable library (`js-module`) | Unknown JavaScript may call the declared root API | internals may still mangle/dissolve; declared export names and public ABI observations stay stable |

Do not model a reusable library as “optimization off.” Build an `AbiManifest`
from root exports and typed boundaries, then optimize everything not reachable
as an observable ABI fact. The same LilScript declarations and imports are used
in both worlds. Raw/gzip/Brotli and size/balanced/performance priorities must
produce the same declared developer API within a world.

The manifest must classify at least export spelling, function arity/name and
constructibility when observable, constructor/prototype identity, aggregate
field names or opaque handles, property descriptors and observable insertion
order, ESM live binding/initialization/module identity, and host/foreign names.
Original `Function.prototype.toString()` source text and engine-specific stack
formatting are explicitly outside the optimized-language ABI.
`mangle.exports` and `public_aggregate_abi` are normalized into this contract
and invalid combinations fail before optimization. Function policy is split:
public callable kind/constructibility is ABI, while private `function` versus
arrow spelling is a scored alternative. A current explicit `function_spelling`
setting may pin both during config migration. Internal owned properties remain
mangleable in library mode.

That is not a theorem about a `JsValue` transliteration of npm. Classify
([compressor surface](../language/compressor-surface.md)):

| Library kind | What 07 owes it | What 07 does not owe |
|---|---|---|
| Written as LilScript (marked, gl-matrix, piece-tree, `comparison/apps`) | Keep the win; unlock starved families so size-first can still find a smaller *T* | A new fold when a family was never searched |
| Identity-observed class API (PostHog error-tracking) | Constructor-value + IR `class` so the compact spelling is legal | `JS.method*` / `defineProperty` fusion as the product |
| Plain-data parsers (micromark family) | A typed no-hook object, then search dissolve vs keep | Default-on `assume_pure_property_reads` |
| Ordinary-`{}` dictionaries (jQuery internals) | A dictionary type + IR statement-vs-`?:` family | A jQuery peephole; post-hoc `if` contraction already lost |
| Genuinely dynamic public API (clsx) | Measure the hatch | Invented types |

### What `size-first` is, and what the root file is not

`JavaScriptPriority::enables_compression` is the size-first matrix: packing,
property mangling, joint representation / chunk search, IR variants,
parameterized merge, outlining (searchable, default off), length-to-number
elision, path-sensitive / superopt, …

Root `lilscript.toml` sets `priority = size-first` and then **replaces** that
matrix with a language-test subset. It omits `property-mangling`,
`joint-representation-search`, `joint-chunk-symbol-search`, `region-outlining`,
`length-to-number-elision`, `expression-superoptimization`,
`path-sensitive-propagation`, `array-pipeline-fusion`,
`parameterized-function-merging`. A library built under that file is not a
size-first library compile.

Library release configs omit `compression = [...]` (so the priority matrix
applies) or list the full size-first set. 07.2 explain output must name
families an exact list **removed**.

### Why libraries lose today

These are the new-doc findings, not a new theory.

| Cause | Where it lives | Why a library feels it |
|---|---|---|
| Last gated search ranks invalid or wrong-binding JS; candidate fix awaits corpus verification | 07.1 / ident-05 | **landed** for marked/react-markdown always; search-02 still records deltas |
| Proposal budget ÷4 above 16 KiB, ÷12 above 64 KiB | 07.6 / search-03 | Real libraries never reach layout, class, naming |
| Sequential 0-delta pairs | [objectives](../compilation/objectives.md) | `function_spelling` × `stable_local_names` on jQuery |
| Brotli one-way priors | 07.3 | **landed:** packing and identifier-string pooling re-enable when the compression decision is legal |
| Always-on scalar replacement | 07.3 / registry | **landed:** `keep-object` IR clone plus a dedicated on/off ablation; root TOML subset still does not admit it |
| No IR named `class` | 07.4 | Identity-observed APIs pay table tax |
| Type-only `export class` | 07.7 | Ports rebuild constructors in user space |
| `assume_pure_property_reads` | 07.7 / md-01 | Flag, not a type; ~5 850 extra stores without it |
| Root / port compression subset | 07.2 | Size-first families silently absent |
| Source and generated i32 coercions are indistinguishable | 07.2 / 07.7 | An explicit source `x \| 0` can disappear as if it were compiler noise |
| ABI and objective knobs share one options object | 07.2 / 07.7 | A codec/profile can accidentally change observable library shape |
| Glue-TS internals | compressor surface | Competing with Terser on Terser’s terms |

### Designed loop — never glue

```text
contract (semantics + world/ABI + explicit intent)
  → proof (types + identity + escape + effects)
  → legal representations (IR / emitter families)
    → bounded search scored by exact T
      → configured baseline retained
```

### Target compiler dataflow

The destination is one optimizer with multiple abstraction levels, not an IR
optimizer followed by a string optimizer:

```text
source AST with NodeId / SymbolId
  → normalized CompilationContract + AbiManifest
  → typed high-level IR (allocations, closures, constructors still explicit)
  → SSA/effect/escape/range/identity facts
  → ChoiceGraph of proof-legal representations and call-graph transforms
  → candidate DecisionVectors
  → hygienic JavaScript AST (BindingId / PropertyId / lowering obligations)
  → target-only contractions + printer alternatives
  → parse/binding/ABI/obligation validation
  → exact complete-artifact raw/gzip/Brotli score
```

The high-level IR must retain a class, object, closure, or source operation until
all relevant alternatives have been registered. Destructive lowering is applied
to a candidate view, not to the only copy before profitability is known. The
`ChoiceGraph` records constraints such as “public constructor identity requires
class-like output,” “these closure captures share a mutable cell,” or “these
field uses belong to one owned slot.” A `DecisionVector` names the selected
alternative for each scope and makes candidate lineage/cache keys deterministic.

The JavaScript target representation is a real AST with resolved binding IDs,
not text that must be reparsed to rediscover scope. Printers may compete on
quotes, parentheses, ASI, and equivalent syntax. A text candidate is never
eligible merely because a partial parser accepted balanced tokens.

Glue is anything that skips a step and patches JS: a library matcher, a
one-way `if cost_model`, an unscored search-off peephole, Terser-on-our-artifact,
a port TOML that hard-wires a pair the beam should have declared as a joint
family, a `candidate_proposal_limit` lift in one pack as the 07.6 fix.

If a library is still larger after 07, classify before coding. Do not add a
fold that only pays on that AST.

### Phase-complete (libraries)

07 is not complete when the registry compiles. It is complete when all of
these are true under `priority = size-first` and each configured `cost_model`:

1. **Typed libraries stay wins and do not regress** vs the 07 start baseline:
   marked, gl-matrix, monaco piece-tree, mitt/nanoid, `comparison/apps`.
2. **Identity-observed class libraries** emit named `class` from IR (or a
   scored table-vs-class family). PostHog error-tracking no longer needs
   user-space `defineProperty` tables to keep `constructor.name` / prototype
   enumerability, and beats Oxc on Brotli without dropping those names.
3. **Plain-data ports** can delete `assume_pure_property_reads`. Dissolve vs
   keep is searched (07.3), not forced.
4. **Library-scale explain** (`>16 KiB`) lists layout / class-identity /
   naming as reached or **starved** — never silent exhaustion treated as
   “incumbent won.”
5. **Size-first library configs** are the priority matrix, not the root
   subset. Explain names removed families.
6. **Zero** new library-specific folds. A port may pin a measured incumbent.
7. **Explicit lowering survives.** A live source `x | 0` is present in every
   objective artifact while generated redundant i32 normalization remains free
   to vary.
8. **API parity is objective-independent.** Raw/gzip/Brotli and every priority
   pass the same generated `AbiManifest` tests in library mode; only internals
   differ.
9. **Representation decisions are explainable per source entity.** Allocations,
   closures, properties, and call sites report legal alternatives, rejected
   proofs, selected choice, objective delta, and budget starvation.

### Refuse even if it would shrink a library this week

- Post-minify of LilScript output.
- Default-on `assume_pure_property_reads` / silent `pure_getters`.
- Flip `export class` to emit constructors (destroys dissolved identity-free types).
- Infer ordinary `{}` after record observation projection.
- Always-on Brotli ternary / jQuery `if(` contraction (jquery-01 lost).
- Pack-local proposal-limit lifts as a substitute for 07.6 reserved slices.
- ROM dictionary bait, host-name aliasing as size tactics.
- Removing or respelling a live source-authored `|0` because range analysis says
  it is semantically redundant.
- Letting `cost_model` or `priority` alter a public export, descriptor,
  constructibility, or field-name contract.

## Order

Do not skip ahead to “score more `IrJsOptions`” while identity is red. A
broader beam that ranks invalid JS is a worse compressor.

| Step | Purpose | Exit signal |
|---|---|---|
| [07.1](#071--identity-before-search) | Search cannot select a program whose names or identity are wrong | ident-05 and related identity tasks green; every retained candidate parses and resolves |
| [07.2](#072--one-registry) | One normalized contract, provenance model, and classification of every decision | Adding a tactic is a registry row + one construction site; source/generated origins survive in shadow mode and ABI choices cannot enter the scored set |
| [07.3](#073--reversible-priors) | Codec-conditioned defaults remain priors, not one-way doors | Packing, identifier-string pooling, scalar-replacement off, and remaining compact-is-better flags have incumbents **and** scored opposites |
| [07.4](#074--ir-emits-legal-shapes) | Named ES `class`, arrays, objects, scalars, and closure environments are IR/emitter representations | IR can emit a proof-marked named `class`; identity-free aggregates and closures expose every proof-legal representation; public constructor syntax integrates in 07.7 |
| [07.5](#075--peephole-is-contraction) | Target JS is hygienic; parsed JS only contracts already-legal programs during migration, always scored or skipped | Search-off has no unscored full peephole; binding-aware target AST replaces reparsing; class identity comes from IR |
| [07.6](#076--search-that-can-finish) | Late families and joint interactions have reserved budget | Proposal exhaustion cannot drop class/naming/layout families unnoticed; explain output names starved rows |
| [07.7](#077--language-proofs-and-explicit-lowering-contracts) | Ports state proofs and intentional JS boundaries in the language rather than Terser-shaped flags | `assume_*` gains typed replacements; live explicit `\|0` survives; application/library ABI cases are objective-independent |

07.1 is sequential with the existing `ident` lane. 07.2–07.3 can overlap once
identity is trusted. 07.4 unblocks 07.5. 07.6 can start as soon as the registry
exists (reserved slices are independent of class emit). 07.7 is language work
and may run in parallel as RFCs/case design, not as new compiler flags. Its
constructor-value implementation consumes 07.4; its exact-operation enforcement
consumes 07.5's obligation-aware target AST.

## 07.1 — Identity before search

Board: `ident-05` and anything that still lets a smaller invalid artifact beat a
larger valid one.

- A candidate that fails parse, delimiter, class-body, or binding resolution
  is not a finalist. Scoring it is wasted work and a false minimum.
- Constructor identity, `Function.name` / `.length`, and prototype
  enumerability are proofs carried **into** search, not recovered after a
  table-shaped emit.
- Differential oracle shapes for receiver rebinding and sub-expression-as-value
  (ident-02, ident-03, ident-08 class) stay red if they regress.

Exit is not “more peephole guards.” Exit is: the selector’s admitted set is a
subset of semantically valid JS for the configured ABI.

## 07.2 — One registry

Today: `CompressionDecision`, `JavaScriptOptimization`, `IrJsOptions`,
`OptimizationOptions`, `js_options()`, and ~40 `extend_javascript_candidate_beam`
closures. The [decision registry](../compilation/decision-registry.md) is the
prose version of the table the compiler should own.

- Every `IrJsOptions` field declares: proof-only / ABI / incumbent / scored /
  illegal-to-flip.
- Introduce stable `NodeId`/symbol/property identities and operation provenance.
  Stop using shifted `Span` values or source spellings as semantic identity.
- Build one normalized `CompilationContract` before optimization. World/ABI,
  target assumptions, and source lowering obligations are immutable inputs to
  legality; the objective is a separate input to profitability.
- A registry row declares its scope (module, function, call site, allocation,
  closure environment, property family, chunk), proof predicate, legal
  alternatives, incumbent, incompatible decisions, objective family, budget
  class, validator, and explain schema.
- Beam construction iterates the scored set. A field cannot be “searched” in
  docs and absent from the iterator.
- Dual-gated features (compression **and** optimization) stay dual-gated, but
  in one place (`optimization_enabled` already does this; beam construction
  should call it, not re-encode the matrix).
- The former exhibit: `elide_length_tonumber` flipped with no compression gate.
  That family is now `length-to-number-elision` in `SCORED_EMISSION_FAMILIES`;
  omitting the name keeps the spelling off.
- Root `lilscript.toml` subset remains allowed **for language tests**. Library
  release compiles use the size-first matrix (omit `compression` or list it
  whole). Explain output must say which scored families an exact list
  **removed**, not only which it ran. See
  [size-first library contract](#size-first-library-contract).

Exit: a reviewer can answer “is layout searched in this compile?”, “why is this
property stable?”, and “was this operation source-authored or generated?” from
one table and one explain dump. No objective decision can mutate the normalized
contract. The v0.2 `|0` behavior itself remains gated on 07.7 language approval.

## 07.3 — Reversible priors

Keep codec-conditioned incumbents. They are empirically useful (Brotli and
identifier-shaped string pooling fight; packing under Brotli was disabled for
a reason). Change the contract:

- Incumbent = prior for this `cost_model`.
- Search, when the compression decision is legal, must be able to propose the
  opposite **including re-enabling** a Brotli-forced-off packing flag.
- IR always-on size passes that change representation (scalar replacement)
  need an off-clone or an explicit “proof-mandatory, not a size tactic”
  classification. If dissolution is mandatory because the object is unobservable,
  say so. If it is a size tactic, score keeping the object.
- Inlining, specialization, outlining, function merging, and closure-environment
  lowering are opposing choices in one call-graph family. Instruction-count
  heuristics may order proposals; they may not be the final size verdict.

Measure on more than jQuery. `struct_method_shorthand` was the successful
template: default stays, search flips, ports that have measured their artifact
may pin the incumbent. Do not cite md-01 as a scalar-replacement win/loss: its
typed-bag and captured-local experiments changed source shape and one was
semantically broken. Add a dedicated scalar-replacement on/off ablation.

The former exhibit: Brotli `js_options()` forced `pack_string_arrays` and
`pool_identifier_strings` off, and Cartesian could not reopen them; scalar
replacement had no keep-object clone. Packing and identifier pooling now use
`reversible_boolean_alternatives` on cartesian axes. `keep-object` is a
`SCORED_IR_VARIANTS` row gated by `joint-representation-search`.
`optimizer::tests::scalar_replacement_on_and_keep_object_are_both_legal` is the
isolated ablation. Exact `compression = ['identifier-mangling']` does not admit
keep-object. Full call-graph cartesian remains 07.6.

Exit: size-first Brotli library compiles can try packing, identifier-string
pooling, keep-object, and the named call-graph IR opposites. Incumbents stay
first. Language tests on the root TOML subset do not grow those clones.

## 07.4 — IR emits legal shapes

Follow [class identity](../compilation/class-identity.md) phases 1–3 without
turning object-lowering off globally:

- Identity-free structs/classes stay dissolved / positional / named as today,
  with joint layout search **actually reachable** from release configs that
  claim size-first.
- Identity-observed constructors emit named `class` (or a scored table vs
  class family). `export class` remains type-only unless the constructor
  **value** is exported.
- Add an IR/emitter unit fixture for a proof-marked identity-observed
  constructor versus existing `class-scale` / `class-counter` (`lt`, must keep
  dissolving). The public `canonical/aggregates/exported-class-identity` case
  lands with the 07.7 constructor-value syntax, using this representation.

This is how “must I keep this class?” becomes a search question instead of a
port-specific `defineProperty` novel.

Representation selection is per allocation/closure SCC, not one global
`aggregate_layout` boolean:

| Entity | Proof-legal alternatives |
|---|---|
| Local identity-free aggregate | no allocation/SSA fields, scalar tuple, positional array, named object |
| Typed escaping aggregate | positional array, mangled owned object, stable named object at ABI |
| Identity-observed constructor | named ES `class`, constructor/prototype form when semantically identical |
| Non-escaping closure | inline body, lifted function with scalar captures, native lexical closure |
| Repeated/escaping closure | lexical closure or explicit positional/named environment when capture identity and mutation permit |
| Owned property family | numeric slot, mangled identifier, quoted stable key according to ownership/ABI proof |

Property identity becomes `(owner, slot)`, not a source string. The trailing-`_`
private-key convention is removed once typed ownership covers it. Host,
`Record<T>`, dynamic-string, reflected, and public-manifest keys remain exact.
Closure capture slots are compiler-owned and may mangle or become positional;
an escaping public callable still preserves manifest-required `name`, `length`,
constructibility, and own/prototype descriptors.

## 07.5 — Peephole is contraction

The parsed-JS pipeline stays during migration, but it is not the destination.
ASI, keyword spacing, and local statement contraction belong on a hygienic
target JavaScript AST **after** IR has chosen a representation. Binding identity
must be carried into that AST instead of reconstructed from generated strings.

- Search-off: either skip peephole or score one clone against the untouched
  emit. Do not apply `optimize_generated_javascript` as an unscored policy.
- Every terminal challenger, including the current
  `apply_selected_canonical_peephole` path, must use the same full
  `javascript_candidate_rank`, startup guards, ABI validator, and lowering-
  obligation validator as other finalists. Transfer-then-raw-only acceptance is
  not valid for balanced or performance priorities.
- Class fusion, copy coalescing identity, and “sub-expression means parent
  value” bugs move to emitter/IR proofs or die as folds.
- Late cleanup already treats skip-the-pass as a branch; the full pipeline
  must not be a stealth always-on second optimizer on the winner except as
  the already-scored `apply_selected_canonical_peephole` comparison.
- Migrate contraction folds one family at a time from parsed text to the target
  AST, with binding-aware before/after tests and exact-codec ablations. Retire
  the generated-JS tokenizer/parser once no production decision depends on it.

## 07.6 — Search that can finish

- Reserved proposal slices for layout, class-identity emit, and naming — the
  same idea as the terminal naming reserve — so a **library-scale** (16 KiB+)
  artifact does not spend the ledger on punctuation families and never reach
  them. A pack-local `candidate_proposal_limit` lift is config glue, not this
  step.
- Explain human/json lists **starved families**, not only total work units.
- Joint families stay explicit and rare (helper × table is the model). Do not
  replace sequential search with a full `IrJsOptions` cross-product. Measured
  non-monotone pair that sequential search cannot reach: `function_spelling` ×
  `stable_local_names` on jQuery callbacks ([objectives](../compilation/objectives.md)).
- Broad-module phase-order collapse stays a compile-time trade; document it
  as “missed IR interaction,” not as equivalent to the small-module search.

The target search is a deterministic portfolio, not a giant Cartesian product:

1. Generate only proof-legal alternatives from registry rows.
2. Keep the configured incumbent and contract-pinned choices unconditionally.
3. Use cheap raw/static estimates only to order work. Prune only byte-identical
   candidates or candidates with a proof that every legal continuation is the
   same and the exact configured rank is no better; raw dominance alone never
   prunes a gzip/Brotli candidate.
4. Reserve work by family: representation, call graph, control flow, naming,
   pooling/layout, and terminal contraction.
5. Keep structurally and raw/gzip/Brotli-diverse intermediate candidates.
6. Validate parse and binding identity before an artifact can be scored.
7. Select with exact raw/zlib/Brotli bytes and the configured priority.

Raw, gzip, and Brotli are separate invocations and may intentionally select
different inlining, layouts, names, literal pools, and declaration order.
`size-first` lands first. Balanced and performance policies reuse the same legal
set but add explicit performance constraints/ranking; they never weaken proof,
ABI, or lowering obligations. Static performance estimates remain selection
proxies and are calibrated against browser measurements rather than advertised
as measured runtime.

For `split` and `preserve-modules`, normalize a `DeliveryObjective` instead of
silently switching scoring systems. Its transfer term is the selected
`cost_model` applied to every emitted chunk, weighted by reachability/cache
policy, plus explicit request and depth costs; `priority` ranks that plan against
its aggregate performance model. The existing `[bundle.cost]` raw/gzip/Brotli
weights become an explicitly reported legacy mixed-codec objective during
migration. They must not silently override `javascript.cost_model`. Fixed
`preserve-modules` partitioning still searches legal symbol/layout choices.

Compile time remains a first-class axis of the [tradeoff triangle](../mission.md).
`always` + huge budgets is a release profile, not the edit loop.

Search should reuse immutable analyses and structurally hash equivalent
candidate states. A family may use a learned or corpus-derived prior to choose
proposal order, but that prior is versioned evidence, never legality and never
the final winner. Cache keys include target, contract fingerprint, decision
vector, and compiler/codec version.

## 07.7 — Language proofs and explicit lowering contracts

Ports lose to Terser/Oxc/Closure when the `.lil` cannot state a fact those
tools guess from JavaScript. A compiler flag that papers over the hole is
glue. Inventory and refusal list:
[compressor surface](../language/compressor-surface.md).

Each RFC is syntax + semantics + paired cases, or an explicit **unsafe ABI**
flag that stays default-off. None is a peephole special case or a library
matcher.

| RFC | Proof it gives search | Must not |
|---|---|---|
| Constructor **value** export, distinct from type-only `export class` | Connects public syntax to the identity-observed IR `class` family built in 07.4 | Flip the type-only default; emit `class` for identity-free `class-scale` |
| Plain-data / no-hook object | Dynamic `o[k]` is not a getter/proxy; `assume_pure_property_reads` becomes the typed form or stays unsafe-off | Silent Terser `pure_getters` in the optimizer |
| Ordinary-`{}` dictionary vs null-proto `Record<T>` | jQuery-shaped maps without `createEmptyObject()` trampolines; Record stays `Object.create(null)` | Infer `{}` after observation projection |
| Expression-if / general `match` (beyond enum) | Statement vs `?:` is an IR family the codec can score; jquery-01 post-hoc contraction already lost | Always-on Brotli ternary prior |
| Host-callable typed method (`this` + rest on a typed receiver) | Public JS methods without `JS.method*` `JsValue` wrappers | Treating JS `this` as free; fusing escaping wrappers |
| Getters/setters as ABI | Accessor contract without `defineProperty` novels | Peephole inventing accessors |
| Sound optional / structural bags | Motion-style option objects without `JsValue` | Structural TS `any` |
| Explicit i32 normalization | Source-written live `x \| 0` carries a JS lowering obligation distinct from generated int normalization | A global `integer_coercions` switch erasing source intent; treating all compiler-generated coercions as pinned |
| Typed target lowering intrinsics | A narrow, versioned way to request an exact JS operation when semantics alone are insufficient | Raw JS strings, arbitrary text templates, or a file-wide optimization barrier |
| Application vs library boundary contract | Same source API; closed internals in both; stable manifest only where unknown JS can observe it | “Library mode” disabling internal mangling/inlining, or an objective changing public ABI |

Proxy, Reflect, `instanceof` constructor identity stay host.

“Plain data” is not merely a nominal label. A hook-free read proof requires a
compiler-owned non-proxy allocation, no accessor definitions, no untyped escape
before the read, and either a proven-own key, a controlled null prototype, or a
separately explicit pristine-prototype assumption. An ordinary `{}` dictionary
does not make arbitrary missing-key reads pure because mutable
`Object.prototype` may contain an accessor. Values entering from JavaScript need
validation/copying or remain under an explicit unsafe boundary contract.

Order relative to compiler work: RFCs may be drafted in parallel with 07.1.
They must not land as optimizer flags first. The 07.4 named-class representation
enables constructor-value syntax for published classes; plain-data unblocks deleting
`assume_pure_property_reads` from port TOMLs; expression-if unblocks treating
`local_phi_expression_regions` as recovery of source, not invention.

Existing surface that is **not** an RFC: `enum`+`match`, `object` singletons,
class `this`, `pure`, positional `struct`, `JS.method*` as the dynamic hatch.
Ports that still use `int kind` ladders are unfinished ports.

Every target-lowering RFC states the exact operation/spelling it preserves,
which surrounding transformations remain legal, behavior on unsupported
targets, and whether dead enclosing code may disappear. No ordinary syntax is
silently reinterpreted as a hint except the deliberately specified source
`x | 0` rule above.

The source-feedback loop is part of this step. `--explain` must attribute
retained bytes to source entities and say whether the blocker was public ABI,
escape, dynamic property access, identity, effects, an explicit obligation, or
budget starvation. If LilScript source is the reason a library loses, rewrite
the source to state a reusable proof (`struct`, `enum`, plain-data object,
typed method) and add a paired case. Do not compensate with a library matcher.

## Working rules

1. No library-specific compiler folds. jQuery, marked, PostHog, markdown
   stacks are **pressure**, not matchers.
2. No post-minify of LilScript output.
3. Do not weaken a paired gate because the current architecture loses it.
4. Prefer deleting port identity emulation after 07.4 over teaching the
   peephole more `defineProperty` shapes.
5. Record negative complete-artifact measurements. A reversible prior that
   loses everywhere can stay a prior; it must not stay irreversible “because
   we already know.”
6. A step that does not move the [size-first library contract](#size-first-library-contract)
   is unfinished, even if the code looks cleaner.
7. Objectives may select different JS, never different language behavior,
   public ABI, or explicit source intent.

## Landing strategy and gates

This is an architectural replacement without a big-bang rewrite:

1. **Freeze evidence.** Record current compiler SHA, config fingerprints,
   raw/gzip/Brotli outputs, runtime/API results, compile time, and peak memory for
   canonical cases and representative libraries. Existing red rows stay named.
2. **Add identities and contracts in shadow mode.** Assign `NodeId`, `BindingId`,
   `PropertyId`, operation origins, and an `AbiManifest` without changing output.
   Compare old/new classifications and fail on unresolved ownership. This step
   records source/generated provenance but does not change v0.1 `|0` behavior;
   07.7 activates the v0.2 obligation after 07.5 can preserve it in target AST.
3. **Install the registry around current behavior.** Every old boolean and pass
   becomes a row whose incumbent reproduces current output. Explain snapshots
   prove no family silently disappeared.
4. **Migrate one family at a time.** Representation, call graph, control flow,
   naming, pooling/layout, and terminal contraction each gain legal alternatives,
   isolated ablations, and a disabled/incumbent candidate.
5. **Introduce hygienic target JS AST.** Initially print byte-identical output;
   then move parsed-JS contractions family by family. Delete each text fold only
   after semantic and codec gates pass.
6. **Retire duplicate policy.** Remove imperative beam flips, magic property-name
   conventions, duplicated emitter analyses, and the legacy AST emitter only
   after no production path consults them.

Required gates for each milestone once its supporting contract machinery has
landed (the intent gate begins when the v0.2 contract lands in 07.7):

| Gate | Requirement |
|---|---|
| Semantics | optimized, optimizer-disabled, and independent interpreter agree; JS/C/native agree where portable |
| Identity | every candidate resolves each use to the intended `BindingId`; no unresolved or nearer-binding substitution |
| Intent | live explicit `x \| 0` survives search on/off and all objectives; generated redundant normalization has both expected policies |
| ABI | application and library fixtures validate names, arity, constructibility, descriptors, class identity, fields, and host names |
| Representation | scalar/array/object/class and closure variants execute identically; illegal alternatives explain the failed proof |
| Objective | size-first raw/gzip/Brotli builds independently select the smallest found legal artifact; other priorities beat the incumbent under their configured rank and guards |
| Scale | >16 KiB libraries report reached/starved families; no silent budget exhaustion |
| Performance | non-size priorities satisfy configured guards; browser evidence calibrates static estimates |
| Determinism | same process, separate process, thread counts, and supported OSes emit the same artifact and decision report |
| Resources | production-search p95 wall time and peak RSS stay within 10% of the frozen library baseline unless an approved size delta explicitly buys more budget; configured candidate/byte limits are hard |

First contract fixtures, before implementation convenience tests:

- `canonical/scalars/explicit-i32-intent`: live source `x | 0`, generated
  normalization, dead explicit operation, search on/off, three codecs;
- `canonical/host/library-abi-manifest`: the same source under application and
  library worlds, with exported names/arity/constructibility/descriptors checked;
- `canonical/aggregates/exported-class-identity`: constructor value is public,
  while existing identity-free class cases still dissolve;
- `canonical/functions/closure-environment`: immutable/mutable captures,
  escaping/non-escaping closures, and public callable identity;
- `canonical/aggregates/property-ownership`: private owned, public, extern,
  `Record`, reflected, and dynamic keys in one mangling matrix.

Current config files are a concrete compatibility requirement. During the
migration, old keys map once into `CompilationContract` or
`OptimizationObjective` and emit deprecation diagnostics when their ownership
changes. New implementation code consumes only normalized values; it does not
carry dual semantics throughout the optimizer.

## Relationship to 00–06

00–06 are the **standing evidence loop**, not a queue in front of this phase.
Status lives on [migration](README.md). Keep adding canonical folders. A
registry row without a case is how heuristics return.

- Phase 03 gains `aggregates/exported-class-identity` (`le`) when 07.4 IR emit
  and 07.7 constructor-value syntax both land.
  `class-scale` / `class-counter` stay `lt`.
- Phase 02 does not close the jQuery `if(` gap; that is 07.7 expression-if plus
  an IR family (jquery-01 already lost post-hoc contraction).
- Phase 05 is where 07.1 selector holes become minimized compiler tests and
  canonical folders.
- Phase 06 stays green while this phase lands. `gate-01` is a separate
  codec-contract red.

## Out of scope for this phase

- Beating `jquery.min.js` by rewriting `deferred.lil` (already refuted as
  the remaining gap).
- ROM dictionary bait, `.length` → `["length"]`, host-name aliasing as
  size tactics ([research](../research/brotli-global-mangle/08-bait-and-glue.md)).
- Native inheritance ABI (real, separate).
- Making `--mode development` identical to production search.
