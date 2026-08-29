# gate-04 — final-artifact admission

Parent: [ledger](../LEDGER.md). Status: active.

## Question

Can every incumbent and challenger be rejected before codec scoring when its
printed JavaScript has invalid syntax, unresolved or changed bindings, an
unclassified property, a broken module edge or ABI element, or a dropped lowering
obligation?

## Current hypothesis

The existing parser, binding resolver, compilation contract, and source ABI
manifest can supply a first mandatory admission path without introducing the
planned target-JS tree early. Missing witnesses must fail closed.

## Constraints specific to this task

Validation runs before every exact codec call and applies equally to incumbents
and challengers. Final-byte parsing is an independent check, not a new source of
semantic identity. Do not weaken a rejection to preserve bytes.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-29 | Predecessor G2 | `node comparison/large-libraries/run.mjs --run --compiler migration,candidate ...` | every current candidate boundary passed; the invalid incumbent Marked raw/gzip artifacts were rejected by fresh semantics | gate |
| 2026-08-29 | Codec admission rejection | `cargo test --release --lib generated_javascript_admission`; `cargo test --release --lib declaration_variants_are_admitted`; `cargo test --release --lib terminal_codec_probe_budget` | malformed syntax and cross-function unresolved bindings were rejected with zero codec calls; declaration variants use the same path; 3 targeted groups passed | gate |
| 2026-08-29 | Full Rust library suite | `cargo test --release --lib` | 1,605 passed, 0 failed | gate |
| 2026-08-29 | Marked gzip canary | `lilscript src/entry.lil --target js-module --config lilscript.gzip.toml --mode production ...`; `node comparison/large-libraries/semantic/marked-lane.mjs ...` | candidate compiled in 75.78 s and passed 660 corpus cases / 2,640 parse checks / 660 inline checks | gate |

## Log

- 2026-08-29 — Gate-02, V-02, and V-03 landed. V-01 is now the first open canonical phase-0.5 unit. — **OPEN**
- 2026-08-29 — Routed declaration variants, initial/optional emissions, objective rescoring, baseline fallbacks, terminal probes, cleanup, and identifier remaps through generated-JS syntax/binding admission before codec invocation. Property/module/ABI/obligation witnesses remain open. — **OPEN**

## Next step

Extend admission with observed export/module ABI and property/lowering-obligation
witnesses; reject missing witnesses before codec invocation.
