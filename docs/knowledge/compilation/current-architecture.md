# Current compiler source map

Parent: [compilation](README.md). This map identifies the implementation at `8b3d5cc5`, the starting point of the 2026-09-29 replan. Source and tests determine actual behavior; [migration progress](../../migration/index.md) tracks later changes. The [active plan](../../migration/plan.md) describes the target architecture and owns remaining work.

## Pipeline

```text
source graph + lilscript.toml
  -> policy resolution, parsing and checking
  -> typed Program IR and shared facts
  -> program rules
  -> JavaScript formation, target rules, delivery, naming and printing
  -> admitted candidates, requested-codec judging and publication

  -> native formation and C/runtime output for supported features
```

| Area | Source entry points | Remaining work |
|---|---|---|
| Configuration | `src/config.rs`, `src/compilation_policy.rs`, `src/compilation_contract.rs` | C1: permissions/registry/compatibility; D3: several objectives per request |
| Public entry and checking | `src/build.rs`, `src/check.rs`, `src/module.rs` | S4: remaining identities/contracts; D3: shared graph across clients |
| Program and facts | `src/program/`, including effects, values, uses and demand | S1/S2/S4: additional consumers, escape/field facts, removal of duplicate analyses |
| Program transformations | `src/program/rules/` | S1/S2/S3: folding, scalar replacement, inlining and related cleanup |
| JavaScript formation and rule scheduling | `src/program/javascript.rs`, `src/js/rules.rs` | Q2: incremental work; G3/S4: consume facts and finish target-rule migration |
| Naming and spelling | `src/js/naming.rs`, `src/js/families.rs`, `src/js/print.rs` | G1/G2/G3: richer naming, private fields and per-site choices |
| Candidate lifecycle and search | `src/program/publication.rs`, `src/program/search.rs` and related modules | Q1/Q2/Q3: one family mechanism, reuse and independent calibrated policies |
| Native | `src/program/native.rs` and related modules | N1/N2: common optimized input, toolchain, capabilities and remaining runtime support |

## Important current limits

- TOML accepts one selected codec per build. `[policy.tactics]` exists, but the checklist still carries permission holes for terminal pooling/packing.
- Raw/gzip/Brotli have their own exact objective scoring; gzip and Brotli still share their output-family seed.
- A scoped naming allocator and several naming/spelling alternatives exist. That does not complete the planned private-property mangler or richer coupled naming policy.
- Structural search and terminal choices still have mechanisms to consolidate. The markedlil trace demonstrates repeated formation/judging work; exact score equality alone does not establish identical candidate state.
- Program folding and shared facts are active. The old snapshot's claim that the program-rule layer is empty is obsolete.
- Runtime tests establish correctness on their workloads. The dedicated runtime parity runner and full performance qualification remain open.

For implementation details, read the owning source and its tests, then the relevant [work package](../../migration/plan.md#7-work-packages-and-dependencies). The old stage-by-stage description is [archived](../../old-history/knowledge/compilation/current-architecture.md).
