# arch-03 — 07.3 reversible codec priors

Parent: [ledger](../LEDGER.md). Status: landed. Plan:
[07.3](../../07-global-compressor.md#073--reversible-priors). Identity:
[ident-03](ident-03.md) has landed; 07.2 has landed.

## Question

Which compact-is-better defaults are priors for a `cost_model`, and which are
one-way doors search cannot reopen?

## Current hypothesis

Codec-conditioned incumbents stay. Search proposes the opposite when the
compression decision is legal: Brotli packing and identifier-string pooling
re-enable on cartesian axes; scalar-replacement off is the `keep-object` IR
clone. Inlining, specialization, and reusable-helper opposites live in one
`SCORED_IR_VARIANTS` table. Phase-order and compress-pass probes stay in
`compiler.rs`. Full call-graph cartesian is 07.6.

## Constraints specific to this task

- Do not start while ident-05 is red. 07.2–07.3 may overlap once identity is trusted.
- Measure on more than jQuery. `struct_method_shorthand` is the successful template.
- Do not delete a prior because it lost once.
- Do not silently enable keep-object on the root TOML language-test subset.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | architecture audit | `rg pool_identifier_strings pack_string_arrays src/compiler.rs src/config.rs` | pooling never flipped; packing Cartesian cannot re-enable under Brotli; no scalar-replace IR off-clone | diag |
| 2026-08-28 | packing/pooling reopen on size-first Brotli | `/Users/yeargun/.cargo/bin/cargo test --lib --offline cartesian_seed_keeps_the_configured_incumbent` | pass; seeds include `pack_string_arrays` true and `pool_identifier_strings` true | gate |
| 2026-08-28 | keep-object admitted on size-first, not on exact identifier-mangling | `/Users/yeargun/.cargo/bin/cargo test --lib --offline scored_ir_variants_are_named_uniquely_and_keep_object` | pass | gate |
| 2026-08-28 | dedicated scalar-replace on/off ablation | `/Users/yeargun/.cargo/bin/cargo test --lib --offline scalar_replacement_on_and_keep_object_are_both_legal` | pass; on-clone reports `scalar-replacement-cfg` changed and drops `Struct`; off-clone keeps `Struct`; both emit | gate |
| 2026-08-28 | explain dump names keep-object | `/Users/yeargun/.cargo/bin/cargo test --lib --offline explained_compilation_reports_selection_costs` | pass; `ir_variants_searched` contains `keep-object` | gate |
| 2026-08-28 | call-graph reusable-helper clone from registry | `/Users/yeargun/.cargo/bin/cargo test --lib --offline optimizer_search_includes_the_reusable_helper_corner` | pass | gate |
| 2026-08-28 | size-first vs performance keep-object gate | `/Users/yeargun/.cargo/bin/cargo test --lib --offline maps_javascript_priorities_to_concrete_policies` | pass | gate |

## Log

- 2026-08-28 — Scheduled as 07.3. — **OPEN**
- 2026-08-28 — `SCORED_IR_VARIANTS` owns keep-object, inlining-off, specialization-off, and reusable-helper clones. Cartesian packing/pooling already reversible from 07.2; tests now require a true seed under size-first Brotli. Dedicated ablation is not md-01. Uncommitted. — **LANDED**

## Next step

Proceed to [arch-04](arch-04.md): IR named `class` for identity-observed constructors, with joint layout reachable from size-first library configs.
