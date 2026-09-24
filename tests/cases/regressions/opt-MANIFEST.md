# Harvested old-route regressions: optimizer, lowering, analyses, native

Programs harvested from the `#[test]` modules of `src/optimizer.rs`, `src/lower.rs`,
`src/value_analysis.rs`, `src/compress_passes.rs`, `src/codegen_native.rs` and
`src/decision_registry.rs` at commit `0c17237e` (branch `one-compiler`), before migration
step M1.6 deletes those modules. Line numbers refer to that commit.

## What a case is

- `opt-<test>.lil` is the test's program. A `// Harvested from …` comment is its first line; the rest is the test's source with Rust string escapes resolved.
- `opt-<test>.out` is the expected stdout. It comes from the test's own assertion, never from running a compiler.
- `opt-<test>.host.js` exists only when the program uses `extern` values. It defines them on `globalThis` inside an IIFE, as the test's harness did.
  Where the test printed a trace after the program (`'TRACE:'+…`, an event log), the prelude registers `process.on('exit')` and writes that line there, so the trace stays the last line of stdout.
- Cases with a `host.js` or a `JsValue` are JavaScript-only: the language rejects `JsValue` on C.
- No case needs a `.toml`. The tests set only optimizer knobs (inlining, scalar replacement, …), and those are not carried over.

To run one case: compile it with `[javascript] strip_console = false`, concatenate `host.js` (if any) and the output into one file, and run it with node. On the `module` lane the combined file must be `.mjs`.

## Harvested cases

Verified on 2026-09-23 with the frozen reference binary `~/lilscript-work/bin/reference-2026-09-23/lilscript` (`--target js`, Node 24.11.1).
The semantic column is the remaining compiler (`--backend semantic`); the legacy column is the old route (`--backend legacy`).
The runs are status only, not expectations.

| Case | Source test | Semantic | Legacy | Note |
|---|---|---|---|---|
| `opt-keeps_borrowed_array_call_after_extern_can_shadow_method` | `src/optimizer.rs:14280` | pass | pass | JsValue; host `expose()` installs an own `push`, so JS.push must stay `Array.prototype.push.call`. TRACE written at exit. |
| `opt-inherited_field_summaries_preserve_subclass_hooks_and_integer_overflow` | `src/optimizer.rs:15030` | pass | pass | The test ran with inlining off and on (an optimizer knob; not carried over). C lane (info): semantic passes; legacy refuses inheritance for native. |
| `opt-javascript_coercions_keep_js_value_aggregates_named` | `src/optimizer.rs:16068` | pass | pass | JsValue. The semantic route spells `JsValue == "7"` as `===`; legacy emits loose `==`. Both print `false` here, so on the semantic route the third line no longer exercises a coercion (see Observations). |
| `opt-js_value_length_keeps_object_semantics` | `src/optimizer.rs:16109` | pass | pass | JsValue. A positional struct would print `1`. |
| `opt-js_value_bracket_read_keeps_nominal_object_semantics` | `src/optimizer.rs:16144` | pass | pass | JsValue. A positional struct would print `undefined`. |
| `opt-wrapped_and_generic_dynamic_aliases_keep_nominal_object_semantics` | `src/optimizer.rs:16179` | **fail** (compile refusal: `generic product transport requires a complete private interface`) | pass | Host `coin()` returns true. Semantic refuses all three parts: the `Left\|Right` union widened to JsValue (`value-struct transfer requires an ABI adapter`), and the generic `read<T>`/`stringify<T>` over structs (`generic product transport requires a complete private interface`; module target: `generic product body requires closed value forwarding`). Even `T id<T>(T v){return v;}print(id(E{9}).value);` is refused on the script target. |
| `opt-forwards_stable_root_export_aliases_to_one_public_identity` | `src/optimizer.rs:17095` | pass | pass | The test also imported the module namespace and asserted `alias,second:true` (keys, and `alias===second`). Not expressible as stdout of a script; checked by hand on `--target js-module`: both routes give `alias,second:true`. |
| `opt-exported_internal_inlining_is_scored_without_changing_the_export` | `src/optimizer.rs:17438` | pass | pass | The script target drops the export. C lane (info): legacy passes; semantic refuses `native exported ABI`. |
| `opt-exported_internal_inlining_allocates_per_caller_ssa_metadata` | `src/optimizer.rs:17501` | pass | pass | The script target drops the export. C lane (info): legacy passes; semantic refuses `native exported ABI`. |
| `opt-preserves_array_push_inherited_setter_observation` | `src/optimizer.rs:18147` | pass | pass | Host pollutes `Array.prototype[0]` with an accessor. push is [[Set]]: setter runs once, no own element. The test removed the accessor in a `finally`; the host removes it in `consume()` (the last statement) and at exit. |
| `opt-preserves_plain_object_inherited_setter_observation` | `src/optimizer.rs:18170` | pass | pass | Host puts an accessor for `_lilPlainObjectProbe` on Object.prototype; the store must be an ordinary [[Set]]. |
| `opt-stringify_elision_crosses_intervening_constants` | `src/optimizer.rs:18231` | pass | pass | Host event log, written at exit. |
| `opt-stringify_elision_does_not_move_dynamic_coercion_across_a_call` | `src/optimizer.rs:18246` | pass | pass | Host event log, written at exit. Coercion via Symbol.toPrimitive. |
| `opt-stringify_elision_does_not_move_dynamic_coercion_across_a_getter` | `src/optimizer.rs:18266` | pass | pass | Host event log, written at exit. |
| `opt-stringify_elision_does_not_move_dynamic_coercion_across_a_mutation` | `src/optimizer.rs:18281` | pass | pass | Host event log, written at exit. |
| `opt-stringify_elision_does_not_move_dynamic_coercion_across_another_coercion` | `src/optimizer.rs:18296` | pass | pass | Host event log, written at exit. |
| `opt-a_stored_argument_is_observed_with_the_container_it_is_stored_in` | `src/optimizer.rs:18317` | pass | pass | Host records JSON of what `consume` receives, written at exit. A dead-store miscompile emptied extension tables built through such helpers. |
| `opt-closed_record_projection_uses_decoded_identity_escapes` | `src/optimizer.rs:18398` | pass | pass | `"\q"` is an identity escape for `q`. C lane (info): legacy passes; semantic refuses records (`native source type`). |
| `opt-closed_record_projection_preserves_null_prototype_observations` | `src/optimizer.rs:18492` | pass | pass | Host replaces Object.prototype.toString with 99; a record has a null prototype. TRACE written at exit. |
| `opt-closed_record_json_projection_uses_ecmascript_own_key_order` | `src/optimizer.rs:18515` | pass | pass | Expectation is the folded `print` argument whose text the test asserts; the test itself did not run node. C lane (info): legacy passes; semantic refuses records. |
| `opt-decoded_strings_execute_in_native_c_before_and_after_folding` | `src/codegen_native.rs:4778` | pass | pass | Native test: it compiled the old route's C before and after optimization and ran both. C lane (info): legacy passes with `-std=gnu11` (clang 18 `-std=c11` rejects the runtime's undeclared `strdup`); semantic refuses the Record (`native source type`). |

Script lane (`--target js`): semantic 20/21, legacy 21/21.
Module lane (`--target js-module`, same harness as `.mjs`): semantic 20/21, legacy 21/21. The failing case is the same; its refusal reads `generic product body requires closed value forwarding`.

The C lane was not a gate here. For information, the six cases without JsValue or host externs were compiled with `--target c` and run under clang 18.
The semantic route passes only the inheritance case: it refuses Records (`native source type`) and exported functions (`native exported ABI`).
The legacy route passes the other five with `-std=gnu11` and refuses inheritance.

## Observations

- **Failing on the remaining compiler: `opt-wrapped_and_generic_dynamic_aliases_keep_nominal_object_semantics`.** This is a gap, not a wrong answer: the program is refused at compile time. It has three parts, and each is refused on its own:
  - a struct union widened to `JsValue` (`src/program/javascript_struct_boundaries.rs:73-110`);
  - a generic function taking a struct, which needs a complete private interface on scripts (`src/program/demand.rs:2469`);
  - a generic body that does not return its parameter, on modules (`demand.rs:2482`).
- **`==` on `JsValue` differs between the routes.** The old route emits loose `==`. The remaining compiler lowers `BinaryOp::Eq` to `===` (`src/program/javascript.rs:4721`) and loosens it only between two operands of one primitive type.
  Take `extern JsValue v;print(v=="7");` with host `v={toString(){return "7"}}`: legacy prints `true`, semantic prints `false`.
  `docs/language-v0.1.md` (the JsValue section) counts dynamic equality among the coercing operations, and the old `pure` check treated `value==0` as an observation.
  So one of the two is the language rule, and the case runner cannot decide which. The harvested `javascript_coercions…` case passes on both routes because `{eqValue:7}` is unequal to `"7"` under either rule.
- `opt-forwards_stable_root_export_aliases_to_one_public_identity` keeps only the program's own `print`. Its export-namespace assertion needs a module importer, which the stdout format does not have.

## Skipped tests

219 tests in these six files; 21 harvested, 198 skipped. Reasons:

- `ir` (90): asserts IR, pass reports or escape states only
- `irh` (9): asserts IR built or edited by hand (injected exports/lazy modules, moved instructions)
- `js` (16): asserts emitted JavaScript text only
- `bnd` (15): asserts the named field shape a struct has at an untyped JS boundary (emitted text)
- `c` (19): asserts emitted C text only
- `va` (20): asserts value-analysis facts only
- `unit` (9): tests an internal helper or hand-built IR; no LilScript program
- `diag` (10): asserts a compile-time diagnostic, not stdout
- `reg` (9): decision-registry/config tables; no program
- `dup` (1): program already in tests/cases

The Program column tells a follow-up which programs could still become cases:

- `P`: the program prints and is closed, so an independent oracle could supply `.out`. That oracle is the reference interpreter or JS-against-C agreement, never the compiler under test.
- `H`: the program prints or calls out, but needs host externs whose behaviour the test never states.
- `-`: no observable output.
- `n/a`: no LilScript program.

The `bnd` group is the most valuable one to convert next. Each test states, in the text it asserts, the named field shape a struct has when it reaches the host. That is the `public_aggregate_abi = Named` contract. A host that prints `JSON.stringify` of what it receives would turn each one into a trace case.

| Test | Source | Reason | Program | Note |
|---|---|---|---|---|
| `calls_literal_array_methods_directly` | `src/optimizer.rs:14250` | ir | - |  |
| `calls_stable_private_global_array_methods_directly` | `src/optimizer.rs:14260` | ir | - |  |
| `calls_private_typed_field_array_methods_directly` | `src/optimizer.rs:14270` | ir | - |  |
| `keeps_borrowed_array_call_for_exported_global` | `src/optimizer.rs:14296` | ir | - |  |
| `keeps_borrowed_array_call_for_mutable_global` | `src/optimizer.rs:14306` | ir | - |  |
| `keeps_borrowed_array_call_for_mixed_field_stores` | `src/optimizer.rs:14316` | ir | - | externs, no print; the borrowed-call hazard is covered at runtime by opt-keeps_borrowed_array_call_after_extern_can_shadow_method |
| `keeps_borrowed_array_call_for_defaulted_class_field` | `src/optimizer.rs:14326` | ir | - |  |
| `keeps_borrowed_array_call_after_dynamic_string_key_write` | `src/optimizer.rs:14336` | ir | - | externs, no print; the borrowed-call hazard is covered at runtime by opt-keeps_borrowed_array_call_after_extern_can_shadow_method |
| `keeps_borrowed_array_calls_for_typed_array_and_array_like_js_value` | `src/optimizer.rs:14346` | ir | - | externs, no print; the borrowed-call hazard is covered at runtime by opt-keeps_borrowed_array_call_after_extern_can_shadow_method |
| `keeps_borrowed_field_array_call_when_owner_escapes_untyped` | `src/optimizer.rs:14356` | ir | - | externs, no print; the borrowed-call hazard is covered at runtime by opt-keeps_borrowed_array_call_after_extern_can_shadow_method |
| `method10_wrapper_propagates_untyped_escape_to_callback_captures` | `src/optimizer.rs:14472` | ir | - |  |
| `specializes_constant_arguments_and_removes_unused_call_results` | `src/optimizer.rs:14483` | js | P |  |
| `folds_identical_private_functions_after_inlining_decisions` | `src/optimizer.rs:14520` | ir | H | extern `read()`; runtime coverage exists in tests/cases/function_subsumption.lil |
| `subsumes_private_function_proven_by_constant_binding` | `src/optimizer.rs:14560` | ir | H | extern `read()`; runtime coverage exists in tests/cases/function_subsumption.lil |
| `subsumes_private_function_with_a_middle_constant_parameter` | `src/optimizer.rs:14613` | ir | H | extern `read()`; runtime coverage exists in tests/cases/function_subsumption.lil |
| `subsumes_private_function_proven_by_known_callback_binding` | `src/optimizer.rs:14665` | ir | H | extern `read()`; runtime coverage exists in tests/cases/function_subsumption.lil |
| `preserves_exported_identity_during_function_subsumption` | `src/optimizer.rs:14699` | irh | H | export injected into the IR |
| `preserves_address_taken_identity_during_function_subsumption` | `src/optimizer.rs:14734` | ir | H | identity is observable only through a host `retain`, which the test never exercised |
| `rejects_function_subsumption_without_exact_specialized_cfg` | `src/optimizer.rs:14764` | ir | H | extern `read()`; runtime coverage exists in tests/cases/function_subsumption.lil |
| `preserves_exported_function_identity_during_identical_folding` | `src/optimizer.rs:14794` | irh | H | export injected into the IR |
| `preserves_address_taken_function_identity_during_identical_folding` | `src/optimizer.rs:14835` | ir | H | identity is observable only through a host `retain`, which the test never exercised |
| `preserves_distinct_escape_contracts_during_identical_folding` | `src/optimizer.rs:14864` | ir | H | identity is observable only through a host `retain`, which the test never exercised |
| `normalizes_phi_locals_removed_from_function_metadata` | `src/optimizer.rs:14893` | dup | P | include_str!(tests/cases/nested_short_circuit.lil) |
| `does_not_specialize_tagged_generic_parameters_as_raw_constants` | `src/optimizer.rs:14911` | ir | P |  |
| `specializes_tagged_generic_constants_for_javascript_only` | `src/optimizer.rs:14945` | ir | P |  |
| `folds_interprocedural_finite_values_without_dropping_effectful_calls` | `src/optimizer.rs:14984` | js | P | hazard: folding must keep the effectful call (`effect` then `A`) |
| `folds_exact_nominal_field_values_across_typed_calls` | `src/optimizer.rs:15007` | js | P |  |
| `folds_optional_access_guard_for_a_proven_non_null_receiver` | `src/optimizer.rs:15075` | js | P |  |
| `folds_constants` | `src/optimizer.rs:15094` | unit | n/a |  |
| `folds_mixed_numeric_constants_by_value` | `src/optimizer.rs:15131` | unit | n/a |  |
| `folds_integer_multiplication_with_javascript_operator_semantics` | `src/optimizer.rs:15147` | unit | n/a | its three products are already lines 1, 3 and 5 of tests/cases/integer_multiplication.out |
| `removes_dead_value_chains` | `src/optimizer.rs:15157` | unit | n/a |  |
| `scalar_replaces_non_escaping_structs` | `src/optimizer.rs:15188` | unit | n/a |  |
| `preserves_escaping_structs` | `src/optimizer.rs:15231` | unit | n/a |  |
| `promotes_cfg_locals_and_inserts_loop_phis` | `src/optimizer.rs:15250` | ir | - |  |
| `mem2reg_is_reentrant_for_newly_internalized_entry_globals` | `src/optimizer.rs:15280` | irh | P |  |
| `promotes_only_locals_independent_of_exception_edges` | `src/optimizer.rs:15335` | ir | - | declares `run` but never calls it |
| `keeps_writes_in_nested_exception_regions_mutable` | `src/optimizer.rs:15382` | ir | - | declares `run` but never calls it |
| `keeps_locals_read_by_finally_mutable_across_early_return` | `src/optimizer.rs:15415` | ir | - | declares `run` but never calls it |
| `runs_the_whole_cfg_optimization_pipeline` | `src/optimizer.rs:15462` | ir | P |  |
| `scalar_replacement_on_and_keep_object_are_both_legal` | `src/optimizer.rs:15496` | ir | H | extern `read`/`count`/`choose` inputs |
| `scalar_replacement_explodes_structs_used_by_loop_phis_atomically` | `src/optimizer.rs:15530` | ir | H | extern `read`/`count`/`choose` inputs |
| `scalar_replacement_explodes_the_aggregate_ledger_loop` | `src/optimizer.rs:15560` | ir | H | extern `read`/`count`/`choose` inputs |
| `loop_struct_scalar_replacement_rejects_field_mutation` | `src/optimizer.rs:15615` | ir | H | extern `read`/`count`/`choose` inputs |
| `loop_struct_scalar_replacement_rejects_typed_escape` | `src/optimizer.rs:15642` | ir | H | extern `read`/`count`/`choose` inputs |
| `loop_struct_scalar_replacement_rejects_branch_merges` | `src/optimizer.rs:15669` | ir | H | extern `read`/`count`/`choose` inputs |
| `loop_struct_scalar_replacement_rejects_shared_phi_inputs` | `src/optimizer.rs:15696` | ir | H | extern `read`/`count`/`choose` inputs |
| `devirtualizes_class_method_calls` | `src/optimizer.rs:15723` | ir | P |  |
| `escape_worklist_visits_a_directed_chain_once` | `src/optimizer.rs:15745` | unit | n/a |  |
| `escape_worklist_matches_full_rescan_with_mixed_ranks_and_cycles` | `src/optimizer.rs:15767` | unit | n/a |  |
| `exact_local_closure_call_keeps_nominal_argument_and_result_typed` | `src/optimizer.rs:15825` | ir | P |  |
| `unknown_callback_call_remains_an_untyped_boundary` | `src/optimizer.rs:15849` | ir | - |  |
| `differing_closure_phi_call_remains_an_untyped_boundary` | `src/optimizer.rs:15865` | ir | H |  |
| `mutable_capture_shared_by_sibling_closures_keeps_call_fallback` | `src/optimizer.rs:15888` | ir | P |  |
| `host_retained_exact_closure_keeps_nominal_call_shapes_untyped` | `src/optimizer.rs:15919` | ir | H |  |
| `marks_extern_arguments_as_untyped_escapes` | `src/optimizer.rs:15939` | ir | - |  |
| `extern_aggregate_global_uses_named_field_access` | `src/optimizer.rs:15966` | bnd | H | host-supplied aggregate read by name; convertible with a host that supplies `{value:…}` |
| `thrown_aggregate_keeps_a_named_shape` | `src/optimizer.rs:16000` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `assumed_host_exception_aggregate_uses_named_fields` | `src/optimizer.rs:16034` | bnd | H | host-supplied aggregate read by name; convertible with a host that supplies `{value:…}` |
| `rejected_task_reason_keeps_a_named_shape` | `src/optimizer.rs:16221` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `typed_inherited_field_extraction_keeps_the_stored_aggregate_named` | `src/optimizer.rs:16255` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `hidden_captured_class_with_js_value_field_stays_typed_and_positional` | `src/optimizer.rs:16289` | js | H | asserts the positional layout is kept where no boundary sees it |
| `nominal_values_stored_in_dynamic_fields_keep_named_shapes` | `src/optimizer.rs:16343` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `externally_exposed_known_callable_keeps_return_shape_named` | `src/optimizer.rs:16413` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `escaping_generic_closure_keeps_its_captured_return_named` | `src/optimizer.rs:16466` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `aggregate_pushed_into_js_value_array_is_untyped_and_named` | `src/optimizer.rs:16500` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `generic_container_result_propagates_dynamic_element_shape` | `src/optimizer.rs:16550` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `generic_array_callbacks_preserve_dynamic_element_shapes` | `src/optimizer.rs:16612` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `extern_array_observer_keeps_consumed_aggregates_named` | `src/optimizer.rs:16673` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `extern_array_mapper_uses_named_fields_on_host_results` | `src/optimizer.rs:16707` | bnd | H | host-supplied aggregate read by name; convertible with a host that supplies `{value:…}` |
| `wrapped_js_value_container_elements_are_detected_recursively` | `src/optimizer.rs:16741` | unit | n/a |  |
| `contextual_nullable_js_value_arrays_keep_literal_and_spread_entries_named` | `src/optimizer.rs:16755` | bnd | H | convertible: a host `consume` printing JSON.stringify would see the asserted `{field:value}` literals |
| `dynamic_collection_mutators_and_calls_expose_stored_aggregate_shapes` | `src/optimizer.rs:16831` | bnd | H | asserts escape states only; same contract as the group above |
| `pruned_ssa_handles_locals_scoped_inside_nested_loops` | `src/optimizer.rs:16873` | ir | P |  |
| `eliminates_repeated_ssa_expressions_with_value_numbering` | `src/optimizer.rs:16894` | ir | H |  |
| `canonicalizes_same_block_runtime_type_predicate_spellings` | `src/optimizer.rs:16926` | js | - | export-only library; counts `typeof` |
| `keeps_numeric_type_guards_separate_from_javascript_typeof` | `src/optimizer.rs:16948` | js | - | export-only library; counts `typeof` |
| `reuses_only_dominated_runtime_type_predicates_on_the_same_ssa_value` | `src/optimizer.rs:16967` | js | - | export-only library; counts `typeof` |
| `propagates_single_assignment_globals_into_functions` | `src/optimizer.rs:17010` | ir | P |  |
| `forwards_stable_internal_global_aliases` | `src/optimizer.rs:17062` | ir | H |  |
| `keeps_aliases_of_mutated_globals` | `src/optimizer.rs:17178` | ir | H |  |
| `keeps_globals_with_multiple_alias_stores` | `src/optimizer.rs:17197` | ir | H |  |
| `keeps_aliases_observed_before_initialization` | `src/optimizer.rs:17216` | irh | H |  |
| `keeps_cyclic_global_aliases` | `src/optimizer.rs:17261` | irh | H |  |
| `keeps_lazy_exported_alias_bindings` | `src/optimizer.rs:17311` | irh | H |  |
| `keeps_root_export_aliases_with_mutable_or_external_targets` | `src/optimizer.rs:17349` | ir | - |  |
| `keeps_root_export_aliases_observed_before_initialization` | `src/optimizer.rs:17391` | irh | P | the hazard needs the hand-moved call; the source program alone does not reach it |
| `exported_internal_inlining_preserves_owned_field_slots` | `src/optimizer.rs:17577` | ir | P |  |
| `exported_internal_inlining_keeps_sensitive_call_shapes` | `src/optimizer.rs:17618` | irh | P | five programs; the first recurses forever |
| `does_not_inline_recursive_functions` | `src/optimizer.rs:17771` | ir | P |  |
| `late_inlining_keeps_protected_composites_and_absorbs_their_leaf_callees` | `src/optimizer.rs:17790` | ir | H |  |
| `optimizer_keeps_its_tracked_entry_outline_through_late_inlining` | `src/optimizer.rs:17847` | ir | H |  |
| `outlined_aggregate_results_recompute_untyped_escape_state` | `src/optimizer.rs:17896` | ir | H |  |
| `independently_controls_closure_factory_inlining` | `src/optimizer.rs:17985` | ir | P |  |
| `removes_field_stores_overwritten_before_observation` | `src/optimizer.rs:18050` | ir | H |  |
| `folds_literal_array_lengths_across_unrelated_effects_only` | `src/optimizer.rs:18074` | ir | H | three programs; the third is closed |
| `projects_closed_null_prototype_record_observations` | `src/optimizer.rs:18342` | js | P |  |
| `closed_record_projection_stops_at_unknown_calls` | `src/optimizer.rs:18362` | js | H |  |
| `closed_record_json_projection_requires_portable_constants` | `src/optimizer.rs:18380` | js | H |  |
| `closed_record_projection_does_not_carry_mutation_facts_across_branches` | `src/optimizer.rs:18416` | js | H |  |
| `closed_record_projection_preserves_a_store_before_a_later_observer` | `src/optimizer.rs:18434` | js | P |  |
| `closed_record_projection_crosses_an_unrelated_branch_by_dominance` | `src/optimizer.rs:18454` | js | H |  |
| `closed_record_projection_does_not_cross_a_loop_mutation` | `src/optimizer.rs:18473` | js | H |  |
| `erases_explicit_js_assume_to_the_original_runtime_value` | `src/optimizer.rs:18536` | js | H |  |
| `folds_interprocedural_array_lengths_only_for_closed_stable_calls` | `src/optimizer.rs:18553` | ir | P | four programs; runtime coverage in tests/cases/interprocedural_array_length.lil |
| `removes_unobserved_local_collection_mutation_graphs` | `src/optimizer.rs:18599` | ir | P | prints 7; see tests/cases/local_collection_elision.lil |
| `infers_fluent_local_collection_helpers_are_effect_free` | `src/optimizer.rs:18632` | ir | P | prints 7; see tests/cases/local_collection_elision.lil |
| `rejects_declared_pure_dynamic_javascript_observations` | `src/optimizer.rs:18660` | diag | H | the `pure` contract check; migration plan restores it in M4.3 |
| `keeps_typed_record_for_in_pure_and_dynamic_iteration_calls_effectful` | `src/optimizer.rs:18692` | ir | P | the record program is closed; the two iteration programs have no output |
| `dce_retains_each_unused_dynamic_coercion_and_array_check` | `src/optimizer.rs:18747` | ir | H | convertible as a host trace (coercions run twice, isArray and the read once) but the test states no trace |
| `removes_parameter_mutation_calls_only_for_unobserved_roots` | `src/optimizer.rs:18802` | ir | P |  |
| `preserves_parameter_mutation_calls_with_inherent_effects` | `src/optimizer.rs:18835` | ir | P |  |
| `preserves_collection_mutations_with_observed_state_or_results` | `src/optimizer.rs:18864` | ir | P |  |
| `profile_guidance_specializes_higher_order_calls_and_devirtualizes_them` | `src/optimizer.rs:18912` | ir | P |  |
| `profile_guidance_clones_constant_capture_signatures` | `src/optimizer.rs:18954` | ir | P |  |
| `legacy_entry_points_refuse_source_reference_calls_before_mode_erasure` | `src/lower.rs:5570` | diag | P | asserts the old entry points refuse `ref`; the reference semantic route accepts it on `js-module` only (script: `reference callable requires strict module execution`) |
| `lowers_branches_and_loops_to_blocks` | `src/lower.rs:5611` | ir | P |  |
| `lowers_methods_as_directly_identified_calls` | `src/lower.rs:5622` | ir | - |  |
| `lowers_extended_javascript_method_adapters_to_distinct_intrinsics` | `src/lower.rs:5633` | ir | - |  |
| `preserves_source_arity_while_materializing_direct_call_defaults` | `src/lower.rs:5672` | ir | - |  |
| `lowers_short_circuit_to_phi` | `src/lower.rs:5701` | ir | - |  |
| `lowers_expression_if_to_a_source_conditional_phi` | `src/lower.rs:5710` | ir | - |  |
| `lowers_recursive_local_function_through_its_own_slot` | `src/lower.rs:5724` | ir | P |  |
| `lowers_lexical_closure_captures_explicitly` | `src/lower.rs:5742` | ir | P |  |
| `lowers_inherited_methods_and_base_constructor_calls_directly` | `src/lower.rs:5765` | ir | - |  |
| `lowers_generators_to_yield_intrinsics_and_native_for_of_shapes` | `src/lower.rs:5800` | ir | - |  |
| `lowers_static_record_indexes_to_record_fields` | `src/lower.rs:5828` | ir | - |  |
| `keeps_dynamic_record_indexes_generic` | `src/lower.rs:5858` | ir | - |  |
| `lowers_static_proto_record_indexes_without_changing_the_key` | `src/lower.rs:5883` | ir | - |  |
| `static_record_index_enables_closed_record_projection` | `src/lower.rs:5912` | ir | P |  |
| `propagates_direct_call_arguments_and_returns` | `src/value_analysis.rs:1982` | va | H |  |
| `proves_unit_updates_bounded_by_an_enclosing_induction` | `src/value_analysis.rs:2023` | va | H |  |
| `proves_unit_update_bounded_by_a_derived_array_length` | `src/value_analysis.rs:2051` | va | H |  |
| `proves_filtered_compaction_count_bounded_by_array_induction` | `src/value_analysis.rs:2079` | va | H | also counts `+1\|0` in the text |
| `keeps_filtered_compaction_count_normalized_at_inclusive_i32_max` | `src/value_analysis.rs:2116` | va | H | i32-overflow hazard at 2147483647; convertible with a host `keep`/`consume` trace |
| `keeps_follower_normalized_when_it_can_advance_twice_per_iteration` | `src/value_analysis.rs:2153` | va | H |  |
| `keeps_inclusive_array_length_update_normalized_at_i32_max` | `src/value_analysis.rs:2183` | va | H |  |
| `keeps_inclusive_i32_max_induction_update_normalized` | `src/value_analysis.rs:2211` | va | H | i32-overflow hazard at 2147483647; convertible with a host `keep`/`consume` trace |
| `invalidates_fields_that_cross_an_untyped_boundary` | `src/value_analysis.rs:2239` | va | H |  |
| `invalidates_fields_in_exported_function_type_graphs` | `src/value_analysis.rs:2260` | va | - | export-only |
| `treats_lazy_exported_integer_parameters_as_full_public_inputs` | `src/value_analysis.rs:2281` | va | P | lazy module injected into the IR |
| `treats_lazy_exported_finite_parameters_as_unknown_public_inputs` | `src/value_analysis.rs:2318` | va | P | lazy module injected into the IR |
| `invalidates_aggregate_facts_crossing_lazy_exported_type_graphs` | `src/value_analysis.rs:2354` | va | P | lazy module injected into the IR |
| `widens_recursive_argument_growth_instead_of_iterating_by_value` | `src/value_analysis.rs:2391` | va | P |  |
| `propagates_finite_arguments_and_returns_across_direct_calls` | `src/value_analysis.rs:2419` | va | P |  |
| `summarizes_exact_and_finite_nominal_fields` | `src/value_analysis.rs:2455` | va | P |  |
| `invalidates_finite_fields_at_untyped_boundaries` | `src/value_analysis.rs:2478` | va | H |  |
| `does_not_treat_unmodeled_generic_values_as_absent_nullable_values` | `src/value_analysis.rs:2494` | va | P |  |
| `does_not_treat_class_values_as_absent_nullable_returns` | `src/value_analysis.rs:2515` | va | P |  |
| `widens_large_value_sets_to_unknown` | `src/value_analysis.rs:2536` | va | P |  |
| `fuses_map_map_pipelines_into_one_callback` | `src/compress_passes.rs:1983` | ir | P |  |
| `sinks_local_array_into_single_branch` | `src/compress_passes.rs:2048` | ir | H |  |
| `superoptimizes_int_identities_and_double_not` | `src/compress_passes.rs:2093` | ir | P |  |
| `propagates_constants_along_known_branch` | `src/compress_passes.rs:2133` | ir | P |  |
| `run_compress_passes_respects_options_order` | `src/compress_passes.rs:2168` | ir | P |  |
| `outlining_tracks_repeated_regions_from_the_closed_script_entry` | `src/compress_passes.rs:2198` | ir | H |  |
| `outlining_declines_modules_with_lazy_chunk_ownership` | `src/compress_passes.rs:2259` | irh | P |  |
| `outlining_separates_typed_strings_from_dynamic_coercion_regions` | `src/compress_passes.rs:2307` | ir | H |  |
| `outlining_preserves_non_int_phi_live_in_types` | `src/compress_passes.rs:2422` | ir | H |  |
| `outlining_resolves_pending_cross_block_dependencies` | `src/compress_passes.rs:2487` | ir | H |  |
| `outlines_repeated_index_set_store_regions` | `src/compress_passes.rs:2582` | ir | P |  |
| `emits_enums_as_int32_without_runtime_metadata` | `src/codegen_native.rs:4304` | c | P |  |
| `emits_structural_records_through_the_native_map_runtime` | `src/codegen_native.rs:4319` | c | P |  |
| `emits_portable_record_object_and_json_runtime_calls` | `src/codegen_native.rs:4333` | c | P |  |
| `rejects_json_parse_for_native_targets` | `src/codegen_native.rs:4348` | diag | - | native-target refusal |
| `rejects_first_class_javascript_abi_for_native_targets` | `src/codegen_native.rs:4361` | diag | - | native-target refusal |
| `rejects_typed_javascript_adapters_for_native_targets` | `src/codegen_native.rs:4378` | diag | - | native-target refusal |
| `rejects_regex_for_native_targets_without_approximating_ecmascript` | `src/codegen_native.rs:4395` | diag | P | native-target refusal; the program prints on the JavaScript lanes |
| `rejects_exceptions_for_native_targets` | `src/codegen_native.rs:4412` | diag | - | native-target refusal |
| `rejects_generators_and_inheritance_until_native_abis_are_exact` | `src/codegen_native.rs:4423` | diag | P | native-target refusal; the program prints on the JavaScript lanes |
| `emits_native_indexed_for_of_loops` | `src/codegen_native.rs:4458` | c | P |  |
| `emits_native_shallow_spread_runtime_calls` | `src/codegen_native.rs:4471` | c | P |  |
| `emits_native_bounds_checked_destructuring_and_copied_rest` | `src/codegen_native.rs:4484` | c | P |  |
| `emits_c_from_optimized_ssa` | `src/codegen_native.rs:4502` | c | P |  |
| `rejects_javascript_values_at_the_native_boundary` | `src/codegen_native.rs:4514` | diag | - | native-target refusal |
| `emits_nominal_c_abi_for_escaping_aggregates` | `src/codegen_native.rs:4529` | c | H | extern C functions |
| `emits_tagged_generic_equality` | `src/codegen_native.rs:4544` | c | P |  |
| `devirtualizes_named_functions_used_as_local_closures` | `src/codegen_native.rs:4559` | c | P |  |
| `retained_and_thrown_allocations_are_not_function_bounded` | `src/codegen_native.rs:4571` | ir | - |  |
| `stack_allocates_fixed_local_arrays_and_keeps_resizable_arrays_on_heap` | `src/codegen_native.rs:4624` | c | P |  |
| `emits_in_place_array_fill_for_native_targets` | `src/codegen_native.rs:4654` | c | P |  |
| `emits_native_collection_search_join_and_typed_bulk_operations` | `src/codegen_native.rs:4668` | c | P |  |
| `emits_native_utf16_string_search_and_repeat_operations` | `src/codegen_native.rs:4691` | c | P |  |
| `emits_native_lazy_nullish_control_flow` | `src/codegen_native.rs:4705` | c | P |  |
| `emits_native_optional_access_control_flow` | `src/codegen_native.rs:4719` | c | P |  |
| `lowers_number_alias_to_native_binary64` | `src/codegen_native.rs:4731` | c | P |  |
| `region_allocates_bounded_arrays_above_the_stack_limit` | `src/codegen_native.rs:4743` | c | P |  |
| `heap_allocates_closures_retained_in_class_fields` | `src/codegen_native.rs:4763` | c | P |  |
| `native_literal_nul_is_rejected_until_the_runtime_can_represent_it` | `src/codegen_native.rs:4841` | diag | P | native-target refusal; the program prints on the JavaScript lanes |
| `migrated_decision_names_and_ids_are_unique` | `src/decision_registry.rs:1732` | reg | n/a |  |
| `reversible_boolean_family_keeps_the_incumbent_first` | `src/decision_registry.rs:1756` | reg | n/a |  |
| `every_ir_js_options_field_is_classified_once` | `src/decision_registry.rs:1767` | reg | n/a |  |
| `scored_emission_families_are_named_uniquely_and_skip_illegal_axes` | `src/decision_registry.rs:1808` | reg | n/a |  |
| `omitting_length_to_number_elision_does_not_admit_that_family` | `src/decision_registry.rs:1851` | reg | n/a |  |
| `cartesian_seed_keeps_the_configured_incumbent` | `src/decision_registry.rs:1886` | reg | n/a |  |
| `scored_ir_variants_are_named_uniquely_and_keep_object_is_legal_on_size_first` | `src/decision_registry.rs:1915` | reg | n/a |  |
| `exported_internal_inlining_is_a_distinct_opt_in_ir_clone` | `src/decision_registry.rs:1942` | reg | n/a |  |
| `global_alias_forwarding_is_a_distinct_exact_list_aware_ir_clone` | `src/decision_registry.rs:1996` | reg | n/a |  |

Scratch, scripts and raw results: `~/lilscript-work/out/harvest/opt/` (`gen.py` writes the cases, `verify.sh` runs them, `results-js.tsv` and `results-js-module.tsv` hold the runs).
