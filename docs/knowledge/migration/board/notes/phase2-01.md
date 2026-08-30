# phase2-01 — consolidate candidate acceptance

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Can imperative IR/emission/terminal candidate paths use one registry-owned
materialize, validate, exact-score, guard, and compare primitive without changing
the reachable artifact set?

## Current hypothesis

The final-artifact admission contract supplies the shared validation layer. The
smallest next consolidation is to inventory and route duplicate score/accept
sites through one existing candidate abstraction before adding any new choices.

## Constraints specific to this task

No new optimizer alternatives, config DSL, solver, or byte changes are allowed
in a consolidation batch. The configured incumbent remains explicit and every
existing candidate identity stays reachable.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Phase-1 recovery review | `recover-01..04` | MobX and Motion exact direct boundaries reach or beat reproducible incumbents; Marked current is below committed artifacts; jQuery exact migration pair ties | gate |
| 2026-08-29 | Shared syntax/binding score boundary | `cargo check --lib`; `cargo test --lib generated_javascript_admission_rejects_invalid_code_before_codec` | all internal generated-text codec routes use `AdmittedGeneratedJavaScript`; invalid bytes do not increment the codec counter | gate |
| 2026-08-29 | Shared ordering key | `cargo check --lib`; `cargo test --lib parallel_brotli_finalizer_preserves_exact_tie_breaking` | scored and finalized candidates share rank/raw/declaration/startup/code/identity ordering; focused tie-break test passed | gate |
| 2026-08-29 | Registry-owned phase ordering | `cargo check --lib`; `cargo test --lib phase_ordering_recipes_preserve_small_and_broad_variants` | small and broad module recipes moved from `compiler.rs` to the decision registry unchanged | gate |
| 2026-08-29 | Registry-owned compress contrasts | `cargo check --lib`; `cargo test --lib compress_pass_recipes_retain_incumbent_and_named_contrasts` | all-compress-off, outline contrast/interaction, fusion-off, and merging-off construction moved unchanged into the registry; incumbent retained | gate |
| 2026-08-29 | Registry-owned terminal options | `cargo check --lib`; focused terminal scope-naming and string-pooling tests | existing naming and pooling option sets moved unchanged into `decision_registry.rs` | gate |

## Log

- 2026-08-29 — Phase-1 implementation triage closed without package-specific compiler logic. Began acceptance-path inventory. — **OPEN**
- 2026-08-29 — Consolidated the syntax/binding admission and exact-score boundary, including entropy alphabet probes. Full artifact contracts remain attached to typed/terminal candidates. — **OPEN**
- 2026-08-29 — Consolidated scored/final candidate ordering onto one key and removed a non-portable temporary diagnostic test that referenced `/tmp`. — **OPEN**
- 2026-08-29 — Moved the existing phase-order probe construction into a registry-owned helper; no options or admission order changed. — **OPEN**
- 2026-08-29 — Moved the imperative compress-pass contrast batch into the registry without adding candidates or changing ordering. — **OPEN**
- 2026-08-29 — Moved terminal scope-naming and string-pooling option construction into the registry. Phase/order/compress/terminal recipe ownership and shared candidate ordering are consolidated. — **LANDED**

## Next step

Continue with `phase2-02`: unify terminal materialize/validate/score/compare calls.
