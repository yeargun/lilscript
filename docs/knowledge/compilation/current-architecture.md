# Current compiler architecture

Authority: source and tests. Status snapshot:
[`docs/current-status.md`](../../current-status.md). Parent:
[compilation](README.md). Target: [planned architecture](planned-architecture.md).

This page describes implemented behavior in the 2026-08-29 checkout. It does
not turn current limitations into design goals.

## System Shape

```text
source graph + config
  -> parse and link
  -> semantic analysis
  -> typed CFG/SSA lowering with operation provenance
  -> JavaScript optimization/selection
       configured IR incumbent
       scored IR variants
       emission options and scored families
       generated-JS contraction/naming challengers
       exact complete-artifact ranking
  -> JavaScript artifact or chunk set

lowered IR clone
  -> native-specific optimization
  -> C/native artifact for the portable subset
```

`src/codegen_js.rs` is a facade into the IR path; new production optimization
belongs in typed IR, `src/codegen_ir_js.rs`, the decision registry, or the
target-side migration layer.

## Contract And Objective

`src/compilation_contract.rs` defines:

- `JavaScriptCompilationContract` for world, syntax target, ABI policy, unsafe
  assumptions, and effect-removal policy;
- `JavaScriptOptimizationObjective` for transfer metric and priority;
- `JavaScriptAbiManifest` derived from typed IR.

`src/compiler.rs` compares the pre-selection and selected-IR manifests. This
prevents some optimizer drift, but it is not a complete emitted-ABI gate:

- the manifest is source/IR-derived rather than extracted from final artifacts;
- fields are not owner-qualified and ordered for every public boundary;
- ESM live bindings, lazy/module identity, descriptors, emitted export spelling,
  and all host interactions are not fully represented;
- single, split, and preserve-module paths do not yet share one contract and
  observed-artifact validation path;
- compilation world is still coupled too closely to module output.

These are active correctness boundaries. The planned design separates world,
artifact format, and boundary roots and compares expected ABI with an observed
artifact witness.

## Typed IR And Proofs

`src/ir.rs` and `src/lower.rs` retain source/generated operation origin and
optional `NodeId`. A live source `value | 0` carries
`PreserveJavaScriptBitOrZero`; generated integer normalization remains
optimizable. Current provenance is function-local and broad peephole skipping is
still used when an obligation survives, so this is a first contract, not a
general witness system.

`src/semantic.rs`, `src/optimizer.rs`, and `src/value_analysis.rs` provide
conservative type, escape, use, range, alias, identity, and effect facts. Several
effect and observability queries remain duplicated between optimizer and
emitter. `EscapeState` alone is not a proof that identity is unobservable;
transforms also inspect uses and boundaries.

Implemented representation work includes:

- scalar replacement with a scored `keep-object` IR alternative when admitted;
- positional/named aggregate emission and proof-marked named classes;
- `export constructor C [as PublicC]` distinct from type-only `export class`;
- owner/slot identity on typed fields and optional owner-scoped property naming;
- lexical mutable captures and scored immutable scalar snapshots;
- expression `if`, scalar literal `match`, and ordinary `object{...}`.

## Decision Registry

`src/decision_registry.rs` is both a census and scheduler:

- all 77 `IrJsOptions` fields are classified;
- 48 scored emission families are named and gated;
- scored IR variants include reversible priors and `keep-object`;
- ABI, unsafe, and illegal fields are excluded from scored axes.

This is not yet a complete declarative proof engine. Only a small subset uses the
new `DecisionSpec` form. Phase-order/compress probes, entropy/naming work, target
contractions, validators, and some candidate construction remain specialized in
`src/compiler.rs`. The 77-field exhaustiveness check is based on a maintained
list rather than a type-level generated schema.

The registry should remain a compact census and recipe owner. Typed
materializers should return a candidate plus proof witness or a rejection
reason; a generic proof-query DSL is not required.

## Search And Scoring

The configured IR/emission artifact is retained. Search adds proof/config-gated
IR variants, Cartesian seed axes, sequential emission families, naming/entropy
alternatives, and terminal generated-JS challengers. It uses deterministic work
budgets, family reserves, exact codec scores, startup/performance guards, and
starvation reporting.

Important limitations:

- production search is a bounded portfolio, not exhaustive;
- family order and beam retention affect which interactions are reached;
- larger beam/limits are not guaranteed to consume a strict prefix of smaller
  work and have measured regressions under fixed budgets;
- current frontier diversity computes exact raw/gzip/Brotli costs for more
  intermediate candidates than the selected metric alone requires;
- some validation occurs only when finalists are prepared, so rejected shapes
  can consume earlier beam work;
- no stable serialized recipe can replay a previous compiler winner directly;
- split planning uses a separate greedy mixed deployment-cost path and
  preserve-modules has a narrower fixed path.

Normal explain output reports best-observed search and starvation, but does not
yet serialize every option, parent recipe, rejection, and evidence fingerprint
needed for exact regression replay.

## JavaScript Target Layer

`src/codegen_ir_js.rs` emits strings with local expression metadata.
`src/js_peephole/` tokenizes/parses generated JavaScript for binding-aware folds,
naming, class/prototype contraction, control/loop contraction, and final
validation. Search-on, canonical-winner, and search-off challengers are now
scored against an incumbent.

This layer has prevented real miscompiles, but semantic identity is still being
reconstructed from generated text. The parser is targeted at compiler output,
not a standards-complete ECMAScript frontend. It cannot prove general semantic
equivalence, and it does not yet provide a complete target-syntax/ABI witness.

The active architecture task is a narrow hygienic emission IR carrying binding,
external/global, property, function/call, allocation, effect-order, module,
syntax-floor, and lowering-obligation identities. The existing parser remains an
independent final-byte checker during migration.

## Boundaries And Mangling

Private owned properties may be mangled or owner-scoped. Public manifest fields,
dynamic/reflected keys, records, and host fields require exact treatment unless
an explicit coordinated closed-world ABI owns both producer and consumer.

The current `mangle.extern_fields = false` mode and trailing-underscore key
convention predate that distinction. They are legacy closed-world mechanisms,
not proof that an arbitrary host `extern` field is safe to rename. The planned
architecture replaces them with typed ownership or an explicit foreign ABI map;
ordinary host names remain exact.

## Verification

- Rust unit/integration/all-target tests own compiler behavior.
- The differential evaluator checks a portable semantic subset independently of
  CFG/SSA optimization.
- Canonical paired cases compare compiler output with independently authored JS
  under matching raw/gzip/Brotli objectives.
- The codec contract pins encoder behavior.
- External library suites check scoped API/behavior boundaries.
- Large-library evidence tooling exists but does not yet capture every current
  artifact, explain recipe, resource metric, or deployment boundary needed for
  the planned regression workflow.

Current counts and mixed size results live only in
[`docs/current-status.md`](../../current-status.md).

## Primary Gaps

1. Freeze expected ABI independently of artifact format and validate observed
   final artifact/module-set ABI.
2. Validate syntax/bindings/properties/ABI/obligations before exact scoring;
   behavioral semantics remain test-suite evidence.
3. Serialize complete recipes and make evidence replayable before restoring old
   size incumbents.
4. Consolidate candidate ownership and acceptance without inventing a universal
   choice graph or proof DSL.
5. Introduce the minimal hygienic target-JS representation and retire text
   identity recovery family by family.
6. Recover current Motion/Marked/MobX regressions, then close maintained library
   gaps with generic proofs and measured interactions.
7. Defer unified chunk optimization until a maintained chunk workload defines a
   calibrated delivery objective.

Execution order: [planned migration](../migration/planned-migration.md).
Rationale: [design decisions](../decisions/README.md).
