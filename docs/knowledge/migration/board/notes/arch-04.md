# arch-04 — 07.4 IR emits legal shapes including named class

Parent: [ledger](../LEDGER.md). Status: landed. Plan:
[07.4](../../07-global-compressor.md#074--ir-emits-legal-shapes). Language:
[class identity](../../../compilation/class-identity.md).

## Question

Can identity-observed constructors emit named ES `class` from
`codegen_ir_js.rs`, while identity-free `class-scale` still dissolves? Can
closure capture environments and owned properties use proof-legal scalar,
positional, named, or lexical forms instead of one global shape?

## Current hypothesis

IR emit produces named `class` when `AggregateLayout.identity_observed` is set.
07.7's `export constructor C [as PublicC];` now supplies that proof and protects
the public method table; `export class` stays type-only. Identity-free
`class-scale` still dissolves. Closure capture slots and owned-property
`(owner, slot)` identity now drives naming. Closure candidates choose lexical
capture or lifted immutable scalar snapshots; mutable cells remain lexical.

## Constraints specific to this task

- Do not start while ident-05 is red.
- 07.2–07.3 may overlap now that identity is trusted.
- Do not emit `class` for identity-free types.
- Add an IR/emitter unit fixture for a synthetic identity-required constructor.
  The public `canonical/aggregates/exported-class-identity` case waits for the
  arch-07 constructor-value syntax.
- Prefer deleting port identity emulation over teaching the peephole more tables.
- Property identity is `(owner, slot)`, not a trailing-underscore spelling.
- Public callable name/length/constructibility and descriptors come from the
  ABI manifest; private closure capture slots remain mangleable.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | architecture audit | `lower.rs` `Item::Class` | `ExportBinding::TypeOnly`; IR named-class emit is still the class-identity plan | diag |
| 2026-08-28 | proof-marked constructor emits named class | `/Users/yeargun/.cargo/bin/cargo test --lib --offline identity_observed_constructor_emits_named_class` | pass; `class Scale` + `new Scale(` + `108\\n3`; no `$init` | gate |
| 2026-08-28 | identity-free Scale still dissolves | `/Users/yeargun/.cargo/bin/cargo test --lib --offline dissolves_internal_scale_class_without_es_class` | pass; no `class ` | gate |
| 2026-08-28 | exported identity-free position classes dissolve | `/Users/yeargun/.cargo/bin/cargo test --lib --offline dissolves_monaco_style_exported_position_classes` | pass | gate |
| 2026-08-28 | explicit public constructor reaches named-class IR emit | `cargo test --release --lib explicit_constructor_export_preserves_named_class_identity` | pass; name/length/new/method runtime parity, aliased ESM export | gate |
| 2026-08-28 | public constructor hierarchy | `cargo test --release --lib constructor_export_preserves_explicit_inheritance` | pass; base-first class emit, extends/super, instanceof, inherited and own methods | gate |
| 2026-08-28 | published default constructor | `cargo test --release --lib constructor_export_synthesizes_only_the_published_default_constructor` | pass; zero arity, identity-free sibling still dissolves | gate |
| 2026-08-28 | published field defaults | same gate plus `permits_public_class_field_initializers` | int/bool/string/array defaults survive; generated-JS validator accepts legal class fields and still rejects fused invalid elements | gate |
| 2026-08-28 | owner/slot property naming | `cargo test --release --lib property_mangling`; `unrelated_owned_property_components_reuse_short_names`; `extern_property_spelling_does_not_pin_an_unrelated_owned_slot` | inheritance components remain collision-free; unrelated owners reuse short keys; external spelling does not pin private slots | gate |
| 2026-08-28 | closure environment alternatives | `cargo test --release --lib immutable_closure_captures_can_use_lifted_scalar_snapshots`; `mutable_closure_captures_remain_shared_lexical_cells`; `decision_registry` | lexical/lifted scalar choices are scored; mutable siblings retain shared cells | gate |

## Log

- 2026-08-28 — Scheduled as 07.4. — **OPEN**
- 2026-08-28 — IR named-class emit for `identity_observed` layouts. Inlining and scalar-replace skip those constructors/methods. `export class` stays TypeOnly. Closure/property SCC alternatives remain. Uncommitted. — **OPEN**
- 2026-08-28 — Constructor-value syntax now connects the public ABI to the proof-marked class representation. Closure/property alternatives remain. — **OPEN**
- 2026-08-28 — Owned property names now key on canonical `(owner, slot)` identity;
  spelling-only trailing-underscore mangling was removed. Closure environment
  materialization remains. — **OPEN**
- 2026-08-28 — Added the lifted immutable-snapshot closure representation and
  registry family; mutable cells remain lexical by proof. — **LANDED**

## Next step

Continue adding representation families only from measured corpus losses;
explicit object environments are not emitted without a winning case.
Contract:
[size-first libraries](../../07-global-compressor.md#size-first-library-contract).
