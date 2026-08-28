# arch-06 — 07.6 search that can finish

Parent: [ledger](../LEDGER.md). Status: landed. Plan:
[07.6](../../07-global-compressor.md#076--search-that-can-finish). Math:
[objectives](../../../compilation/objectives.md).

## Question

Can late families (layout, class-identity, naming) and measured non-monotone
pairs consume reserved budget instead of starving after punctuation families?
Can raw/gzip/Brotli each select a different legal decision vector without ever
changing the contract?

## Current hypothesis

Proposal budget remains artifact-scaled. The registry now marks layout and
naming families `Priority`, the coordinator reserves one third of structural
work across admitted priority families, and explain names families truncated by
the ledger. The measured `function_spelling` × `stable_local_names` pair is one
declared joint family. A full Cartesian is not used.

## Constraints specific to this task

- Blocked on arch-02 (registry iterator). Reserved slices can be designed now,
  not landed as more imperative closures.
- Explain output must name starved families.
- Do not replace sequential search with a full cross-product.
- Cheap models order work only. Final size-first selection uses the exact
  configured codec on a parse- and binding-valid complete artifact.
- Define bundle composition: selected codec per chunk plus request/depth/cache
  terms; report legacy mixed-codec `[bundle.cost]` explicitly instead of
  silently replacing `javascript.cost_model`.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-25 | search-03 | `--explain human` | 18 KiB module at level 15 gets 96 units; naming/class-shape never reached | diag |
| 2026-08-25 | jquery-01 pair | `lilscript-codec --json` | `function_spelling` × `stable_local_names` jointly −6 on callbacks; −106 across six modules; −11 on the full artifact | gate |
| 2026-08-28 | protected registry slices | `cargo test --release --lib scored_emission_families_are_named_uniquely_and_skip_illegal_axes` | layout/local-reserve/stable-name families are priority; measured spelling×stable pair is declared | gate |
| 2026-08-28 | starvation is observable | `cargo test --release --lib zero_structural_proposal_budget_skips_optional_emission_before_codegen` | pass; explain metrics name starved emission families | gate |
| 2026-08-28 | deep deterministic release tier | `cargo test --release --lib brotli_objective_carries_async_literal_movement_into_name_search`; canonical runner | always tier reaches naming+movement winner; canonical 52/52 | gate |
| 2026-08-28 | explicit bundle objective | `cargo test --release --lib preserves_surviving_dependency_functions_as_esm_chunks` | manifest records selected codec bytes, deployment weights, penalties/discounts, and deterministic objective fingerprint | gate |

## Log

- 2026-08-28 — Scheduled as 07.6. search-03 finding stands; compiler default unchanged. — **OPEN**
- 2026-08-28 — Priority slices, starvation reporting, one measured joint family,
  and a deeper `always` terminal tier landed. Bundle composition remains. — **OPEN**
- 2026-08-28 — Bundle selected-codec totals and the separate deployment-cost
  formula are fingerprinted in the manifest. — **LANDED**

## Next step

Continue measured family additions through the registry; do not reintroduce
pack-local budget lifts. Contract:
[size-first libraries](../../07-global-compressor.md#size-first-library-contract).
