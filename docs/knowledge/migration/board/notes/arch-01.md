# arch-01 — independent architecture and documentation audit

Parent: [ledger](../LEDGER.md). Status: landed.

## Question

Does the documentation distinguish LilScript's implemented compiler decision
system from its intended globally codec-optimal architecture, and what migration
closes the gap without glue?

## Current hypothesis

The repository already has strong language, compiler, configuration, and evidence
documents, but recent implementation changes and a growing set of one-off candidate
families have outpaced the shared architectural model. A source-grounded inventory
will either confirm that gap or falsify it by locating one authoritative decision
registry and end-to-end search contract.

## Constraints specific to this task

Read-only implementation audit. The brief allowed factual fixes on
`architecture.md`, `decision-registry.md`, and `07-global-compressor.md` plus
this note. Separate implemented behavior, measured evidence, known debt, and
proposal. Do not mark 07.1–07.7 complete.

## Evidence

| date | what | command | result | tag |
|---|---|---|---|---|
| 2026-08-28 | prove-it status + symbols | `git status --short`; `rg -n "optimize_and_select_javascript\|apply_search_off_declaration_peephole\|pool_identifier_strings\|pack_string_arrays\|ordinary_records_safe\|scalar_replacement" src/compiler.rs src/config.rs src/optimizer.rs` | `pool_identifier_strings` absent from `compiler.rs`; packing Cartesian `[configured, false]` at `compiler.rs:3354`; `ordinary_records_safe = false` at `:3337`; `scalar_replacement` never assigned in `compiler.rs` search clones; `apply_search_off_declaration_peephole` at `:8501` | diag |
| 2026-08-28 | line counts | `wc -l src/codegen_ir_js.rs src/compiler.rs src/optimizer.rs src/js_peephole/folds/classes.rs src/semantic.rs` | 35479 / 19521 / 16445 / 7740 / 8171 | diag |
| 2026-08-28 | root compression list | read `lilscript.toml` | omits `joint-representation-search`, `joint-chunk-symbol-search`, `property-mangling`, `region-outlining`, `length-to-number-elision`, …; includes `string-array-packing` and `ir-inlining-variants` | diag |
| 2026-08-28 | export class | `lower.rs` `Item::Class` → `type_names`; export → `ExportBinding::TypeOnly` unless the name is also a global (`object` types) | `export class` is TypeOnly | diag |
| 2026-08-28 | ident-05 | `LEDGER.md` ident-05 **active**; `notes/ident-05.md` still OPEN | search can still rank unresolved names | diag |
| 2026-08-28 | peephole scoring | `apply_selected_canonical_peephole` (`:8448`) calls `compressed_size` and keeps only transfer-then-raw wins; `apply_search_off_declaration_peephole` (`:8501`) applies `configured_declaration_peephole` with no codec compare; search-off zeros terminal budget (`CandidateSearch::Off` → `terminal_codec_probe_level_limit` 0) | scored vs unscored split holds | diag |
| 2026-08-28 | length-to-number hole | `select_javascript_candidate_global` flips `elide_length_tonumber` with no compression gate (`compiler.rs:3748`) | omitted `length-to-number-elision` can still turn on; docs claimed the opposite | diag |
| 2026-08-28 | explicit i32 intent | `lower.rs:4254-4276`; `codegen_ir_js.rs:11955-12018`; `js_peephole/folds/integers.rs:1586-1624` | source `x \| 0` starts as IR `BitOr`; generated normalization starts in emission; terminal folds receive text without an end-to-end source obligation and remove some redundant `\|0` forms | diag |
| 2026-08-28 | boundary worlds | `language-v0.1.md`; `packages-exports-abi.md`; `JavaScriptConfig` | internal graph is closed in both app/library output, but ABI, unsafe assumptions, and objective knobs are not normalized into separate compiler inputs | diag |

## Current state

Implemented vs intended is already named in
[current architecture](../../../compilation/current-architecture.md),
[goal architecture](../../../compilation/goal-architecture.md), and
[decision registry](../../../compilation/decision-registry.md). The coordinator
is still `src/compiler.rs` (~19.5k); there is no code registry. 07.1–07.7 remain
a plan.

### Layers (who decides today)

1. Proof — `semantic.rs` types/escape, `optimizer.rs` (`EscapeState::LocalOnly` for scalar replace), `ir.rs`.
2. ABI — `public_aggregate_abi`, `aggregate_layout`, `function_spelling`, `[mangle]`, `assume_*`.
3. Compression allowlist — `JavaScriptConfig::compression_enabled` / `JavaScriptPriority::enables_compression`. Exact `compression = [...]` replaces the priority list for **canonical** emission.
4. Search feature — `optimization_enabled` (level or exact `optimizations`); dual-gated rows pass `legacy: Some(CompressionDecision)` so omitting the compression name kills that search (`ir_inlining_variants_enabled`).
5. Codec incumbent — `ProjectConfig::js_options()`. Brotli forces `pack_string_arrays = false` and `pool_identifier_strings = false`; `function_spelling` defaults Function vs Arrow; `local_phi_expression_regions` / `phi_edge_value_forwarding` off under Brotli (level ≥ 4 otherwise).
6. Candidate search — `optimize_and_select_javascript` → IR clones → `select_javascript_candidate_global` Cartesian then sequential `extend_javascript_candidate_beam`. Rank: `javascript_candidate_rank` (size-first = exact transfer, performance on tie).
7. Terminal rewrite — peephole leaves + late cleanup (`LateJavaScriptCleanupPass`, skip-the-pass is a branch) + `apply_selected_canonical_peephole` (scored).
8. Unscored heuristic — `apply_search_off_declaration_peephole`; always-on IR size passes; `repair_late_javascript_candidate`.

`--mode development` sets `candidate_search = Off` (`main.rs`). That caps IR/emission expansion and zeros optional terminal codec work. Configured IR + emit still run.

### Two-level search (what actually clones)

**IR** (`optimize_and_select_javascript_inner`): always `js_optimizer_options()` first. Opportunistic clones: no closure-factory, inlining fully off, no const-param / call-site / capture specialization, reusable-helper combo, subsumption on/off, phase-order (no early CSE / aggressive 48/128/40 / both; broad module = `functions.len() > 24` or >2048 ops → one combined probe), compress all-off, outlining contrast, fusion/merging off, reserved 2nd/3rd slots for outlining contrast and aggressive×outlining. Each probe emits with **configured** `js_options()`.

**Not an IR clone:** `scalar_replacement` off (pass at `optimizer.rs:302`, never flipped in `compiler.rs`), DCE off, escape off, ES class vs dissolved object.

**Emission:** Cartesian of pooling / booleans / structured closures / `pack_string_arrays` `[configured, false]` / regex / catch / generator-star / scalar-phi / phi-affinity. Then sequential families (shadowing, frequency names, `stable_local_names` early **and** late — zod −58 comment, `struct_method_shorthand`, length-to-number, window-root pooling, array-prototype alias, exclusive closures, IIFE clusters, `nested_once_run_helpers` off, string-pool thresholds 16…512, batch-assign minima, constructor fusion, host-alias Direct, `indexed_char_at` on, `effect_ternary` off, independent grammar punctuations, `function_spelling` if unset, `inline_single_use_functions` on, pure-helper × dense tables, callee defaults, truthy-nullable, conditionals / phi / comma / loops / mutation / SSA destruction / control / switch / function-layout / joint named-vs-positional with `public_aggregate_fields: true` both ways / property mangling / entropy alphabets). Fresh-array factories: at most two complete option sets. Budgets: production cap 384, beam 12, proposal/terminal full through 16 KiB then ÷4 to 64 KiB then ÷12. Objective-stratified intermediate retention; one `cost_model` still wins.

**Never flipped:** `pool_identifier_strings` (no hits in `compiler.rs`); `pack_string_arrays` under Brotli (both Cartesian values false); `ordinary_record_literals` (`ordinary_records_safe = false` hardcoded, `ir_javascript_ordinary_records_safe` unused for production); `bare_window_root` (comment at `compiler.rs:3772`); `ecmascript`; ES class for identity-free types (not an `IrJsOptions` field; IR emits `Foo$init` / objects / arrays; named `class` is peephole `fold_constructor_prototype_tables_to_classes` / `fold_named_class_identity` in `folds/classes.rs`); scalar-replacement off.

**Allowlist hole:** `elide_length_tonumber` flipped unconditionally (`compiler.rs:3748`). Root omits `length-to-number-elision`; search can still turn it on. `indexed-char-at` is the documented search-only overlay (`search_compression_enabled`). Packing is the opposite hole: listed in root compression but Brotli `js_options` forces false and Cartesian cannot re-enable.

**Chunks:** `score_javascript_chunk_plan`. Joint-chunk-symbol-search (omitted from root; size-first + level ≥ 14) adds `function_layout` + `local_name_reserve: 0` only — not the single-file alphabet beam.

**Peephole:** `optimize_generated_javascript_pass` runs dozens of folds (class fusion twice plus late `fold_named_class_identity`). Search-on: scored leaves + scored canonical rewrite of the winner (`apply_selected_canonical_peephole`). Search-off: same pipeline via `apply_search_off_declaration_peephole` with **no** `compressed_size`. `parsed-peephole` min level 9.

**Language:** `export class` → `ExportBinding::TypeOnly` (`lower.rs:291`). `object` declarations also enter `global_names` and can export as `Global`. Native clone is the **unoptimized** IR (`compile_program_all_configured`); C uses `optimizer_options()` + `[native]` storage, not `javascript.priority`. HostField/JsValue/Regex/Task/dynamic import/inheritance rejected in `codegen_native.rs::validate_host_boundaries`.

**Root `lilscript.toml`:** `priority = size-first`, `cost_model = brotli`, `candidate_search = production`, `local_name_reserve = 48`, subset compression. Language tests compile under that subset, not a bare size-first matrix.

`IrJsOptions` has 74 fields. Policy is scattered across `CompressionDecision`, `JavaScriptOptimization`, `js_options()`, `OptimizationOptions`, and imperative beam closures.

### Highest-risk debt

1. **ident-05 still active.** Search can rank unresolved bindings. 07.1 before widening search.
2. **Identity invented in peephole** (`classes.rs` ~7.7k), not IR named-class emit. `export class` cannot publish a constructor value.
3. **Irreversible Brotli priors:** packing, identifier-string pooling; scalar-replacement has no off-clone.
4. **Budgets starve late families** (search-03: 18 KiB / level 15 → 96 units vs ~38 families). Exhaustion is reported, not treated as “incumbent proved best.”
5. **No code registry.** Adding a tactic is a special case in `compiler.rs`. `elide_length_tonumber` already escaped the allowlist story.
6. **`assume_pure_property_reads`** (default off) is a library contract, not a type.
7. **No semantic firewall.** Source/generated operations, application/library
   ABI, unsafe assumptions, and profitability policy meet in the same pipeline.
8. **Generated JavaScript is reparsed.** Binding identity is reconstructed from
   text instead of carried into a hygienic target AST.

### Doc errors fixed this pass

- Architecture claimed split/preserve-modules run a per-chunk **alphabet** beam under `joint-chunk-symbol-search`. Source scores `function_layout` + `local_name_reserve` only (`score_javascript_chunk_plan`).
- Architecture/registry claimed search cannot enable an omitted non-search-only decision. `elide_length_tonumber` is an unconditional flip. 07 had no false claim here (packing/pooling irreversibility holds).
- `IrJsOptions` field count ~70 → 74.

Flagged claims that **hold:** packing Cartesian cannot re-enable under Brotli; `pool_identifier_strings` never flipped in `compiler.rs`; no scalar-replacement IR off-clone; `joint-representation-search` omitted from root toml; search-off peephole unscored; canonical peephole scored; `export class` TypeOnly; ES class for identity-free types is not an `IrJsOptions` family; `ordinary_records_safe = false`; ident-05 active.

## Log

- 2026-08-28 — Source-audited architecture/registry/07 against `compiler.rs` / `config.rs` / `optimizer.rs` / `codegen_ir_js.rs` / `js_peephole` / `lower.rs` / root toml. Corrected chunk-search wording and the length-to-number allowlist exception. 07.1–07.7 unmarked complete. ident-05 still blocks widening search. — **LANDED**
- 2026-08-28 — Deeper pass: objectives (exact vs heuristic, coordinate-descent pairs, one winner per invocation), compressor surface (forks lose from missing proofs, not missing folds; typed rewrite is not a *T* theorem), 07.7 RFC table. No compiler code. — **LANDED**
- 2026-08-28 — Extended the migration with the explicit-intent/application-library contract requested for the optimizer-native language: live source `x | 0` vs generated normalization, stable provenance, ABI manifest, closure/property representation families, hygienic target JS AST, deterministic portfolio search, and source-attributed explain gates. No compiler code. — **LANDED**
- 2026-08-28 — Split implemented facts into `current-architecture.md` and the target solver into `goal-architecture.md`. The goal specifies exactness labels, `ChoiceGraph`/`DecisionVector`, exact islands, property and closure search, deterministic anytime budgets, Pareto reporting, bundle composition, caches, and pseudocode. No compiler code. — **LANDED**

## Next step

ident-05: refuse unresolved bindings in the admitted candidate set. Do not start
07.2/07.3 while that is red.
