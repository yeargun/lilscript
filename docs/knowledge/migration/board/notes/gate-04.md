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
| 2026-08-29 | Export ABI witness | `cargo test --release --lib observed_export_names`; `cargo test --release --lib generated_export_names`; `cargo test --release --lib can_mangle_public_esm_export_names` | exact aliases, missing/extra/duplicate names, and explicitly mangleable export counts passed | gate |
| 2026-08-29 | Module and obligation witnesses | `cargo test --release --lib observes_generated_static_import_edges`; `cargo test --release --lib foreign_import`; `cargo test --release --lib observed_javascript_must_retain`; `cargo test --release --lib source_written_i32_normalization` | static source/imported names ignore local aliases; six foreign-import tests and three `|0` witness tests passed | gate |
| 2026-08-29 | Callable ABI witness | `cargo test --release --lib observes_generated_export_callable_shapes`; `cargo test --release --lib can_mangle_public_esm_export_names`; `cargo test --release --lib explicit_constructor_export`; `cargo test --release --lib constructor_export_synthesizes` | function/arrow/constructor kind, default-sensitive arity, constructibility, inherited method shape, export mangling, and default constructors passed | gate |
| 2026-08-29 | Static property admission | `cargo test --release --lib observes_static_properties`; `cargo test --release --lib final_javascript_cannot_introduce`; Marked gzip compile plus semantic lane | final contraction may remove but cannot invent a static property outside the selected direct typed emission; focused tests passed and Marked passed 660/660 in 65.59 s | gate |
| 2026-08-29 | Pre-score typed admission | targeted export, constructor, foreign-import, and source-`|0` tests | configured root, optional IR contexts, projection variants, spelling variants, and entropy variants are checked against their typed IR before entering initial or terminal exact scoring | gate |
| 2026-08-29 | Late-probe witness propagation | `cargo test --release --lib terminal_cleanup_reopens`; `cargo test --release --lib selected_canonical_peephole`; `cargo test --release --lib search_off_finalization`; targeted export/constructor/import/obligation tests | admission contract is retained with selected candidates; canonical, search-off, cleanup, Boolean, and final remap winners fail closed against it | gate |

## Log

- 2026-08-29 — Gate-02, V-02, and V-03 landed. V-01 is now the first open canonical phase-0.5 unit. — **OPEN**
- 2026-08-29 — Routed declaration variants, initial/optional emissions, objective rescoring, baseline fallbacks, terminal probes, cleanup, and identifier remaps through generated-JS syntax/binding admission before codec invocation. Property/module/ABI/obligation witnesses remain open. — **OPEN**
- 2026-08-29 — Final selected bytes now match typed runtime export names (or exact export count under explicit export mangling), static foreign module edges, and a conservative live source-`|0` obligation count. Callable topology and owner-qualified property categories remain open. The requested full G2 rerun was aborted and is not evidence. — **OPEN**
- 2026-08-29 — Final selected exports now resolve to declarations and match typed callable kind, arity, constructibility, and inherited method signatures. Owner-qualified property categories remain open. — **OPEN**
- 2026-08-29 — Added a conservative property gate: every final dot/object/class/static-bracket property must already occur in the selected plan's direct typed emission. This blocks text-stage property mutation; owner/slot identities still wait for target-JS provenance. — **OPEN**
- 2026-08-29 — Moved current ABI/obligation/property admission ahead of initial and terminal scoring for every typed emission and prepared leaf. Late text challengers retain final fail-closed validation; carrying the full witness into each late probe remains open. — **OPEN**
- 2026-08-29 — Candidate admission now survives into terminal selection and gates text cleanup before exact scoring; binding-only remaps retain their V-02 proof and are revalidated before selection. Owner/slot property identity still waits for hygienic target emission. — **OPEN**

## Next step

Replace the spelling-set property check with owner/slot provenance from target
emission, then run focused cross-objective API fixtures.
