# Current compiler architecture

Status: implemented behavior in this checkout. Goal architecture:
[goal architecture](goal-architecture.md).

Parent: [Compilation](README.md). Behavior catalog:
[decision registry](decision-registry.md). Ranking math:
[objectives](objectives.md). Language leverage:
[compressor surface](../language/compressor-surface.md). Mission:
[mission](../mission.md). Plan:
[07 — global compressor](../migration/07-global-compressor.md).

Snapshot of **what the compiler is**, not what the comments wish it were.
Date of this reading: 2026-08-28.

## Implemented thesis

The implementation already combines:

- a closed typed module graph and whole-program facts;
- typed CFG/SSA optimization and representation lowering;
- multiple optimizer and emission variants;
- exact scoring of emitted artifacts with pinned raw, gzip, or Brotli metrics;
- a separate native/C optimization path from the same lowered IR.

`optimize_and_select_javascript`, two-level search, bundled zlib 1.3.1 / Brotli
1.1.0 scorers, `[optimization]` vs `javascript.compression` vs
`javascript.optimizations`, typed `extern`, scalar replacement, positional
fields, and entropy-aware mangling are production code. Search is bounded and
heuristic; exact codec measurement does not make the explored domain exhaustive.
The [goal architecture](goal-architecture.md) specifies the replacement decision
system. This page records the current gaps.

## What “glue” means here

Glue is a locally motivated rewrite, port workaround, or compact-is-better
default that is **not** expressed as (proof → legal representations → complete
artifact score). It includes:

- a fold that repairs invalid or identity-wrong JS the emitter produced;
- a `cost_model` `if` that disables a spelling search cannot turn back on;
- an IR pass with no competing “don’t” candidate (scalar replacement);
- a sequential beam family that never runs because the proposal ledger emptied;
- a library-shaped assumption (`assume_pure_property_reads`) papering over a
  language hole;
- reconstructing ES class identity in user-space `JS.method*` /
  `Object.defineProperty` because `export class` is type-only.

The board’s standing refusal is “no glue.” The compiler still contains glue
because search, identity, and representation were grown as patches on a
working pipeline rather than as one decision system.

## The real pipeline

Configured path compilation (`src/compiler.rs`, `src/module.rs`):

```
toml → discover → parse → link → semantic → lower to CFG
    → optimize_and_select_javascript
         IR clones (inlining/specialization/compress/phase-order)
         emit each finalist with IrJsOptions
         sequential emission-beam families
         terminal peephole / cleanup / naming (budgeted)
    → optional native: clone of the **lowered** IR, optimized with
      `optimizer_options()` / `[native]`, not the JS search winner
```

Legacy `src/codegen_js.rs` is AST-direct. New size tactics do not belong there.

`--mode development` and `candidate_search = off` skip multi-IR/emission
expansion and zero the optional terminal codec budget. The configured pipeline
still runs. Parsed peephole, if configured, then applies
**unscored** (`apply_search_off_declaration_peephole`). That is a different
compiler than production search, not a fast approximation of the same one.

`split` whole-program optimizes once, then scores chunk plans with
`score_javascript_chunk_plan`. With `joint-chunk-symbol-search`, it also scores
`function_layout` and `local_name_reserve` variants; it does not rerun the
single-file identifier-alphabet beam. `preserve-modules` uses fixed source
partitions and configured `js_options()` rather than the chunk-plan scorer. Root
compression omits `joint-chunk-symbol-search`.

## The semantic firewall is missing

The current pipeline does not materialize one immutable distinction between:

- language semantics;
- closed-application versus reusable-library ABI;
- explicit source lowering intent;
- unsafe host assumptions;
- objective and search policy.

Those concerns meet in `JavaScriptConfig`, `OptimizationOptions`, and
`IrJsOptions`. The first narrow exception now exists: source `value | 0` carries
`LoweringObligation::PreserveJavaScriptBitOrZero`; generated normalization for
other `int` operations remains emitter-owned, and candidates with a live
obligation skip text rewrites that cannot preserve it. General stable `NodeId`
provenance and target-AST obligation tracking do not exist yet.

The proposed separation into contracts, objectives, and durable provenance is
specified in [goal architecture](goal-architecture.md#semantic-firewall).

## Two optimizers, not one

### Typed IR (`src/optimizer.rs`, `src/compress_passes.rs`, `src/value_analysis.rs`)

An ordered schedule contains scalar fixed points and repeated inlining rounds,
plus one-shot interprocedural, escape, scalar-replacement, and compression
stages. This is the Closure-like whole-program machine. It is mostly
**heuristic with gates**: pass on/off and numeric inline budgets, not
complete-artifact scores. Search wraps it by cloning `OptimizationOptions` a
handful of times (no-inline, no-factory, no-CSE, outlining on/off). It does not
wrap the pass list as a searchable object.

Scalar replacement of `LocalOnly` structs/classes is the flagship typed win and
is **not** an IR search dimension. If dissolving a point into `x`/`y` locals
hurts Brotli on some program, production cannot keep the object.

### Parsed JS (`src/js_peephole/`)

A Pratt-parsed rewrite pipeline over generated JavaScript: copies, control,
loops, ASI, integers, declarations, inlining, and a very large **class identity**
module. Docs used to list five or six rewrites. The implemented session in
`optimize_generated_javascript` runs dozens, including
`fold_constructor_prototype_tables_to_classes` and `fold_named_class_identity`.

This second optimizer exists because:

- IR emission still leaves statement-shaped JS a human minifier would contract;
- identity-observed constructors are not emitted as named `class` from IR;
- several miscompiles were fixed across SSA name coalescing, emitter assignment
  parsing, and parsed-JS folds; ident-08 is the recurring
  sub-expression-as-enclosing-expression class.

When search is on, these rewrites enter normally ranked leaves or a late cleanup
beam (skip-the-pass is a first-class branch). The canonical-winner challenger is
now charged to the terminal ledger and uses the full configured rank and startup
guard. Search-off's function-preserving challenger is exactly scored against the
untouched emit. `repair_late_javascript_candidate` still forces several cleanup
transforms inside candidate construction, and generated text is still reparsed;
the hygienic target AST remains unfinished.

## Search is a coordinator, not a solver

`src/compiler.rs` remains the candidate coordinator: arenas, beams, proposal
ledgers, Rayon pools, entropy alphabets, and terminal naming. A code registry now
classifies all 77 `IrJsOptions` fields and owns cartesian axes, sequential
families, and scored IR variants. Entropy mapping and several terminal
neighborhoods remain coordinator-specialized rather than graph-solved.

Consequences:

1. **Order matters.** `stable_local_names` is probed both early and late because
   an early-only split sent zod the wrong way (−58 Brotli). That comment is the
   architecture admitting sequential search is not monotone.
2. **Budgets starve late families.** Artifact scaling divides proposal/terminal
   work by 4 or 12 above 16 KiB. Class-shape and naming families sit at the end.
   Pack-local TOML lifting the proposal limit is a config glue for a compiler
   budget bug. Selected structural priorities and terminal naming/string work
   already have reserves, but there is no minimum coverage guarantee or
   starvation report for every representation family.
3. **Invalid programs can be smaller.** [ident-05](../migration/board/notes/ident-05.md)
   is active. A working-tree fix now reserves enclosing bindings and has focused
   regression tests, but its full marked/react-markdown gate is not recorded as
   landed. The last validated state allowed unresolved or wrong-nearer-binding
   artifacts to rank, so identity remains the blocking class.
4. **Cross-product is sampled, not enumerated.** Joint expansions exist only
   where someone paid for them (pure-helper × dense tables; function spelling ×
   single-use inline; aggressive inlining × outlining). Every other interaction
   is “hope the beam kept the right parent.”

Objective stratification (keep some raw/gzip/Brotli-diverse shapes in the
intermediate beam) is the right idea for recovering non-local wins. It does not
make the terminal ranking multi-objective. One `cost_model` still wins.

## Policy is scattered

A single tactic currently touches some of:

| Place | Example |
|---|---|
| `CompressionDecision` | `structured-closure-inlining` |
| `JavaScriptOptimization` | `ir-inlining-variants` (sometimes **also** a compression decision) |
| `JavaScriptPriority::enables_compression` | size-first-only packing |
| `js_options()` | Brotli incumbents turn packing and identifier-string pooling off |
| `IrJsOptions` field | `pack_string_arrays` |
| beam / `src/decision_registry.rs` | packing and identifier pooling use `reversible_boolean_alternatives`; `keep-object` is a scored IR clone |
| `[optimization]` | `inlining`, `scalar_replacement` |
| board note / port TOML | `lilscript.identity.toml` proposal limit; `assume_pure_property_reads` |

`optimization_enabled` requiring **both** level/allowlist **and** compression
for “legacy” dual-gated features is correct and easy to misconfigure. Putting
`ir-inlining-variants` in `optimizations` but omitting it from an explicit
`compression` list leaves the search off.

Root `lilscript.toml` is itself a policy object: `priority = size-first`,
`cost_model = brotli`, `candidate_search = production`, `local_name_reserve = 48`,
and a **subset** compression list. Language tests compile under that subset, not
under the full size-first matrix. Claims about “what size-first does” must name
whether they mean `enables_compression` or the root file.

## Language holes that force compiler and port glue

These are upstream of search. Search cannot invent proofs the type system
refused. Full inventory: [compressor surface](../language/compressor-surface.md).

| Hole | What happens | Why it is not a fold |
|---|---|---|
| Constructor **value** vs type-only `export class` | `export constructor C [as PublicC];` now marks and emits a named runtime class while `export class` remains type-only. Existing ports still carry `defineProperty` / `JS.method*` identity tables until migrated. | Internal inheritance and a base-class default constructor are preserved; derived defaults require explicit `init`/`super`. |
| `JsValue` and dynamic `o[k]` | A member read is a coercion/proxy hook unless `assume_pure_property_reads`. Markdown stack: ~5,850 extra `NAME=…;` statements. The flag is Terser `pure_getters`, default **off**. Typed bags on that stack **lost** Brotli. | Plain-data proof, then **search** dissolve vs keep. A flag is a library contract. Always-on scalar replacement is also glue here. |
| `?` is nullable | Source expression-if and enum/int/string/bool literal `match` create conditional phis and compete as `?:` versus structured control. | Destructuring and guarded patterns remain outside the current language. |
| Open records vs ordinary objects | `Record<T>` remains null-prototype; `object{...}` now creates an ordinary-prototype `JsValue` dictionary with observable inherited hooks. | Typed ordinary dictionaries, spread, and per-allocation hook-free proof. |
| Host-callable `this` on typed values | Class methods already have `this`. Public JS methods go through `JS.method*` `JsValue`. | Typed host-callable method ABI, or keep the hatch. |
| Host `document` / `window` / builtins | Direct emit, no wrappers. Regex literals require `assume_pristine_builtins`. | Correct. Do not add trampolines (`pickRegex3` is a standing refusal). |

jQuery’s remaining gap is compressibility (smaller raw, larger Brotli) and
**control-flow representation**, not a missing local fold. Sequential search
also cannot reach pairwise-neutral combinations
([objectives](objectives.md); [jquery-01](../migration/board/notes/jquery-01.md)).
Terser −345 on **our** artifact is emergent pipeline interaction. Implementing
that as six separately-landed peepholes has already lost.

## Native is honest and secondary

The same IR lowers to C. `JsValue`, `Regex`, `Task`, `extern class`, dynamic
import are **rejected** on native rather than faked. Inheritance is rejected on
native until the subtype pointer ABI exists. JS size wins must not depend on
semantics native cannot share. `[native]` only places storage. JS
`javascript.priority` does not change the C optimizer.

## What is already globally minded (keep)

- Complete-artifact zlib/Brotli scoring, not an entropy proxy as the winner.
- Configured baseline always retained; experimental variants may fail.
- Search may disable an enabled tactic. Most omitted non-search-only decisions
  stay off (Cartesian `[configured, false]`, or `if configured.*` families).
  Exception: `elide_length_tonumber` is flipped unconditionally in
  `select_javascript_candidate_global`, so omitted `length-to-number-elision`
  can still turn on. Size-first search-only names such as `indexed-char-at` use
  `search_compression_enabled`.
- `[optimization] foo = false` is a hard off.
- Objective-stratified intermediate retention.
- Skip-the-rewrite as a late-cleanup branch.
- `struct_method_shorthand` and several other compact defaults promoted into
  scored families after measurement.
- Differential AST oracle (`lilscript-differential`) that does not go through SSA.
- Paired-case gate: LilScript compiled under `cost_model = m` vs the smallest
  valid JS minifier on metric `m`.

## Current gap summary

- ident-05 has landed (marked `local_name_reserve` 0/8/12/48 is 660/660;
  react-markdown `always` is 93/93). search-02 still records corpus deltas.
- There is no `ChoiceGraph` or stable decision vector; 07.2 put `IrJsOptions`
  classification and scored emission families in `src/decision_registry.rs`.
- `compilation_contract.rs` now separates world/ABI/unsafe/effect policy from the
  optimization objective and emits an ABI manifest; deeper ABI validation is
  still incomplete.
- `export constructor C [as PublicC];` marks identity-observed constructor
  hierarchies and emits named `class`; derived defaults remain explicit.
- Owned properties retain canonical `(owner, slot)` identity through naming;
  closure search can choose lexical capture or lifted immutable scalar snapshots.
  Many contractions still operate on generated text.
- Large artifacts can exhaust proposal work before late representation and
  naming families run.
- Split chunk planning uses a separate mixed deployment-cost path;
  preserve-modules uses fixed configured emission.

The intended replacements and guarantee levels are in
[goal architecture](goal-architecture.md). Their landing order is
[phase 07](../migration/07-global-compressor.md).

## Related pages

- [Objectives](objectives.md) — size/performance × raw/gzip/Brotli; exact vs heuristic
- [Decision registry](decision-registry.md) — behavior-by-behavior table
- [Global optima](global-optima.md) — why local smaller loses gzip/Brotli
- [Compressor surface](../language/compressor-surface.md) — proofs ports still lack
- [Class identity](class-identity.md) — constructor vs instance lowering
- [Inlining](inlining-specialization-sharing.md) — opposing function transforms
- [Peephole](peephole.md) — parsed JS as implemented
- [Candidate search](candidate-search.md) — two-level search and budgets
- [Closure ADVANCED](../research/closure-advanced.md) — reference discipline, not a clone checklist
