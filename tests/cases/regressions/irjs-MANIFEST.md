# Harvested regressions from the old route's `codegen_ir_js.rs` tests

Source: the `tests` module of `src/codegen_ir_js.rs` (lines 27603-39432) at commit `0c17237e`; line numbers are at
that commit. Expected stdout comes from each test's own assertions, never from running a compiler.

## Counts

- Tests read: 451 `#[test]` functions. 170 of them execute JavaScript (`run_javascript` or a spawned `node`).
- Harvested: 166 executing tests, giving 202 cases (163 tests own at least one case; 3 more ran a
  program another case already holds and are listed in that case's source column). 146 cases have a `.host.js`,
  11 a `.toml`.
- Skipped: 4 executing tests with no LilScript program to harvest, and 281 tests that never execute (reasons below).
- Reference binary `~/lilscript-work/bin/reference-2026-09-23/lilscript`, `--target js`: semantic route
  **192/202** pass, legacy route **196/202** pass.

## Case conventions

- `irjs-<test>.lil` + `.out`: the program and its expected stdout, one line per `print`/`console.log`. When one test
  holds several programs or host variants, the case is `irjs-<test>-<variant>`. The first program keeps the bare name.
- `.host.js`: a prelude that runs before the compiled program, in the same realm. Each is one block (an arrow IIFE)
  that assigns the harness's externs on `globalThis`. When the harness did work after the program (calling a kept
  callback, printing a trace), the prelude does it in `process.on('exit')`. When the harness wrapped the program in
  `try/catch`, the prelude uses `process.on('uncaughtException')`.
- `.toml`: only language or contract settings the test set: `javascript.public_aggregate_abi = "positional"` (the
  test's `public_aggregate_fields = false`; refused since M1.4, D2, and kept only where the host reads positional
  slots, as ledgered refusal cases), `javascript.assume_pristine_builtins`, `mangle.extern_fields` (no effect since M1.4). Optimizer
  and emitter knobs (spellings, inlining, mangling, pooling, fusion) are never carried over. The runner merges the
  `[javascript]` keys into its own `[javascript]` table rather than appending a second one.
- Module tests whose harness imported an export and called it: the call moves into the program as `print(...)`, and
  the host values the harness passed become `extern` values. The `export` stays in the source.
- Runner used for the status columns: config `[javascript] strip_console = false` plus the case `.toml`;
  `lilscript <case>.lil --backend <route> --target js --config <cfg> -o out.js`; prelude + `out.js` concatenated into one
  file run with Node v24.11.1; stdout compared byte for byte with `.out`. Scripts and outputs:
  `~/lilscript-work/out/harvest/irjs/` (`gen.py`, `verify.py`, `runs/`).

## Harness changes from the original tests

- `transitive_nested_shadowing_keeps_rest_formals_off_defaulted_locals`: the test chose the callback by
  `value.length >= 3`. That relied on the old route promoting the `JS.methodRest` wrapper to three formals.
  docs/language-v0.1.md fixes a rest wrapper's `length` at 0, so the prelude keeps the last kept function.
- `nested_body_still_reads_the_host_alias_method_call`: the test compiled with `lower_known_js_host_calls`, which bound
  the extern `objectToStringTag` by name, so its harness never defined it. The prelude defines it
  (`Object.prototype.toString.call`); an extern is host-provided and the semantic route gives no meaning to extern names.
- `nested_lexical_js_bindings_keep_the_callback_context`: the test wrapped the whole output in a JavaScript function
  to choose the ambient `this` and `arguments`. In the `-this` case the program passes its own top-level `this` to the
  host. In the `-arguments` case the body runs inside a LilScript function called with `"ambient"`.
- `emits_exclusive_recursive_local_called_from_a_nested_closure_as_iife`, `writes_a_captured_or_assign_...`,
  `captured_options_keep_...` and the other module tests: the harness's calls on exported bindings moved into the program.
- Partial harvests, where the JavaScript-side arity or host-omission checks cannot be expressed in a script case:
  `public_class_defaults_preserve_omitted_calls_and_method_arity`,
  `exported_js_undefined_default_uses_host_omission_without_shortening_length`,
  `omits_exported_trailing_undefined_default_at_internal_calls`.
- `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-coercion`: the test asserted only the
  `TRACE:left,right` suffix. The `RL` line before it follows from the program (`b+a`).

## Expectation conflicts with the language spec

- `suppresses_adapter_name_in_variable_and_aggregate_initializers-aggregate` expects the wrapper name `handle`, and
  `...-unused_receiver` expects a non-empty wrapper name. docs/language-v0.1.md says each adapter evaluation returns an
  anonymous function, and that the fused spelling keeps it anonymous where JavaScript would infer a name. The `.out`
  files keep the test's values as instructed. The semantic route is spec-conformant on `-aggregate` (it fails) and
  not on `-unused_receiver` (it passes). Decide which rule is right before relying on either case.

## Cases

| Case | Source test | Semantic | Legacy | Files | Note |
|---|---|---|---|---|---|
| `irjs-js_string_of_a_jsvalue_still_stringifies_under_compression` | `js_string_of_a_jsvalue_still_stringifies_under_compression` (src/codegen_ir_js.rs:27677) | pass | pass | .lil .out .host.js | JS.string(JsValue) must stay ToString (test ran with elide_safe_string_coercions on) |
| `irjs-merge_empty_second_keeps_destination_length` | `merge_empty_second_keeps_destination_length` (src/codegen_ir_js.rs:27813) | pass | pass | .lil .out .host.js | test swept mutation/loop/phi-affinity spellings; one program |
| `irjs-escaped_arrays_keep_borrowed_builtin_method_calls` | `escaped_arrays_keep_borrowed_builtin_method_calls` (src/codegen_ir_js.rs:27890) | pass | pass | .lil .out .host.js |  |
| `irjs-record_literals_have_no_inherited_or_proto_setter_keys` | `record_literals_have_no_inherited_or_proto_setter_keys` (src/codegen_ir_js.rs:28218) | pass | pass | .lil .out |  |
| `irjs-emits_conservatively_proven_regex_literals_and_preserves_effectful_fallbacks` | `emits_conservatively_proven_regex_literals_and_preserves_effectful_fallbacks` (src/codegen_ir_js.rs:28337) | pass | pass | .lil .out | only the executed back-reference program; test ran it with regex_literals on |
| `irjs-adjacent_binding_merge_ignores_names_inside_nested_initializers` | `adjacent_binding_merge_ignores_names_inside_nested_initializers` (src/codegen_ir_js.rs:28876) | pass | pass | .lil .out .host.js |  |
| `irjs-does_not_rematerialize_short_circuit_member_before_receiver_bind` | `does_not_rematerialize_short_circuit_member_before_receiver_bind` (src/codegen_ir_js.rs:29037) | pass | pass | .lil .out .host.js | module test; exported call moved into the program |
| `irjs-does_not_reuse_a_loop_receiver_for_a_nested_counter` | `does_not_reuse_a_loop_receiver_for_a_nested_counter` (src/codegen_ir_js.rs:29082) | pass | **compile-error** | .lil .out .host.js | module test; exported call moved into the program; legacy: CLI output fails its own parser admission ("Unexpected token") |
| `irjs-uncoalesced_local_candidate_can_avoid_reassignment_syntax` | `uncoalesced_local_candidate_can_avoid_reassignment_syntax` (src/codegen_ir_js.rs:29186); `local_coalescing_switch_is_inert_for_unmangled_output` (src/codegen_ir_js.rs:29207) | pass | **compile-error** | .lil .out | same program as local_coalescing_switch_is_inert_for_unmangled_output (29207); legacy: CLI output fails its own parser admission ("Unexpected token") |
| `irjs-materializes_a_short_circuit_value_reused_inside_an_expression_region` | `materializes_a_short_circuit_value_reused_inside_an_expression_region` (src/codegen_ir_js.rs:29351) | pass | pass | .lil .out .host.js |  |
| `irjs-materializes_a_reused_value_inside_one_expression_region` | `materializes_a_reused_value_inside_one_expression_region` (src/codegen_ir_js.rs:29369) | pass | pass | .lil .out .host.js |  |
| `irjs-computed_assignment_keeps_lhs_and_lazy_rhs_evaluation_order-first0` | `computed_assignment_keeps_lhs_and_lazy_rhs_evaluation_order` (src/codegen_ir_js.rs:29399) | pass | pass | .lil .out .host.js |  |
| `irjs-computed_assignment_keeps_lhs_and_lazy_rhs_evaluation_order-first5` | `computed_assignment_keeps_lhs_and_lazy_rhs_evaluation_order` (src/codegen_ir_js.rs:29399) | pass | pass | .lil .out .host.js |  |
| `irjs-keeps_short_circuit_lhs_property_read_before_an_intervening_mutating_call` | `keeps_short_circuit_lhs_property_read_before_an_intervening_mutating_call` (src/codegen_ir_js.rs:29442) | pass | pass | .lil .out |  |
| `irjs-fuses_constructable_method_tables_into_javascript_classes` | `fuses_constructable_method_tables_into_javascript_classes` (src/codegen_ir_js.rs:29469) | pass | pass | .lil .out | test executed the js_peephole class-fold of the output; expectation is the program's |
| `irjs-reconstructs_nullish_expression_phis` | `reconstructs_nullish_expression_phis` (src/codegen_ir_js.rs:29506) | pass | pass | .lil .out .host.js |  |
| `irjs-reconstructs_effectful_collection_reads_after_prior_statements` | `reconstructs_effectful_collection_reads_after_prior_statements` (src/codegen_ir_js.rs:29522) | pass | pass | .lil .out |  |
| `irjs-folds_redundant_record_miss_normalization_into_nullish_fallback` | `folds_redundant_record_miss_normalization_into_nullish_fallback` (src/codegen_ir_js.rs:29534) | pass | pass | .lil .out |  |
| `irjs-folds_redundant_record_miss_normalization_into_nullish_fallback-computed` | `folds_redundant_record_miss_normalization_into_nullish_fallback` (src/codegen_ir_js.rs:29534) | pass | pass | .lil .out |  |
| `irjs-emits_source_expression_if_as_a_conditional_region` | `emits_source_expression_if_as_a_conditional_region` (src/codegen_ir_js.rs:29567) | pass | pass | .lil .out .host.js |  |
| `irjs-emits_scalar_match_as_a_conditional_region` | `emits_scalar_match_as_a_conditional_region` (src/codegen_ir_js.rs:29584) | pass | pass | .lil .out .host.js |  |
| `irjs-reconstructs_effectful_optional_index_with_lazy_fallback` | `reconstructs_effectful_optional_index_with_lazy_fallback` (src/codegen_ir_js.rs:29601) | pass | pass | .lil .out .host.js |  |
| `irjs-reconstructs_optional_member_with_lazy_fallback` | `reconstructs_optional_member_with_lazy_fallback` (src/codegen_ir_js.rs:29618) | pass | pass | .lil .out .host.js |  |
| `irjs-nests_observable_array_elements_across_inert_literals` | `nests_observable_array_elements_across_inert_literals` (src/codegen_ir_js.rs:29657) | pass | pass | .lil .out .host.js | the executed nested_rows program; test ran it with aggregate operand fusion on |
| `irjs-aggregate_operand_fusion_materializes_observable_chained_reads` | `aggregate_operand_fusion_materializes_observable_chained_reads` (src/codegen_ir_js.rs:29725) | pass | pass | .lil .out .host.js |  |
| `irjs-same_block_observable_forwarding_crosses_only_a_constant_gap` | `same_block_observable_forwarding_crosses_only_a_constant_gap` (src/codegen_ir_js.rs:29781) | pass | pass | .lil .out .host.js |  |
| `irjs-same_block_observable_forwarding_crosses_an_ordinary_closure_gap` | `same_block_observable_forwarding_crosses_an_ordinary_closure_gap` (src/codegen_ir_js.rs:29806) | pass | pass | .lil .out .host.js |  |
| `irjs-same_block_observable_forwarding_refuses_call_and_read_barriers-call` | `same_block_observable_forwarding_refuses_call_and_read_barriers` (src/codegen_ir_js.rs:29829) | pass | pass | .lil .out .host.js |  |
| `irjs-same_block_observable_forwarding_refuses_call_and_read_barriers-read` | `same_block_observable_forwarding_refuses_call_and_read_barriers` (src/codegen_ir_js.rs:29829) | pass | pass | .lil .out .host.js |  |
| `irjs-same_block_observable_forwarding_keeps_state_machine_liveness_consistent` | `same_block_observable_forwarding_keeps_state_machine_liveness_consistent` (src/codegen_ir_js.rs:29872) | pass | pass | .lil .out .host.js | test forced the state-machine spelling; one program |
| `irjs-local_phi_expression_keeps_outputless_arm_stores` | `local_phi_expression_keeps_outputless_arm_stores` (src/codegen_ir_js.rs:29938) | pass | pass | .lil .out .host.js |  |
| `irjs-loop_capture_wrapper_params_do_not_shadow_mutable_cells` | `loop_capture_wrapper_params_do_not_shadow_mutable_cells` (src/codegen_ir_js.rs:29979) | pass | pass | .lil .out |  |
| `irjs-inner_temps_do_not_clobber_module_bindings_used_as_exports` | `inner_temps_do_not_clobber_module_bindings_used_as_exports` (src/codegen_ir_js.rs:29996) | pass | pass | .lil .out |  |
| `irjs-arrow_locals_do_not_var_hoist_over_parent_global_reads` | `arrow_locals_do_not_var_hoist_over_parent_global_reads` (src/codegen_ir_js.rs:30061) | pass | pass | .lil .out .host.js |  |
| `irjs-nested_captured_locals_use_reserved_short_names` | `nested_captured_locals_use_reserved_short_names` (src/codegen_ir_js.rs:30091) | pass | pass | .lil .out |  |
| `irjs-unreferenced_module_bindings_can_be_shadowed_by_short_local_names` | `unreferenced_module_bindings_can_be_shadowed_by_short_local_names` (src/codegen_ir_js.rs:30114) | pass | pass | .lil .out .host.js |  |
| `irjs-precise_shadowing_can_reserve_a_repeated_local_name_prefix` | `precise_shadowing_can_reserve_a_repeated_local_name_prefix` (src/codegen_ir_js.rs:30138) | pass | pass | .lil .out .host.js |  |
| `irjs-transitive_nested_shadowing_reuses_only_unreferenced_function_names` | `transitive_nested_shadowing_reuses_only_unreferenced_function_names` (src/codegen_ir_js.rs:30164) | pass | pass | .lil .out .host.js |  |
| `irjs-transitive_nested_shadowing_reuses_only_unreferenced_function_names-referenced` | `transitive_nested_shadowing_reuses_only_unreferenced_function_names` (src/codegen_ir_js.rs:30164) | pass | pass | .lil .out .host.js |  |
| `irjs-transitive_nested_shadowing_keeps_rest_formals_off_defaulted_locals` | `transitive_nested_shadowing_keeps_rest_formals_off_defaulted_locals` (src/codegen_ir_js.rs:30214) | pass | pass | .lil .out .host.js | prelude keeps the last kept function; old harness picked it by .length>=3, a legacy arity the language spec forbids (methodRest length is 0) |
| `irjs-nested_body_still_reads_the_outer_binding_an_inner_local_must_not_steal` | `nested_body_still_reads_the_outer_binding_an_inner_local_must_not_steal` (src/codegen_ir_js.rs:30263) | pass | pass | .lil .out .host.js | test swept three naming layouts with and without inlining; one program |
| `irjs-nested_body_still_reads_the_array_prototype_alias` | `nested_body_still_reads_the_array_prototype_alias` (src/codegen_ir_js.rs:30335) | pass | pass | .lil .out .host.js |  |
| `irjs-nested_body_still_reads_the_host_alias_method_call` | `nested_body_still_reads_the_host_alias_method_call` (src/codegen_ir_js.rs:30378) | pass | pass | .lil .out .host.js | host defines objectToStringTag (old test relied on name-based host-alias lowering) |
| `irjs-nested_for_init_array_must_not_overwrite_an_outer_function` | `nested_for_init_array_must_not_overwrite_an_outer_function` (src/codegen_ir_js.rs:30419) | pass | pass | .lil .out .host.js |  |
| `irjs-caller_local_must_not_steal_a_name_a_callee_still_calls` | `caller_local_must_not_steal_a_name_a_callee_still_calls` (src/codegen_ir_js.rs:30469) | pass | pass | .lil .out .host.js |  |
| `irjs-promoted_rest_formals_do_not_split_coalesced_default_locals` | `promoted_rest_formals_do_not_split_coalesced_default_locals` (src/codegen_ir_js.rs:30512) | pass | pass | .lil .out .host.js |  |
| `irjs-precise_shadowing_does_not_let_host_locals_steal_nested_helper_names` | `precise_shadowing_does_not_let_host_locals_steal_nested_helper_names` (src/codegen_ir_js.rs:30563) | pass | pass | .lil .out |  |
| `irjs-precise_shadowing_keeps_helpers_used_by_inlined_module_record_traps` | `precise_shadowing_keeps_helpers_used_by_inlined_module_record_traps` (src/codegen_ir_js.rs:30596) | pass | pass | .lil .out |  |
| `irjs-snapshot_of_a_mutable_capture_survives_a_nested_store` | `snapshot_of_a_mutable_capture_survives_a_nested_store` (src/codegen_ir_js.rs:30629) | pass | pass | .lil .out |  |
| `irjs-snapshot_of_a_mutable_capture_survives_an_indirect_nested_store` | `snapshot_of_a_mutable_capture_survives_an_indirect_nested_store` (src/codegen_ir_js.rs:30682) | pass | pass | .lil .out |  |
| `irjs-nested_function_expressions_do_not_reuse_a_captured_module_name` | `nested_function_expressions_do_not_reuse_a_captured_module_name` (src/codegen_ir_js.rs:30726) | pass | pass | .lil .out |  |
| `irjs-nested_locals_do_not_reuse_an_implicit_capture_name` | `nested_locals_do_not_reuse_an_implicit_capture_name` (src/codegen_ir_js.rs:30742) | pass | pass | .lil .out .host.js |  |
| `irjs-parenthesizes_nullish_and_logical_mixes` | `parenthesizes_nullish_and_logical_mixes` (src/codegen_ir_js.rs:30780) | pass | pass | .lil .out .host.js |  |
| `irjs-loop_index_copied_from_a_call_keeps_progress_when_a_sibling_captures_it` | `loop_index_copied_from_a_call_keeps_progress_when_a_sibling_captures_it` (src/codegen_ir_js.rs:31218) | pass | pass | .lil .out |  |
| `irjs-local_phi_expression_does_not_read_a_coalesced_input_before_its_definition` | `local_phi_expression_does_not_read_a_coalesced_input_before_its_definition` (src/codegen_ir_js.rs:31347) | pass | pass | .lil .out .host.js |  |
| `irjs-later_field_reads_do_not_index_a_previous_extract` | `later_field_reads_do_not_index_a_previous_extract` (src/codegen_ir_js.rs:31769) | pass | pass | .lil .out |  |
| `irjs-later_field_reads_do_not_index_a_previous_extract-strings` | `later_field_reads_do_not_index_a_previous_extract` (src/codegen_ir_js.rs:31769) | pass | pass | .lil .out | test also ran the js_peephole output |
| `irjs-later_field_reads_do_not_index_a_previous_extract-walked` | `later_field_reads_do_not_index_a_previous_extract` (src/codegen_ir_js.rs:31769) | pass | pass | .lil .out | test also ran the js_peephole output |
| `irjs-dissolves_internal_scale_class_without_es_class` | `dissolves_internal_scale_class_without_es_class` (src/codegen_ir_js.rs:32027); `identity_observed_constructor_emits_named_class` (src/codegen_ir_js.rs:32073) | pass | pass | .lil .out | same program as identity_observed_constructor_emits_named_class (32073, which forced identity_observed on the IR) |
| `irjs-public_class_defaults_preserve_omitted_calls_and_method_arity` | `public_class_defaults_preserve_omitted_calls_and_method_arity` (src/codegen_ir_js.rs:32124) | **compile-error** | pass | .lil .out | partial: module ABI test; arity/host-omission checks not expressible, value calls moved into the program; semantic: refuses `export constructor` ("direct module checking does not yet support nominal constructor exports") |
| `irjs-fuses_partial_constant_constructor_initializers_at_named_escape_boundaries` | `fuses_partial_constant_constructor_initializers_at_named_escape_boundaries` (src/codegen_ir_js.rs:32212) | pass | pass | .lil .out .host.js |  |
| `irjs-fuses_constructor_fields_that_share_one_constant` | `fuses_constructor_fields_that_share_one_constant` (src/codegen_ir_js.rs:32233) | pass | pass | .lil .out |  |
| `irjs-fuses_constructor_defaults_then_overwritten_fields` | `fuses_constructor_defaults_then_overwritten_fields` (src/codegen_ir_js.rs:32250) | pass | pass | .lil .out |  |
| `irjs-fuses_partial_positional_and_complete_constant_initializers` | `fuses_partial_positional_and_complete_constant_initializers` (src/codegen_ir_js.rs:32274) | pass | pass | .lil .out | positional program (test set public_aggregate_fields=false); its `.toml` was removed in M1.4, which refuses the positional ABI (D2): the output never read a slot, so the case runs under the named ABI |
| `irjs-fuses_partial_positional_and_complete_constant_initializers-named` | `fuses_partial_positional_and_complete_constant_initializers` (src/codegen_ir_js.rs:32274) | pass | pass | .lil .out .host.js |  |
| `irjs-directly_fuses_constructor_parameters_once_in_left_to_right_order` | `directly_fuses_constructor_parameters_once_in_left_to_right_order` (src/codegen_ir_js.rs:32309) | pass | pass | .lil .out .host.js | `.toml` (positional ABI) removed in M1.4, which refuses it (D2); the host reads no slot |
| `irjs-direct_constructor_literals_preserve_named_and_positional_own_properties-named` | `direct_constructor_literals_preserve_named_and_positional_own_properties` (src/codegen_ir_js.rs:32330) | pass | pass | .lil .out .host.js |  |
| `irjs-direct_constructor_literals_preserve_named_and_positional_own_properties-positional` | `direct_constructor_literals_preserve_named_and_positional_own_properties` (src/codegen_ir_js.rs:32330) | **FAIL** | pass | .lil .out .host.js .toml | semantic: ignores `public_aggregate_abi = "positional"`: the instance reaches the extern as `{value:7}`, so `slot[0]` is undefined (first diff: line 1: expected 'TRACE:0:true:7:1' got 'TRACE:0:false:undefined:undefined') |
| `irjs-direct_constructor_fusion_refuses_observable_host_defaults` | `direct_constructor_fusion_refuses_observable_host_defaults` (src/codegen_ir_js.rs:32371) | **FAIL** | pass | .lil .out .host.js .toml | semantic: ignores `public_aggregate_abi = "positional"` (`cache[0]` undefined, TypeError); it also builds the `new Map` field default before calling `supplied()` (`b={values:new Map},a(b,supplied())`) (first diff: exit 1: TypeError: Cannot read properties of undefined (reading 'tag'); line 1: expected 'TRACE:1:true' got 'TRACE:1:false') |
| `irjs-direct_constructor_fusion_refuses_unmapped_host_defaults` | `direct_constructor_fusion_refuses_unmapped_host_defaults` (src/codegen_ir_js.rs:32393) | **FAIL** | pass | .lil .out .host.js .toml | semantic: two bugs: the field default `new Map` runs before the constructor argument `read()` (`b={guard:new Map,value:0},a(b,read())`, also under the named ABI), and `public_aggregate_abi = "positional"` is ignored (`cache[1]` undefined) (first diff: line 1: expected 'TRACE:read,map,inspect:7' got 'TRACE:map,read,inspect:undefined') |
| `irjs-an_internal_class_extending_a_host_class_is_a_real_subclass` | `an_internal_class_extending_a_host_class_is_a_real_subclass` (src/codegen_ir_js.rs:32418) | pass | pass | .lil .out |  |
| `irjs-direct_constructor_fusion_keeps_helpers_with_remaining_super_uses` | `direct_constructor_fusion_keeps_helpers_with_remaining_super_uses` (src/codegen_ir_js.rs:32430) | pass | pass | .lil .out .host.js | `.toml` (positional ABI) removed in M1.4, which refuses it (D2); the host reads no slot |
| `irjs-retains_effectful_and_observed_constructor_field_writes-effectful` | `retains_effectful_and_observed_constructor_field_writes` (src/codegen_ir_js.rs:32451) | pass | pass | .lil .out .host.js |  |
| `irjs-retains_effectful_and_observed_constructor_field_writes-observed` | `retains_effectful_and_observed_constructor_field_writes` (src/codegen_ir_js.rs:32451) | pass | pass | .lil .out .host.js |  |
| `irjs-constructor_fusion_survives_rejected_loop_update_probe` | `constructor_fusion_survives_rejected_loop_update_probe` (src/codegen_ir_js.rs:32481) | pass | pass | .lil .out .host.js |  |
| `irjs-dense_constructor_literals_ignore_inherited_prototype_setters-named` | `dense_constructor_literals_ignore_inherited_prototype_setters` (src/codegen_ir_js.rs:32501) | pass | pass | .lil .out .host.js |  |
| `irjs-dense_constructor_literals_ignore_inherited_prototype_setters-positional` | `dense_constructor_literals_ignore_inherited_prototype_setters` (src/codegen_ir_js.rs:32501) | **FAIL** | pass | .lil .out .host.js .toml | semantic: ignores `public_aggregate_abi = "positional"`: the instance reaches the extern as `{value:7}`, so `box[0]` is undefined (first diff: line 1: expected 'TRACE:0:true:7:1' got 'TRACE:0:false:undefined:undefined') |
| `irjs-named_proto_fields_are_own_data_properties-class` | `named_proto_fields_are_own_data_properties` (src/codegen_ir_js.rs:32549); `prototype_sensitive_fields_remain_own_properties_when_mangling` (src/codegen_ir_js.rs:32822) | pass | pass | .lil .out .host.js | same program as prototype_sensitive_fields_remain_own_properties_when_mangling (32822, property mangling on) |
| `irjs-named_proto_fields_are_own_data_properties-struct` | `named_proto_fields_are_own_data_properties` (src/codegen_ir_js.rs:32549) | **compile-error** | pass | .lil .out .host.js | semantic: refuses a struct passed to an extern: Unsupported "foreign value-struct storage adaptation" |
| `irjs-property_mangling_targets_skip_short_public_field_names` | `property_mangling_targets_skip_short_public_field_names` (src/codegen_ir_js.rs:32580) | pass | pass | .lil .out |  |
| `irjs-unrelated_owned_property_components_reuse_short_names` | `unrelated_owned_property_components_reuse_short_names` (src/codegen_ir_js.rs:32655) | pass | pass | .lil .out |  |
| `irjs-extern_property_spelling_does_not_pin_an_unrelated_owned_slot` | `extern_property_spelling_does_not_pin_an_unrelated_owned_slot` (src/codegen_ir_js.rs:32678) | pass | pass | .lil .out .host.js |  |
| `irjs-unowned_static_keys_only_coordinate_with_owned_slots_in_closed_mode` | `unowned_static_keys_only_coordinate_with_owned_slots_in_closed_mode` (src/codegen_ir_js.rs:32703) | pass | pass | .lil .out .host.js |  |
| `irjs-unowned_static_keys_only_coordinate_with_owned_slots_in_closed_mode-released_extern_fields` | `unowned_static_keys_only_coordinate_with_owned_slots_in_closed_mode` (src/codegen_ir_js.rs:32703) | pass | pass | .lil .out .host.js .toml | same program under the released extern-field contract (test: mangle_extern_fields=false) |
| `irjs-immutable_closure_captures_can_use_lifted_scalar_snapshots` | `immutable_closure_captures_can_use_lifted_scalar_snapshots` (src/codegen_ir_js.rs:32889) | pass | pass | .lil .out .host.js |  |
| `irjs-mutable_closure_captures_remain_shared_lexical_cells` | `mutable_closure_captures_remain_shared_lexical_cells` (src/codegen_ir_js.rs:32924) | pass | pass | .lil .out .host.js |  |
| `irjs-invoked_sibling_closures_share_their_mutable_capture_cell` | `invoked_sibling_closures_share_their_mutable_capture_cell` (src/codegen_ir_js.rs:33042) | pass | pass | .lil .out |  |
| `irjs-emits_exclusive_recursive_local_as_a_named_function_expression_iife` | `emits_exclusive_recursive_local_as_a_named_function_expression_iife` (src/codegen_ir_js.rs:33089) | pass | pass | .lil .out |  |
| `irjs-emits_exclusive_recursive_local_called_from_a_nested_closure_as_iife` | `emits_exclusive_recursive_local_called_from_a_nested_closure_as_iife` (src/codegen_ir_js.rs:33112) | pass | pass | .lil .out .host.js | the harness's walk([[1],[2,3],4]) call moved into the program |
| `irjs-writes_a_captured_or_assign_from_a_property_read_back_to_the_cell` | `writes_a_captured_or_assign_from_a_property_read_back_to_the_cell` (src/codegen_ir_js.rs:33210) | pass | pass | .lil .out .host.js | module test; exported calls moved into the program |
| `irjs-captured_then_body_is_not_overwritten_by_a_later_host_load` | `captured_then_body_is_not_overwritten_by_a_later_host_load` (src/codegen_ir_js.rs:33237) | pass | pass | .lil .out .host.js |  |
| `irjs-short_circuit_and_does_not_overwrite_an_object_used_after` | `short_circuit_and_does_not_overwrite_an_object_used_after` (src/codegen_ir_js.rs:33289) | pass | pass | .lil .out .host.js | expects empty stdout and a clean exit |
| `irjs-captured_options_keep_their_name_off_the_closure_dest` | `captured_options_keep_their_name_off_the_closure_dest` (src/codegen_ir_js.rs:33311) | pass | pass | .lil .out .host.js | module test; exported calls moved into the program |
| `irjs-fuses_apply_index_operands_in_evaluation_order` | `fuses_apply_index_operands_in_evaluation_order` (src/codegen_ir_js.rs:33411) | pass | pass | .lil .out .host.js |  |
| `irjs-reconstructs_optional_method_reassign_as_ternary` | `reconstructs_optional_method_reassign_as_ternary` (src/codegen_ir_js.rs:33534) | pass | pass | .lil .out .host.js | module test; exported calls moved into the program |
| `irjs-keeps_module_regex_off_isxmldoc_short_circuit_lhs` | `keeps_module_regex_off_isxmldoc_short_circuit_lhs` (src/codegen_ir_js.rs:33597) | pass | pass | .lil .out .host.js | module test; exported call moved into the program |
| `irjs-assigns_for_in_copy_before_proto_guard` | `assigns_for_in_copy_before_proto_guard` (src/codegen_ir_js.rs:33644) | pass | pass | .lil .out .host.js | module test; exported call moved into the program |
| `irjs-keeps_shared_short_circuit_lhs_off_its_results` | `keeps_shared_short_circuit_lhs_off_its_results` (src/codegen_ir_js.rs:33772) | pass | pass | .lil .out .host.js | module test; exported call moved into the program |
| `irjs-preserves_captured_receiver_by_aliasing_this` | `preserves_captured_receiver_by_aliasing_this` (src/codegen_ir_js.rs:33825) | pass | pass | .lil .out .host.js |  |
| `irjs-assigned_this_methods_keep_named_formals_instead_of_arguments` | `assigned_this_methods_keep_named_formals_instead_of_arguments` (src/codegen_ir_js.rs:33868) | pass | pass | .lil .out .host.js |  |
| `irjs-fused_method2_keeps_named_formals_instead_of_arguments` | `fused_method2_keeps_named_formals_instead_of_arguments` (src/codegen_ir_js.rs:33884) | pass | pass | .lil .out .host.js |  |
| `irjs-fused_method3_keeps_named_formals_instead_of_arguments` | `fused_method3_keeps_named_formals_instead_of_arguments` (src/codegen_ir_js.rs:33907) | pass | pass | .lil .out .host.js |  |
| `irjs-emits_typed_string_and_regex_javascript_members` | `emits_typed_string_and_regex_javascript_members` (src/codegen_ir_js.rs:34015) | pass | pass | .lil .out |  |
| `irjs-if_return_regex_choice_emits_a_javascript_ternary` | `if_return_regex_choice_emits_a_javascript_ternary` (src/codegen_ir_js.rs:34056) | pass | pass | .lil .out .host.js |  |
| `irjs-grouping_keeps_inlined_int_length_from_stealing_subtract` | `grouping_keeps_inlined_int_length_from_stealing_subtract` (src/codegen_ir_js.rs:34072) | pass | pass | .lil .out |  |
| `irjs-wraps_exclusive_callee_trees_in_once_run_iife_scopes` | `wraps_exclusive_callee_trees_in_once_run_iife_scopes` (src/codegen_ir_js.rs:34280) | pass | pass | .lil .out | clustered program |
| `irjs-wraps_exclusive_callee_trees_in_once_run_iife_scopes-mangled` | `wraps_exclusive_callee_trees_in_once_run_iife_scopes` (src/codegen_ir_js.rs:34280) | pass | pass | .lil .out |  |
| `irjs-wraps_exclusive_callee_trees_in_once_run_iife_scopes-shared` | `wraps_exclusive_callee_trees_in_once_run_iife_scopes` (src/codegen_ir_js.rs:34280) | pass | pass | .lil .out | shared and shared_mangled programs (identical source) |
| `irjs-wraps_helpers_reached_only_through_exclusive_nested_closures` | `wraps_helpers_reached_only_through_exclusive_nested_closures` (src/codegen_ir_js.rs:34356) | pass | pass | .lil .out |  |
| `irjs-clustered_root_params_do_not_shadow_helpers_captured_by_nested_closures` | `clustered_root_params_do_not_shadow_helpers_captured_by_nested_closures` (src/codegen_ir_js.rs:34383) | pass | pass | .lil .out .host.js |  |
| `irjs-clustered_helper_locals_do_not_shadow_sibling_helpers` | `clustered_helper_locals_do_not_shadow_sibling_helpers` (src/codegen_ir_js.rs:34403) | pass | pass | .lil .out |  |
| `irjs-named_cluster_roots_stay_callable_from_foreign_closures` | `named_cluster_roots_stay_callable_from_foreign_closures` (src/codegen_ir_js.rs:34420) | pass | pass | .lil .out |  |
| `irjs-named_cluster_helpers_called_from_a_foreign_emit_body_keep_names` | `named_cluster_helpers_called_from_a_foreign_emit_body_keep_names` (src/codegen_ir_js.rs:34438) | pass | pass | .lil .out |  |
| `irjs-overlapping_named_cluster_trees_have_one_final_helper_owner` | `overlapping_named_cluster_trees_have_one_final_helper_owner` (src/codegen_ir_js.rs:34455) | pass | pass | .lil .out .host.js |  |
| `irjs-clustered_helpers_of_js_method_binders_keep_emitted_names` | `clustered_helpers_of_js_method_binders_keep_emitted_names` (src/codegen_ir_js.rs:34610) | pass | pass | .lil .out .host.js |  |
| `irjs-public_class_members_do_not_strand_private_helper_bindings` | `public_class_members_do_not_strand_private_helper_bindings` (src/codegen_ir_js.rs:34629) | **compile-error** | pass | .lil .out | module test; the JavaScript construction moved into the program; semantic: refuses `export constructor` ("direct module checking does not yet support nominal constructor exports") |
| `irjs-type_annotations_do_not_emit_runtime_validation` | `type_annotations_do_not_emit_runtime_validation` (src/codegen_ir_js.rs:34703) | pass | pass | .lil .out | module test; the five exported calls moved into one printed line |
| `irjs-loop_index_survives_nullable_map_get_in_the_body` | `loop_index_survives_nullable_map_get_in_the_body` (src/codegen_ir_js.rs:34718) | pass | pass | .lil .out |  |
| `irjs-nests_helpers_shared_by_once_run_exclusive_closures` | `nests_helpers_shared_by_once_run_exclusive_closures` (src/codegen_ir_js.rs:34760) | pass | pass | .lil .out .host.js |  |
| `irjs-nests_address_taken_helpers_private_to_a_once_run_host` | `nests_address_taken_helpers_private_to_a_once_run_host` (src/codegen_ir_js.rs:34804) | pass | pass | .lil .out .host.js |  |
| `irjs-emits_statement_if_returns_as_a_conditional_expression` | `emits_statement_if_returns_as_a_conditional_expression` (src/codegen_ir_js.rs:34890) | pass | pass | .lil .out .host.js |  |
| `irjs-collapses_assignment_guard_returns_into_one_if` | `collapses_assignment_guard_returns_into_one_if` (src/codegen_ir_js.rs:34947) | pass | pass | .lil .out .host.js |  |
| `irjs-folds_fresh_empty_object_writes_into_a_literal` | `folds_fresh_empty_object_writes_into_a_literal` (src/codegen_ir_js.rs:34971) | pass | pass | .lil .out .host.js .toml | test assumed pristine builtins |
| `irjs-preserves_fresh_object_assignments_without_pristine_builtins` | `preserves_fresh_object_assignments_without_pristine_builtins` (src/codegen_ir_js.rs:34993) | pass | pass | .lil .out .host.js .toml | test ran without the pristine-builtins assumption (explicit here) |
| `irjs-folds_fresh_object_writes_across_unrelated_prefix` | `folds_fresh_object_writes_across_unrelated_prefix` (src/codegen_ir_js.rs:35011) | pass | pass | .lil .out .host.js .toml | test assumed pristine builtins |
| `irjs-batches_consecutive_property_writes_into_object_assign` | `batches_consecutive_property_writes_into_object_assign` (src/codegen_ir_js.rs:35033) | pass | pass | .lil .out .host.js |  |
| `irjs-property_write_batching_emits_sunk_function_declarations` | `property_write_batching_emits_sunk_function_declarations` (src/codegen_ir_js.rs:35073) | pass | pass | .lil .out .host.js |  |
| `irjs-emits_negated_empty_else_as_or_statement-keep_false` | `emits_negated_empty_else_as_or_statement` (src/codegen_ir_js.rs:35096) | pass | pass | .lil .out .host.js |  |
| `irjs-emits_negated_empty_else_as_or_statement-keep_true` | `emits_negated_empty_else_as_or_statement` (src/codegen_ir_js.rs:35096) | pass | pass | .lil .out .host.js |  |
| `irjs-preserves_adapter_for_stored_or_mutably_captured_replaced_parameters` | `preserves_adapter_for_stored_or_mutably_captured_replaced_parameters` (src/codegen_ir_js.rs:35123) | pass | pass | .lil .out .host.js |  |
| `irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-aggregate` | `suppresses_adapter_name_in_variable_and_aggregate_initializers` (src/codegen_ir_js.rs:35170) | **FAIL** | pass | .lil .out .host.js | CONFLICT: test expects the inferred name 'handle'; docs/language-v0.1.md says adapter wrappers stay anonymous; semantic: keeps the wrapper anonymous, as the spec requires (expectation conflict, not a semantic bug) (first diff: line 1: expected 'TRACE:handle' got 'TRACE:') |
| `irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-local` | `suppresses_adapter_name_in_variable_and_aggregate_initializers` (src/codegen_ir_js.rs:35170) | pass | pass | .lil .out .host.js |  |
| `irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-unused_receiver` | `suppresses_adapter_name_in_variable_and_aggregate_initializers` (src/codegen_ir_js.rs:35170) | pass | pass | .lil .out .host.js | CONFLICT: test expects a non-empty (inferred) wrapper name; docs/language-v0.1.md says adapter wrappers stay anonymous |
| `irjs-inlines_eraseable_host_getters_at_constant_identifier_keys` | `inlines_eraseable_host_getters_at_constant_identifier_keys` (src/codegen_ir_js.rs:35236) | pass | pass | .lil .out .host.js |  |
| `irjs-nested_lexical_js_bindings_keep_the_callback_context-arguments` | `nested_lexical_js_bindings_keep_the_callback_context` (src/codegen_ir_js.rs:35294) | pass | **FAIL** | .lil .out .host.js | program body wrapped in a LilScript function (the test wrapped the output in a JS function); legacy: inlines `ambient`, so `arguments` becomes the CommonJS wrapper's (first diff: line 1: expected 'TRACE:ambient:4:true' got 'TRACE:[object Object]:4:true') |
| `irjs-nested_lexical_js_bindings_keep_the_callback_context-this` | `nested_lexical_js_bindings_keep_the_callback_context` (src/codegen_ir_js.rs:35294) | pass | pass | .lil .out .host.js | program reports its top-level this (the test chose it by wrapping the output in a function) |
| `irjs-fallback_adapters_preserve_javascript_function_reflection_and_arguments` | `fallback_adapters_preserve_javascript_function_reflection_and_arguments` (src/codegen_ir_js.rs:35316) | pass | pass | .lil .out .host.js |  |
| `irjs-fallback_method10_preserves_receiver_arguments_evaluation_and_reflection` | `fallback_method10_preserves_receiver_arguments_evaluation_and_reflection` (src/codegen_ir_js.rs:35331) | pass | pass | .lil .out .host.js |  |
| `irjs-fused_adapters_still_allocate_fresh_constructible_anonymous_functions` | `fused_adapters_still_allocate_fresh_constructible_anonymous_functions` (src/codegen_ir_js.rs:35346) | pass | pass | .lil .out .host.js |  |
| `irjs-fused_method10_preserves_receiver_arity_identity_and_arguments` | `fused_method10_preserves_receiver_arity_identity_and_arguments` (src/codegen_ir_js.rs:35358) | pass | pass | .lil .out .host.js |  |
| `irjs-caller_materializes_fresh_array_defaults_for_direct_only_functions` | `caller_materializes_fresh_array_defaults_for_direct_only_functions` (src/codegen_ir_js.rs:35752) | pass | pass | .lil .out |  |
| `irjs-address_taken_defaults_keep_plain_typed_formals` | `address_taken_defaults_keep_plain_typed_formals` (src/codegen_ir_js.rs:35790) | pass | pass | .lil .out .host.js |  |
| `irjs-exported_js_undefined_default_uses_host_omission_without_shortening_length` | `exported_js_undefined_default_uses_host_omission_without_shortening_length` (src/codegen_ir_js.rs:35810) | pass | pass | .lil .out | partial: the harness's read()===void 0 and read.length on the exported binding are not expressible in a script case |
| `irjs-omits_exported_trailing_undefined_default_at_internal_calls` | `omits_exported_trailing_undefined_default_at_internal_calls` (src/codegen_ir_js.rs:35845) | pass | pass | .lil .out .host.js | partial: module test; exported call moved into the program, lookup.length check dropped |
| `irjs-scans_indexof_results_with_uninitialized_from_index` | `scans_indexof_results_with_uninitialized_from_index` (src/codegen_ir_js.rs:35877) | pass | pass | .lil .out .host.js | module test; exported call moved into the program |
| `irjs-accepts_devirtualized_function_values_with_caller_materialized_defaults` | `accepts_devirtualized_function_values_with_caller_materialized_defaults` (src/codegen_ir_js.rs:35905) | pass | pass | .lil .out |  |
| `irjs-direct_only_global_identifier_default_stays_caller_materialized` | `direct_only_global_identifier_default_stays_caller_materialized` (src/codegen_ir_js.rs:35943) | pass | pass | .lil .out |  |
| `irjs-bound_default_identity_reuses_an_earlier_arrow_actual` | `bound_default_identity_reuses_an_earlier_arrow_actual` (src/codegen_ir_js.rs:35984) | pass | pass | .lil .out |  |
| `irjs-captured_closure_wrappers_keep_plain_typed_formals` | `captured_closure_wrappers_keep_plain_typed_formals` (src/codegen_ir_js.rs:36022) | pass | pass | .lil .out .host.js |  |
| `irjs-default_identity_contract_accepts_matching_generic_results` | `default_identity_contract_accepts_matching_generic_results` (src/codegen_ir_js.rs:36052) | pass | pass | .lil .out |  |
| `irjs-default_arrow_capture_contract_allows_global_captures` | `default_arrow_capture_contract_allows_global_captures` (src/codegen_ir_js.rs:36089) | pass | pass | .lil .out |  |
| `irjs-bound_default_identity_ignores_a_callers_same_name_parameter` | `bound_default_identity_ignores_a_callers_same_name_parameter` (src/codegen_ir_js.rs:36099) | pass | pass | .lil .out |  |
| `irjs-parenthesizes_a_ternary_phi_used_as_a_later_ternary_test` | `parenthesizes_a_ternary_phi_used_as_a_later_ternary_test` (src/codegen_ir_js.rs:36577) | pass | pass | .lil .out |  |
| `irjs-dense_string_tables_serialize_decoded_values` | `dense_string_tables_serialize_decoded_values` (src/codegen_ir_js.rs:36935) | pass | pass | .lil .out .host.js |  |
| `irjs-emitted_statement_shapes_keep_try_and_following_assignment_in_branch` | `emitted_statement_shapes_keep_try_and_following_assignment_in_branch` (src/codegen_ir_js.rs:37009) | pass | pass | .lil .out .host.js |  |
| `irjs-emitted_statement_shapes_preserve_catch_finally_and_nested_else` | `emitted_statement_shapes_preserve_catch_finally_and_nested_else` (src/codegen_ir_js.rs:37048) | pass | pass | .lil .out .host.js |  |
| `irjs-emitted_statement_shapes_keep_compound_iteration_bodies` | `emitted_statement_shapes_keep_compound_iteration_bodies` (src/codegen_ir_js.rs:37093) | pass | pass | .lil .out .host.js |  |
| `irjs-emitted_statement_shapes_preserve_declarations_and_captures` | `emitted_statement_shapes_preserve_declarations_and_captures` (src/codegen_ir_js.rs:37138) | pass | pass | .lil .out .host.js |  |
| `irjs-block_terminal_semicolon_elision_covers_nested_statement_blocks` | `block_terminal_semicolon_elision_covers_nested_statement_blocks` (src/codegen_ir_js.rs:37392) | pass | pass | .lil .out | the executed runtime_source program |
| `irjs-sinks_a_defaulted_factory_into_its_once_run_store` | `sinks_a_defaulted_factory_into_its_once_run_store` (src/codegen_ir_js.rs:37606) | pass | pass | .lil .out .host.js |  |
| `irjs-canonical_inline_values_survive_generic_callback_emission` | `canonical_inline_values_survive_generic_callback_emission` (src/codegen_ir_js.rs:37968) | pass | pass | .lil .out .host.js |  |
| `irjs-canonical_inline_values_keep_pooled_and_literal_representations` | `canonical_inline_values_keep_pooled_and_literal_representations` (src/codegen_ir_js.rs:37989) | pass | pass | .lil .out .host.js |  |
| `irjs-canonical_inline_values_do_not_erase_or_repeat_computed_coercions` | `canonical_inline_values_do_not_erase_or_repeat_computed_coercions` (src/codegen_ir_js.rs:38026) | pass | pass | .lil .out .host.js |  |
| `irjs-canonical_inline_values_do_not_erase_or_repeat_computed_coercions-throwing` | `canonical_inline_values_do_not_erase_or_repeat_computed_coercions` (src/codegen_ir_js.rs:38026) | pass | pass | .lil .out .host.js |  |
| `irjs-substitutes_private_fresh_empty_array_factories_without_sharing_identity` | `substitutes_private_fresh_empty_array_factories_without_sharing_identity` (src/codegen_ir_js.rs:38284) | pass | pass | .lil .out |  |
| `irjs-dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-coercion` | `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation` (src/codegen_ir_js.rs:38466) | pass | pass | .lil .out .host.js | test asserted only the TRACE suffix; the printed RL line follows from the program (b+a) |
| `irjs-dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-is_array` | `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation` (src/codegen_ir_js.rs:38466) | pass | pass | .lil .out .host.js |  |
| `irjs-dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-pre_branch` | `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation` (src/codegen_ir_js.rs:38466) | **FAIL** | pass | .lil .out .host.js | semantic: lowers `JsValue == 0` to `=== 0`: the ToPrimitive coercion never runs (no throw), and loose-equality results change (`"0"==0` prints false; legacy and JavaScript print true) (first diff: line 1: expected 'TRACE:coerce,throw' got 'false') |
| `irjs-dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-read_barrier` | `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation` (src/codegen_ir_js.rs:38466) | pass | **FAIL** | .lil .out .host.js | legacy: reads `state` after the coercion that writes it (first diff: line 1: expected '1' got '2') |
| `irjs-dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-typed_read_barrier` | `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation` (src/codegen_ir_js.rs:38466) | pass | pass | .lil .out .host.js |  |
| `irjs-dynamic_js_coercion_preserves_order_and_pre_branch_evaluation-unwrap_barrier` | `dynamic_js_coercion_preserves_order_and_pre_branch_evaluation` (src/codegen_ir_js.rs:38466) | pass | **FAIL** | .lil .out .host.js | legacy: reads `current` after the coercion that writes it (first diff: line 1: expected '1' got '2') |
| `irjs-expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy-eager` | `expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy` (src/codegen_ir_js.rs:38562) | pass | pass | .lil .out .host.js |  |
| `irjs-expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy-eager_store` | `expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy` (src/codegen_ir_js.rs:38562) | pass | pass | .lil .out .host.js |  |
| `irjs-expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy-lazy_first0` | `expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy` (src/codegen_ir_js.rs:38562) | pass | pass | .lil .out .host.js |  |
| `irjs-expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy-lazy_first5` | `expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy` (src/codegen_ir_js.rs:38562) | pass | pass | .lil .out .host.js |  |
| `irjs-expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy-nested` | `expression_regions_keep_header_effects_eager_and_dynamic_arms_lazy` (src/codegen_ir_js.rs:38562) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_stores_across_reused_constants-seed7` | `region_effect_owners_preserve_stores_across_reused_constants` (src/codegen_ir_js.rs:38641) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_stores_across_reused_constants-seed8` | `region_effect_owners_preserve_stores_across_reused_constants` (src/codegen_ir_js.rs:38641) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_throw_reentry_and_short_circuit_order-disabled` | `region_effect_owners_preserve_throw_reentry_and_short_circuit_order` (src/codegen_ir_js.rs:38664) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_throw_reentry_and_short_circuit_order-disabled_failing` | `region_effect_owners_preserve_throw_reentry_and_short_circuit_order` (src/codegen_ir_js.rs:38664) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_throw_reentry_and_short_circuit_order-enabled` | `region_effect_owners_preserve_throw_reentry_and_short_circuit_order` (src/codegen_ir_js.rs:38664) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_throw_reentry_and_short_circuit_order-enabled_failing` | `region_effect_owners_preserve_throw_reentry_and_short_circuit_order` (src/codegen_ir_js.rs:38664) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_preserve_throw_reentry_and_short_circuit_order-enabled_failing_seed8` | `region_effect_owners_preserve_throw_reentry_and_short_circuit_order` (src/codegen_ir_js.rs:38664) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_keep_constant_only_arms_and_repeated_results-disabled` | `region_effect_owners_keep_constant_only_arms_and_repeated_results` (src/codegen_ir_js.rs:38708) | pass | pass | .lil .out .host.js |  |
| `irjs-region_effect_owners_keep_constant_only_arms_and_repeated_results-enabled` | `region_effect_owners_keep_constant_only_arms_and_repeated_results` (src/codegen_ir_js.rs:38708) | pass | pass | .lil .out .host.js |  |
| `irjs-immediate_dynamic_read_fusion_preserves_dependency_names` | `immediate_dynamic_read_fusion_preserves_dependency_names` (src/codegen_ir_js.rs:38731) | pass | pass | .lil .out .host.js |  |
| `irjs-dynamic_iteration_shapes_remain_observable_when_results_are_unused-for_in` | `dynamic_iteration_shapes_remain_observable_when_results_are_unused` (src/codegen_ir_js.rs:38743) | pass | pass | .lil .out .host.js |  |
| `irjs-dynamic_iteration_shapes_remain_observable_when_results_are_unused-for_of` | `dynamic_iteration_shapes_remain_observable_when_results_are_unused` (src/codegen_ir_js.rs:38743) | pass | pass | .lil .out .host.js |  |
| `irjs-shared_for_in_iterable_is_materialized_before_body_reads` | `shared_for_in_iterable_is_materialized_before_body_reads` (src/codegen_ir_js.rs:38772) | pass | pass | .lil .out .host.js |  |
| `irjs-collapses_only_a_single_identity_helper_actual_binding-identity` | `collapses_only_a_single_identity_helper_actual_binding` (src/codegen_ir_js.rs:38925) | pass | pass | .lil .out .host.js |  |
| `irjs-collapses_only_a_single_identity_helper_actual_binding-unused` | `collapses_only_a_single_identity_helper_actual_binding` (src/codegen_ir_js.rs:38925) | pass | pass | .lil .out .host.js |  |
| `irjs-cached_pure_helper_actuals_are_eager_once_and_in_argument_order-eager_unused` | `cached_pure_helper_actuals_are_eager_once_and_in_argument_order` (src/codegen_ir_js.rs:38987) | pass | pass | .lil .out .host.js |  |
| `irjs-cached_pure_helper_actuals_are_eager_once_and_in_argument_order-ordered` | `cached_pure_helper_actuals_are_eager_once_and_in_argument_order` (src/codegen_ir_js.rs:38987) | pass | pass | .lil .out .host.js |  |
| `irjs-cached_pure_helper_actuals_are_eager_once_and_in_argument_order-ordered_throwing` | `cached_pure_helper_actuals_are_eager_once_and_in_argument_order` (src/codegen_ir_js.rs:38987) | pass | pass | .lil .out .host.js |  |
| `irjs-cached_pure_helper_actuals_are_eager_once_and_in_argument_order-repeated` | `cached_pure_helper_actuals_are_eager_once_and_in_argument_order` (src/codegen_ir_js.rs:38987) | pass | pass | .lil .out .host.js |  |
| `irjs-cached_actuals_support_nested_policy_helper_selection` | `cached_actuals_support_nested_policy_helper_selection` (src/codegen_ir_js.rs:39048) | pass | pass | .lil .out .host.js |  |
| `irjs-pure_helper_templates_accept_typed_string_guards_and_ordered_aggregate_fields` | `pure_helper_templates_accept_typed_string_guards_and_ordered_aggregate_fields` (src/codegen_ir_js.rs:39069) | pass | pass | .lil .out |  |
| `irjs-aggregate_read_helper_stays_before_an_intervening_mutation` | `aggregate_read_helper_stays_before_an_intervening_mutation` (src/codegen_ir_js.rs:39085) | **compile-error** | **FAIL** | .lil .out .host.js | semantic: refuses a struct passed to an extern: Unsupported "foreign value-struct storage adaptation"; legacy: reads `.total` after `mutate()` (prints 99) (first diff: line 1: expected '7' got '99') |
| `irjs-all_eligible_helper_policy_preserves_multi_use_outline_boundaries` | `all_eligible_helper_policy_preserves_multi_use_outline_boundaries` (src/codegen_ir_js.rs:39109) | pass | pass | .lil .out | test marked the helper as a repeated-region outline on the IR; one program |
| `irjs-pure_helper_selection_requires_explicit_full_arity_at_every_call_site` | `pure_helper_selection_requires_explicit_full_arity_at_every_call_site` (src/codegen_ir_js.rs:39148) | pass | pass | .lil .out .host.js |  |
| `irjs-decoded_string_values_survive_folding_templates_and_every_quote_family` | `decoded_string_values_survive_folding_templates_and_every_quote_family` (src/codegen_ir_js.rs:39371) | pass | pass | .lil .out .host.js | test swept three string quote families; one program |
| `irjs-decoded_constant_keys_and_regex_patterns_keep_their_values` | `decoded_constant_keys_and_regex_patterns_keep_their_values` (src/codegen_ir_js.rs:39411) | pass | pass | .lil .out | test swept three string quote families; one program |

## Skipped tests

Executing tests with no LilScript program to harvest (4):
- `does_not_redeclare_a_let_name_already_claimed_in_scope` (src/codegen_ir_js.rs:33384): only `node --check` (parse), no execution or output.
- `quote_variants_preserve_non_json_javascript_string_payloads` (src/codegen_ir_js.rs:35679): runs a string built by `render_string_literal`.
- `separates_addition_from_unary_plus_operands` (src/codegen_ir_js.rs:36512): runs a `JsExpression` built in Rust.
- `module_specifier_quotes_preserve_decoded_values` (src/codegen_ir_js.rs:37753): runs `render_module_specifier` output.

Parts of harvested tests that were left out: the shape-only programs beside the executed one (for example in
`emits_conservatively_proven_regex_literals_and_preserves_effectful_fallbacks` and
`nests_observable_array_elements_across_inert_literals`), and the `wrap_pure_helper_actual_bindings` JavaScript
expression in `collapses_only_a_single_identity_helper_actual_binding`.

Tests that never execute (281), by what they assert:
- Emitted-text shape only (contains, not-contains or exact output strings): 238.
- IR or emitter internals (coalescing colors, phi pairs, cluster ownership, layout order, manglers): 20.
- Renderer or helper unit tests with no LilScript program: 11.
- Expected compile or analysis errors: 10.
- Brotli size comparisons between spellings: 2.
