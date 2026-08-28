# arch-02 — 07.2 one decision registry in code

Parent: [ledger](../LEDGER.md). Status: landed. Plan:
[07.2](../../07-global-compressor.md#072--one-registry). Identity:
[ident-05](ident-05.md) and [ident-03](ident-03.md) have landed.

## Question

Can a reviewer answer “is layout / packing / class-identity searched in this
compile?” from one table and one explain dump, without reading an imperative
list of `extend_javascript_candidate_beam` closures? Can the same dump distinguish
a public name and source-authored operation from profitability choices?

## Current hypothesis

The prose registry
([decision-registry](../../../compilation/decision-registry.md)) is now owned in
`src/decision_registry.rs`: every `IrJsOptions` field has a class, cartesian
axes and sequential families are tables, and `extend_scored_emission_phase`
iterates them. ABI/unsafe/illegal fields are not family names. Source
instructions carry `NodeId` + `OperationOrigin::Source`; optimizer inserts are
`Generated`. `--explain` lists layout search, removed size-first families,
cartesian axes, scored families, and source/generated counts.

Remaining 07 work is 07.3–07.7: reversible priors (keep-object / scalar
replacement off-clone), IR named class, scored peephole, reserved slices,
constructor-value cases. Entropy alphabet search is still a named special case
between the two scored phases, not a family row.

## Constraints specific to this task

- Do not add families. Reclassify the existing `IrJsOptions` fields.
- Dual-gated features stay dual-gated, in one place.
- ABI, unsafe assumptions, and explicit lowering obligations cannot become
  scored alternatives.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | architecture audit | read `compiler.rs` / `config.rs` | 74 `IrJsOptions` fields; packing Cartesian `[configured, false]`; `elide_length_tonumber` unguarded at `:3748` | diag |
| 2026-08-28 | Exact list removes length-to-number search | `cargo test --lib applies_an_exact_custom_compression_decision_set` | omitting `length-to-number-elision` keeps baseline off and does not flip the beam; `removed_size_first_compression_families` names `length-to-number-elision` and `joint-representation-search` | gate |
| 2026-08-28 | Explain reports layout and removals | `cargo test --lib explained_compilation_reports_selection_costs` | default size-first: `layout_searched` true, removed families empty | gate |
| 2026-08-28 | Registry owns beam + provenance | `cargo test --lib classified_once`; `scored_emission_families`; `omitting_length_to_number`; `cartesian_seed_keeps`; `explained_compilation_reports_selection_costs`; `lowered_source_operations_carry_node_ids` | 75 fields classified; 45 scored families; omitted length-to-number not admitted; packing cartesian branches on size-first; explain lists `named-aggregate-layout` and `string-array-packing`; source `|0` has `NodeId` + `PreserveJavaScriptBitOrZero` | gate |

## Log

- 2026-08-28 — Scheduled as 07.2. No compiler work until ident-05. — **OPEN**
- 2026-08-28 — ident-03/04 landed. 07.2 is unblocked. — **OPEN**
- 2026-08-28 — Beam no longer flips `elide_length_tonumber` unless
  `length-to-number-elision` is in the compression matrix. `--explain`
  prints `layout searched` and `compression families removed` versus the
  size-first set. Registry gained that scored row. Remaining: classify every
  `IrJsOptions` field, iterate the scored set instead of ~40 closures, and
  shadow-mode `NodeId`/operation origin. — **OPEN**
- 2026-08-28 — Field table, cartesian/scored iterators, explain dump, and
  shadow-mode origin landed. — **LANDED**

## Next step

07.3 (`arch-03`): reversible packing/pooling priors are cartesian-legal when
the compression decision is on; still missing a scored keep-object /
scalar-replacement off-clone. Do not flip committed port toml (search-02).
