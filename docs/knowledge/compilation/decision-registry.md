# Decision registry

Parent: [Compilation](README.md). Architecture:
[current architecture](current-architecture.md),
[goal architecture](goal-architecture.md). Ranking math:
[objectives](objectives.md). Search mechanics:
[candidate search](candidate-search.md). Knobs: [config](../config/README.md).
Migration: [07 — global compressor](../migration/07-global-compressor.md).

This page is the implemented map of the known contested compilation choices. Field
classification and scored emission families live in `src/decision_registry.rs`.
It is not the TOML schema and not the intended end state. A row that says
“heuristic” or “never scored” is a gap, not a blessing.

Authority order, highest first. A lower layer may not legalize what a higher
layer forbade.

| Layer | Question | Who answers it today |
|---|---|---|
| 1. Language / proof | Is the rewrite even legal? | types, escape, effects, identity (`src/semantic.rs`, `src/optimizer.rs`, `src/ir.rs`) |
| 2. Compilation world / public ABI | Would an application or unknown library consumer observe a different contract? | target, exports, `public_aggregate_abi`, `function_spelling`, `[mangle]` |
| 3. Explicit lowering intent | Did source request a live target operation/spelling? | IR `LoweringObligation` plus shadow-mode `OperationOrigin`/`NodeId` on each instruction (`src/ir.rs`). Parsed peephole still sees text, not those identities |
| 4. Unsafe preconditions | Which host facts did the build explicitly promise? | `assume_pristine_builtins`, `assume_pure_property_reads` |
| 5. Compression allowlist | May this representation exist at all? | `javascript.compression` / `priority` (`CompressionDecision`) |
| 6. Search feature / level | May alternatives be measured? | `javascript.optimizations` / `optimization_level` (`JavaScriptOptimization`) |
| 7. Codec canonical | What is the incumbent spelling for this `cost_model`? | `ProjectConfig::js_options()` |
| 8. Candidate search | Which legal opposite wins the complete artifact? | `CARTESIAN_EMISSION_AXES` then `SCORED_EMISSION_FAMILIES` in `src/decision_registry.rs`, applied by `extend_scored_emission_phase` |
| 9. Terminal rewrite | May parsed JS still change after IR search? | parsed peephole + late cleanup |
| 10. Unscored heuristic | Local “smaller/safer/faster” with no complete-artifact competitor | leftover defaults, search-off peephole, IR always-on passes |

In the goal architecture, layers 1–4 form the immutable compilation contract,
layers 5–9 are the compressor, and layer 10 is removed. Today those authorities
are not normalized into one object; this table records their scattered owners.
Codec scores must never compensate for a missing proof, ABI mismatch, unsafe
assumption, or source lowering obligation.

## How to read a varying behavior

For any choice such as “keep this `class` / dissolve it / emit `[x,y]` / emit
`{x,y}` / inline this function”:

1. Name the **proof** that makes each representation legal.
2. Name whether the **configured incumbent** is a codec-conditioned default or a
   size-agnostic ABI knob.
3. Name whether search **introduces**, **disables**, or **never sees** the opposite.
4. Name the **budget** that can starve the family (`candidate_proposal_limit`,
   `terminal_codec_probe_limit`, beam width, broad-module collapse).

If step 3 is “never sees,” the compiler is not seeking a global optimum for that
choice. It is applying a heuristic.

## Source intent and operation provenance

Each IR instruction now carries `OperationOrigin` (`Source` vs `Generated`) and
an optional source `NodeId` assigned at lowering (`src/lower.rs`). Source `x | 0`
is still `IrBinaryOp::BitOr` with
`LoweringObligation::PreserveJavaScriptBitOrZero`; `--explain` reports source vs
generated counts. Optimizer-inserted instructions are `Generated` with no node
id. `Span` remains a diagnostic location. Parsed peephole still reconstructs
bindings from text; carrying `NodeId` into that AST is 07.5.

The first required split is signed-i32 normalization:

| Operation | Desired classification | Search authority |
|---|---|---|
| Source-written live `x \| 0` | `PreserveOperation`, exact JS `\|0` spelling | None. Surrounding live code may optimize; the operation may not disappear or become `~~`. |
| `\|0` inserted for ordinary `int` arithmetic | generated normalization | May disappear after range proof or remain under a performance policy. |
| Unreachable/unused pure expression containing either | ordinary liveness | The whole dead computation may disappear. |

This requires stable `NodeId` and `OperationOrigin` data through lowering,
inlining, specialization, outlining, and peephole preparation. `Span` remains a
diagnostic location, not identity. A future typed JS lowering intrinsic uses the
same mechanism and declares an exact optimization envelope; raw JavaScript
strings and file-wide preservation pragmas are not alternatives.

The registry implementation must therefore classify every decision as one of:

| Class | Meaning |
|---|---|
| mandatory | language correctness or target normalization; no profitability branch |
| ABI | fixed by application/library boundary manifest |
| explicit lowering | fixed by source and carried as an IR obligation |
| unsafe precondition | fixed by explicit build contract; never inferred from size |
| incumbent | legal initial representation for this objective |
| scored family | two or more proof-legal alternatives compete |
| heuristic debt | an unscored profitability choice scheduled for migration |

## Aggregates: class, struct, object, record

This is the question “must I compile this class into `const`/`let`/`var`, an
array, a named object, or keep `class`?” Those are **four different
representations**, not one `aggregate_layout` knob.

| Representation | Legal when | How chosen today | Codec-scored? |
|---|---|---|---|
| SSA scalars (`const`/`let`/`var` field copies) | `LocalOnly` struct/class, `[optimization].scalar_replacement` | Incumbent IR pass. Size-first library configs admit `keep-object` (`scalar_replacement = false`) via `SCORED_IR_VARIANTS` when `joint-representation-search` is on. Root `lilscript.toml` omits that decision, so language tests do not clone keep-object. | **Yes**, when keep-object is admitted. Hard `[optimization].scalar_replacement = false` disables the pass entirely. |
| Positional array instance | Typed escape, identity of the **constructor** unobserved | `javascript.aggregate_layout = positional` (default) | Only if `joint-representation-search` is enabled **and** listed in compression. Root `lilscript.toml` **omits** that decision, so repo-default compiles do **not** compete array vs object. |
| Named object instance `{x,y}` | Same identity conditions | `aggregate_layout = named`, or joint search flipping `named_aggregate_fields` | Same gate as the row above. |
| `Foo$init` / `Foo$method` free functions | Constructor identity unobserved | Production IR emitter (`src/codegen_ir_js.rs`) | Default. ES `class` is **not** a competing family for these. |
| Function + `defineProperty` prototype table | Constructor identity observed | Ports reconstruct this in `.lil` when they need a JS constructor | Legal, large. Peephole may fuse it. |
| Named ES `class` | Constructor identity observed (`AggregateLayout.identity_observed`) | IR emit in `src/codegen_ir_js.rs`; `export constructor C` sets the mark; peephole still fuses port `defineProperty` tables | Named class is forced for a published constructor. Table vs class search is not a family because identity fixes legality. |
| `export class` as a JS constructor | Never, by lowering | `ExportBinding::TypeOnly` in `src/lower.rs` | N/A. Monaco/Motion/Zod export classes as instance types on purpose. |
| `Record<T>` null-prototype object | Open string keys | Always, if the record survives | Ordinary-object backing is **disabled** in production search (`ordinary_records_safe = false`). Closed-record **observation projection** is a JS-only IR candidate. |
| `extern class` | Host ABI by default | Existing host object, never constructed | Unset/true `mangle.extern_fields` preserves names; explicit `false` permits closed-world field mangling. Never dissolved. |

`public_aggregate_abi` is independent of instance backing: it decides whether a
**reusable JS boundary** exposes field names or an opaque array handle. Joint
search currently flips `named_aggregate_fields` (and keeps `public_aggregate_fields:
true` in both probes). It does not score “keep ES class vs dissolve” for
identity-free types.

Scalar replacement of a loop-carried `LocalOnly` struct is the narrow
phi-aware explosion documented in [aggregate lowering](aggregate-lowering.md).
Mutation, escape, shared phi inputs, or any non-field use retains the aggregate.
That is a proof, not a size heuristic. When explosion is legal, size-first
library search also scores **keeping** the `LocalOnly` object (`keep-object`).
Root TOML language tests omit `joint-representation-search` and stay single-clone.

## Functions: inline, share, or spell

Inlining is not monotonically smaller. jQuery is the measured counterexample.
The compiler has three different “inline” mechanisms and they are not one knob.

| Mechanism | Layer | Canonical | Opposite scored? |
|---|---|---|---|
| IR expression / small-CFG / single-use inline | `[optimization].inlining` + priority budgets (`size-first`: 12 / 30 / 16 instr/CFG/growth) | On for size-first / balanced | `ir-inlining-variants` adds a fully-off clone. Phase-order may also probe aggressive 48 / 128 / 40. Broad modules (>24 functions or >2048 IR ops) collapse phase-order to **one** combined probe. |
| Closure-factory inline | `inline_closure_factories` | On | `ir-closure-factory-variants` |
| Specialization / capture clones | `[optimization]` + JS features | On when gated | Off-clones when those features are on |
| Emission `inline_structured_closures` | compression `structured-closure-inlining` | On except performance-first | Initial Cartesian beam can disable |
| Emission `inline_single_use_functions` | same decision + search | **Off** in `js_options()` | Search may introduce `true` (script-only proof) |
| Emission `inline_exclusive_closures` | emitter | **On** | Search flips |
| Emission `pure_helper_inlining` | `pure-helper-inlining` | `None` | Cartesian `None` / `SingleStaticUse` / `AllEligible` × dense string tables |
| Emission `inline_fresh_empty_array_factories` | `fresh-literal-factory-inlining-variants` | Off | Late, at most two complete option sets |
| Region outlining | `[optimization].region_outlining` default **false** | Off because helpers often win raw and lose gzip/Brotli | `ir-compress-pass-variants` may add with/without when the compression decision allows |

There is **no** implemented rule “do not inline under `cost_model = raw`, inline
under Brotli.” What exists:

- IR inline **budgets** come from `javascript.priority`, not from `cost_model`.
- Search may emit a no-inline IR clone under size-first when `ir-inlining-variants`
  is legal.
- Several **emission** tactics that look like inlining are off in the incumbent
  and introduced only by search (`inline_single_use_functions`,
  `pure_helper_inlining`, fresh-array factories). Those can win Brotli while
  losing raw, or the reverse; the configured codec decides **if search reaches
  them**.
- Root `lilscript.toml` lists `ir-inlining-variants` and
  `structured-closure-inlining`, so those families are legal in repo compiles.
  It does **not** list `region-outlining`, so outlining cannot be reintroduced
  by compress-pass variants.

Closure representation is also missing from this registry as one coherent
family. The intended choices are inline, lifted direct function plus scalar
captures, native lexical closure, or explicit positional/named environment.
Legality depends on capture mutation, identity, escape, recursion,
`name`/`length`/constructibility, and host use. Profitability must be scored with
the call-graph/inlining family rather than decided independently by IR, emitter,
and parsed-JS inliners.

## Codec-conditioned incumbents

`js_options()` hard-wires several defaults to `cost_model` **before** search.
Some opposites are later flipped; some are not.

| Field | `raw` / `gzip` incumbent | `brotli` incumbent | Search sees the opposite? |
|---|---|---|---|
| `function_spelling` (when unset) | `Arrow` | `Function` | Yes, if TOML left it unset. Explicit `"arrow"` / `"function"` freezes ABI. |
| `local_phi_expression_regions` | on (level ≥ 4) | **off** | Yes, when `local-phi-expression-region-variants` is on. Comment in `JavaScriptConfig`: forcing on is jQuery −87 Brotli and losses on marked/zod/mobx/monaco/posthog. |
| `phi_edge_value_forwarding` | on (level ≥ 4) | **off** | Yes, when that feature is on. |
| `pack_string_arrays` | on if compression allows | **forced false** | Cartesian axis `string-array-packing` uses `reversible_boolean_alternatives`. When the compression decision is legal, Brotli can re-enable packing. An exact list that omits the name does not. |
| `elide_length_tonumber` | on if `length-to-number-elision` (size-first canonical) | same | Sequential family `length-to-number-elision`, gated by `js_length_to_number_elision_variants_enabled`. Omitting the name keeps the spelling off. |
| `pool_identifier_strings` | true | **false** | Cartesian axis `identifier-string-pooling` when string pooling is configured and search-legal. |
| `alias_array_prototype_methods` | true | **false** | Yes. |
| `string_pool_minimum_savings` | 1 / 4 / 8 | 8 | Search also tries 16, 64, 128, 256, 512 with pooling on. |

`|0` elision is not a codec search: size-first and balanced drop proven-redundant
coercions because `|0` never helps transfer. Performance priorities keep it.
`javascript.integer_coercions = true` overrides.

The objective behavior above now applies only to compiler-generated
normalization. A source-written live `x | 0` carries an explicit lowering
obligation outside objective search; dead enclosing code may still disappear.

`function_spelling` also needs to split during migration. Public callable
kind/constructibility belongs to the ABI manifest and cannot vary by codec;
private `function` versus arrow syntax remains a scored spelling. A current
explicit setting may conservatively pin both while configs migrate.

These codec defaults are the honest answer to “raw vs Brotli changes
compilation behavior.” They are **priors**, not a proof that the incumbent is
best. A prior that search cannot reverse is a heuristic.

## `javascript.priority` vs `cost_model`

They are not substitutes.

| Axis | Selects |
|---|---|
| `cost_model` | What “smaller” **means** (raw bytes vs gzip-9 vs Brotli-11) and several canonical spellings above |
| `priority` | Default compression set, inline budgets, and how transfer trades against the static performance model |

| Priority | Rank | Typical compression set |
|---|---|---|
| `size-first` | exact transfer, performance only on a transfer tie | broadest: packing, property mangling, IR variants, joint search, outlining legality, … |
| `balanced` | `3*transfer + 2*shape` | superopt, path-sensitive, loop spelling; **no** packing / property mangling / joint search / IR inlining variants |
| `realistic-performance-first` | over-limit bucket + transfer ratio, then shape | like balanced on many tactics |
| `performance-first` | shape first | identifier mangling + grammar elision + a few small tactics; no pooling/packing/IR search variants |

An exact `compression = [...]` **replaces** the priority’s canonical list for
named decisions. Root `lilscript.toml` is a **subset**: it enables
`host-alias-spelling` / `pure-helper-inlining` / `dense-string-return-tables`
that a bare size-first profile would also want, and it **omits**
`property-mangling`, `joint-representation-search`, `joint-chunk-symbol-search`,
`region-outlining`, `array-pipeline-fusion`, `partial-escape-sinking`,
`regex-literals`, `length-to-number-elision`, `indexed-char-at`, `effect-ternary`,
and several others.
Omitted names disable the **canonical** tactic. Search may still introduce
size-first **search-only** spellings such as `indexed-char-at` unless the list
is `[]` (`search_compression_enabled` in `src/config.rs`). Most non-search-only
omitted names stay off. `length-to-number-elision` is now a registry family:
omitting it does not flip `elide_length_tonumber`.

`export-mangling` is never implied by priority.

## IR optimizer variants (level 1 search)

Always first: `config.js_optimizer_options()`. Simple off-clones come from
`SCORED_IR_VARIANTS` in `src/decision_registry.rs` (`scored_ir_optimizer_clones`).
Phase-order and compress-pass probes stay in `compiler.rs`. Each clone is
re-optimized and emitted with **configured** `js_options()`:

| Variant | Registry name / gate |
|---|---|
| `inline_closure_factories = false` | `closure-factory-outlining` / `ir-closure-factory-variants` |
| inlining fully off | `ir-inlining-off` / `ir-inlining-variants` |
| scalar-replacement off | `keep-object` / `js_keep_object_variants_enabled` (`joint-representation-search`) |
| no constant-parameter specialization | `ir-specialization-off` / `ir-specialization-variants` |
| no call-site specialization | `call-site-specialization-off` |
| reusable helpers (inline off + specialization off) | `call-graph-reusable-helpers` when both families exist |
| no capture-signature cloning | `capture-signature-cloning-off` |
| function subsumption on and off | `function-subsumption-on` / `function-subsumption-off` |
| phase-order: no early CSE, aggressive inline, both | `ir-phase-ordering-variants`; **broad modules → one combined probe** |
| compress passes all-off; outlining contrast; fusion/merging off | `ir-compress-pass-variants` |
| strongest bounded aggressive-inline × outlining | reserved 2nd/3rd slots |

Not an IR variant today: DCE off, escape analysis off, or “keep this class as ES
class.” Those remain proofs or later emission/peephole (07.4). `--explain`
lists admitted `SCORED_IR_VARIANTS` names as `scored ir variants`.

## Emission families (level 2 search)

`select_javascript_candidate_global` first expands `CARTESIAN_EMISSION_AXES`,
then `extend_scored_emission_phase` walks `SCORED_EMISSION_FAMILIES` (entropy
alphabet search stays a named special case between the before/after-entropy
phases). Each sequential family starts from the current beam’s top
`candidate_beam_width` finalists (default 12). An early winner can starve a late
family when the proposal ledger is exhausted. That is why
[search-03](../migration/board/notes/search-03.md) had to lift the pack config:
an 18 KiB module at level 15 with production search got 96 work units against
~38 beam families. `--explain` lists branching cartesian axes, admitted scored families, and
admitted IR variant names for that compile.

Families that **are** flipped (the registry tables are the authority; names
below match `src/decision_registry.rs`):

precise cross-scope shadowing; frequency-order local names; `stable_local_names`
off; `struct_method_shorthand`; `elide_length_tonumber`; `pool_window_roots`;
`alias_array_prototype_methods`; `inline_exclusive_closures`;
`iife_private_callee_clusters`; `nested_once_run_helpers` off;
string-pool thresholds; `batch_property_assigns` at minima 2/3/4/6/8;
`constructor_initializer_fusion`; `host_alias_spelling = Direct`;
`indexed_char_at`; `effect_ternary` off; independent grammar-elision punctuations;
`function_spelling` toggle; `inline_single_use_functions`; pure-helper × dense
tables; callee defaults; `truthy_nullable_checks`; conditional / phi-region /
phi-edge / operand-order / comma / loop / mutation / SSA-destruction /
control-flow / switch / function-layout / joint named-vs-positional /
property-mangling / entropy alphabets.

Families that **are not** flipped in production search:

| Field / choice | Why it stays |
|---|---|
| `ordinary_record_literals` | `ordinary_records_safe = false`; cartesian axis exists but does not branch |
| `bare_window_root` | Illegal to flip; would change Node/worker behavior |
| `ecmascript` | Search never raises the syntax floor |
| ES class vs dissolved object for identity-**free** types | Not an `IrJsOptions` flag |

`struct_method_shorthand` **is** searched. It used to be a compact-is-better
default; jQuery measured turning it off as −404 raw **and** −94 Brotli. That is
the template for every remaining unscoped prior.

## Parsed JavaScript (layer 9)

Three different applications, documented honestly in [peephole](peephole.md):

1. **Search-on terminal leaves** — peephole clones compete under the codec;
   late cleanup is a per-pass beam that also scores skip-the-rewrite.
2. **Canonical rewrite of the winner** — `apply_selected_canonical_peephole`
   runs only when terminal work remains and competes under full priority/startup
   ranking.
3. **Search-off** — `apply_search_off_declaration_peephole` exactly scores one
   configured function-preserving challenger against the untouched emit.

`src/js_peephole/folds/classes.rs` still repairs user-space prototype tables and
legacy identity shapes. Proof-marked `AggregateLayout.identity_observed` classes
emit named `class` directly from IR, and `export constructor C [as PublicC];`
provides the source-level proof without changing type-only `export class`.

## Compile time is part of the decision

Unbounded Brotli-11 on the cross-product of `IrJsOptions` is unused. The
implemented compromise:

- `optimization_level` 0–15 feature gates **and** effort caps
- `candidate_search`: `off` / `production` (384) / `always`
- `candidate_limit`, `candidate_byte_budget` (default 1 MiB), `candidate_beam_width` (12)
- `candidate_proposal_limit` and `terminal_codec_probe_limit` (artifact-scaled:
  full through 16 KiB, ÷4 to 64 KiB, ÷12 above)
- Broad-module IR phase-order collapse
- Sequential family expansion rather than a joint optimizer

The result is the best artifact **found under those budgets**, not a mathematical
global minimum. Explain metrics report exhaustion. A starved family is a missed
optimum, not evidence the incumbent was best.

## Config surface vs emitter surface

Policy is duplicated across:

- `CompressionDecision` / `JavaScriptOptimization` / `JavaScriptPriority` in `src/config.rs`
- `IrJsOptions` (77 fields) in `src/codegen_ir_js.rs`, each classified in
  `IR_JS_OPTION_FIELDS`
- `OptimizationOptions` in `src/optimizer.rs`
- Scored emission families and cartesian axes in `src/decision_registry.rs`

Adding a scored emission tactic is a registry row plus the existing
`extend_javascript_candidate_beam` construction site. ABI, unsafe, and
illegal-to-flip fields must not appear as family names. The migration in
[07](../migration/07-global-compressor.md) still has to finish reversible
priors, IR class emit, scored peephole, reserved slices, and language cases.

The goal code registry will consume two separate normalized values:

1. `CompilationContract`: application/library world, public/host ABI, explicit
   operation obligations, target floor, and unsafe assumptions.
2. `OptimizationObjective`: raw/gzip/Brotli metric, size/performance priority,
   enabled decision families, and compile-time budgets.

`IrJsOptions` is then an emission plan produced from both, not the place where
ABI and profitability policy are mixed. Objective changes may alter internals;
they must pass the same ABI manifest and explicit-operation checks.
