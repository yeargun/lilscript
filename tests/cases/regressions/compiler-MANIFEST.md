# Harvested regressions from the old route's `compiler.rs` tests

Source: `src/compiler.rs` (the `tests`, `function_scope_tests` and `terminal_javascript_parser_tests` modules) and
`src/compiler_rest_capture_tests.rs`, at commit `0c17237e`. Line numbers below are at that commit.
Expected stdout comes from each test's own assertions, never from running a compiler.

## Counts

- Tests read: 268 (266 in `compiler.rs`, 2 in `compiler_rest_capture_tests.rs`).
- Harvested: 53 tests that compile and execute LilScript, plus 9 `text-exact` tests (below), giving 65 cases.
- Skipped: 206 tests (reasons at the end).
- JavaScript, reference binary `~/lilscript-work/bin/reference-2026-09-23/lilscript`: semantic route 59/65 pass, legacy route 64/65 pass.
- C (informational, the 20 cases with no host, `JsValue`, `async` or module probe; emitted C built with `cc -std=c11 -O2`): semantic 12/20 pass, legacy 20/20 pass.

## Case conventions

- `compiler-<test>.lil` + `.out`: the program and its expected stdout, one line per `print`/`console.log`.
  Where one test holds several programs or host variants, the case name gets a `-<variant>` suffix.
- `.host.js`: a prelude, run before the compiled program in the same realm. Each is one block that assigns the
  test harness's externs on `globalThis`. When the harness did work *after* the program (called a kept callback,
  printed a trace), the prelude schedules it with `queueMicrotask`, so it runs once the program's synchronous code is done.
  Output the harness wrote with `process.stdout.write` is printed with `console.log` (one trailing newline).
- Handoff: tests that imported an ES module and probed its exports keep their `export`s, and the program ends with
  a call to a host driver (`check(...)`/`drive(...)`) that receives the exported functions and replays the probe.
  This keeps the functions escaping to unknown JavaScript callers while running on the script lane.
- `.module-probe.mjs` (**module lane only**, new kind): for tests whose probe observes identity at the ESM boundary
  (constructor name/arity/inheritance, exported function names). Compile with `--target js-module`, import the
  output as `m`, then call the probe's default export with `m`. These cases print nothing on the script lane.
- `// harness: "use strict"` in a `.lil` header: the test ran the emitted script with `"use strict";` prepended.
- Multi-module cases: the entry is `compiler-<test>.lil`; the other modules live in the directory `compiler-<test>/`
  (so a runner that globs `regressions/*.lil` sees only entries). Imports of the entry use `../compiler-<test>`.
- `.toml`: only language/contract settings the test set (`assume_pristine_builtins`, `assume_pure_property_reads`,
  `mangle.exports`). Optimizer knobs from the tests (candidate search, cost model, levels, inlining...) are dropped.
  The verifier prepends `[javascript] strip_console = false`.
- `text-exact`: the test compiled but did not execute; it asserted the *complete* output (`console.log(42)`),
  which fixes the stdout. Included because M1.5 deletes these tests and the behaviour would otherwise be lost.

Verifier: `~/lilscript-work/out/harvest/compiler/verify.py` (JavaScript, Node v24.11.1) and
`HARVEST_C_LANE=1 verify.py` (C). Outputs and stderr per case are in `~/lilscript-work/out/harvest/compiler/work/`.

## Cases

| Case | Source test | Kind | Harness | Extra files | Semantic JS | Legacy JS | Semantic C | Legacy C | Note |
|---|---|---|---|---|---|---|---|---|---|
| `compiler-a_record_read_tested_for_null_stays_undefined_when_absent` | `a_record_read_tested_for_null_stays_undefined_when_absent` (src/compiler.rs:20707) | executed | handoff | .host.js | pass | pass | n/a | n/a | absent record key reads as `null` through `string?` |
| `compiler-a_record_read_that_is_never_tested_keeps_its_null_normalization` | `a_record_read_that_is_never_tested_keeps_its_null_normalization` (src/compiler.rs:20729) | executed | handoff | .host.js | pass | pass | n/a | n/a | absent record key escapes as `null`, not `undefined` |
| `compiler-a_string_element_read_compared_against_a_truthy_value_drops_its_hole_guard` | `a_string_element_read_compared_against_a_truthy_value_drops_its_hole_guard` (src/compiler.rs:20775) | executed | handoff | .host.js | pass | pass | n/a | n/a | out-of-range `string[]` read compared against a truthy value |
| `compiler-an_unnormalized_map_get_tests_strictly_for_undefined_unless_truthiness_is_cheaper` | `an_unnormalized_map_get_tests_strictly_for_undefined_unless_truthiness_is_cheaper` (src/compiler.rs:20747) | executed | handoff | .host.js | pass | pass | n/a | n/a | `Map.get` miss path |
| `compiler-brotli_objective_carries_async_literal_movement_into_name_search` | `brotli_objective_carries_async_literal_movement_into_name_search` (src/compiler.rs:20130) | executed | stdout |  | pass | pass | n/a | n/a | `await Task.resolve` inside an async function |
| `compiler-checked_pure_exports_carry_call_annotations_after_selection` | `checked_pure_exports_carry_call_annotations_after_selection` (src/compiler.rs:20641) | executed | handoff | .host.js | pass | pass | n/a | n/a | exported closure produced by a `pure` factory; handed to host `check` |
| `compiler-compiles_all_backends_from_one_optimized_module` | `compiles_all_backends_from_one_optimized_module` (src/compiler.rs:17333) | text-exact | stdout |  | pass | pass | pass | pass | asserted JavaScript `console.log(42)` |
| `compiler-compiles_and_tree_shakes_a_module_graph` | `compiles_and_tree_shakes_a_module_graph` (src/compiler.rs:17863) | text-exact | multi-module | /facade.lil,math.lil | pass | pass | pass | pass | asserted output `console.log(25)` (facade re-export) |
| `compiler-compiles_source_end_to_end` | `compiles_source_end_to_end` (src/compiler.rs:10603) | text-exact | stdout |  | pass | pass | pass | pass | asserted output `console.log(42)` |
| `compiler-constructor_export_preserves_explicit_inheritance` | `constructor_export_preserves_explicit_inheritance` (src/compiler.rs:17432) | executed | module probe | .toml .module-probe.mjs | **compile-error** | pass | n/a | n/a | exported subclass keeps `extends`/`super`; `Base` reached as `Object.getPrototypeOf(Child)` (not exported) — semantic: error: direct module checking does not yet support nominal constructor exports |
| `compiler-constructor_export_synthesizes_only_the_published_default_constructor` | `constructor_export_synthesizes_only_the_published_default_constructor` (src/compiler.rs:17472) | executed | module probe | .toml .module-probe.mjs | **compile-error** | pass | n/a | n/a | zero-arity constructor synthesized for the published class; field defaults — semantic: error: direct module checking does not yet support nominal constructor exports |
| `compiler-cyclic_imports_observe_live_export_updates` | `cyclic_imports_observe_live_export_updates` (src/compiler.rs:20403) | executed | multi-module | /b.lil | pass | pass | **compile-error** | pass | live binding update visible across a cycle — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" } |
| `compiler-declares_a_loop_carried_binding_before_a_prototype_walk` | `declares_a_loop_carried_binding_before_a_prototype_walk` (src/compiler.rs:18750) | executed | strict |  | pass | pass | n/a | n/a | loop-carried binding must be declared; run as a `"use strict"` script (marker in the `.lil` header) |
| `compiler-drops_globals_only_read_by_dead_functions` | `drops_globals_only_read_by_dead_functions` (src/compiler.rs:17652) | text-exact | stdout |  | pass | pass | pass | pass | asserted output `console.log(1)` |
| `compiler-drops_unread_scheduler_store_and_host_value` | `drops_unread_scheduler_store_and_host_value` (src/compiler.rs:17658) | text-exact | host | .host.js | pass | pass | n/a | n/a | asserted output `console.log(1)`; the test ran no host, the prelude only supplies a never-called `host` |
| `compiler-every_production_objective_invokes_empty_parameter_sibling_closures` | `every_production_objective_invokes_empty_parameter_sibling_closures` (src/compiler.rs:20047) | executed | stdout |  | pass | pass | pass | pass | sibling zero-parameter closures over one mutable capture |
| `compiler-every_production_objective_preserves_async_function_boundaries` | `every_production_objective_preserves_async_function_boundaries` (src/compiler.rs:20086) | executed | stdout |  | **runtime-error** | pass | n/a | n/a | an `async` function's call stays a Task — semantic: exit 1: TypeError: a.then is not a function; line 1: expected '1', got '' |
| `compiler-explicit_constructor_export_preserves_named_class_identity` | `explicit_constructor_export_preserves_named_class_identity` (src/compiler.rs:17381) | executed | module probe | .toml .module-probe.mjs | **compile-error** | pass | n/a | n/a | `export constructor Scale as PublicScale`: name, arity, field, prototype method at the ESM boundary — semantic: error: direct module checking does not yet support nominal constructor exports |
| `compiler-function_scope_wraps_the_module_internals_and_keeps_the_public_api` | `function_scope_wraps_the_module_internals_and_keeps_the_public_api` (src/compiler.rs:20669) | executed | module probe | .toml .module-probe.mjs | pass | pass | n/a | n/a | exported functions' values, names and arity at the ESM boundary; `typeof look.prototype` dropped (no asserted value) |
| `compiler-gzip_search_can_keep_distinct_locals_when_coalescing_adds_syntax` | `gzip_search_can_keep_distinct_locals_when_coalescing_adds_syntax` (src/compiler.rs:17040) | executed | stdout |  | pass | **compile-error** | n/a | n/a | `JS.and` result narrowed with `is string`. Legacy fails parser admission on its own output under the default (Brotli) objective; the test ran the gzip objective — legacy: error: generated JavaScript failed standards parser admission: Unexpected token |
| `compiler-initializes_each_module_once_in_a_static_cycle` | `initializes_each_module_once_in_a_static_cycle` (src/compiler.rs:20444) | executed | multi-module | /b.lil | pass | pass | **compile-error** | pass | each module initialized once — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" } |
| `compiler-inner_temps_do_not_clobber_module_bindings_used_as_exports` | `inner_temps_do_not_clobber_module_bindings_used_as_exports` (src/compiler.rs:18912) | executed | stdout |  | pass | pass | n/a | n/a | inner temporaries do not overwrite module bindings |
| `compiler-js_construct_emits_new_without_a_host_wrapper` | `js_construct_emits_new_without_a_host_wrapper` (src/compiler.rs:18342) | executed | stdout |  | pass | pass | n/a | n/a | `JS.construct` of a `JS.method0` wrapper |
| `compiler-keeps_a_captured_changed_flag_across_a_js_track_call` | `keeps_a_captured_changed_flag_across_a_js_track_call` (src/compiler.rs:18593) | executed | stdout |  | pass | pass | n/a | n/a | captured flag written inside a tracked callback (mobx reaction shape) |
| `compiler-keeps_a_captured_changed_flag_across_a_known_track_method_in_production` | `keeps_a_captured_changed_flag_across_a_known_track_method_in_production` (src/compiler.rs:18670) | executed | stdout |  | pass | pass | n/a | n/a | same, through a prototype `track` method |
| `compiler-keeps_copied_property_compare_across_a_mutating_call_before_a_branch` | `keeps_copied_property_compare_across_a_mutating_call_before_a_branch` (src/compiler.rs:18457) | executed | stdout |  | pass | pass | n/a | n/a | same, with the branch after the call |
| `compiler-keeps_copied_property_load_across_a_mutating_method_call` | `keeps_copied_property_load_across_a_mutating_method_call` (src/compiler.rs:18417) | executed | stdout |  | pass | pass | n/a | n/a | property comparison is not re-read after a mutating method call |
| `compiler-keeps_private_module_bindings_isolated` | `keeps_private_module_bindings_isolated` (src/compiler.rs:17939) | text-exact | multi-module | /left.lil,right.lil | pass | pass | pass | pass | asserted output `console.log(3)` (equal private names) |
| `compiler-keeps_toint_property_compare_across_a_js_call` | `keeps_toint_property_compare_across_a_js_call` (src/compiler.rs:18498) | executed | stdout |  | pass | pass | n/a | n/a | `toInt()` comparison not sunk past a JS call |
| `compiler-keeps_toint_property_compare_across_a_js_call_when_used_in_a_later_or` | `keeps_toint_property_compare_across_a_js_call_when_used_in_a_later_or` (src/compiler.rs:18539) | executed | stdout |  | pass | pass | n/a | n/a | same, consumed by a later `||` (mobx trackAndCompute shape) |
| `compiler-links_two_module_cycle_with_exported_value` | `links_two_module_cycle_with_exported_value` (src/compiler.rs:20363) | executed | multi-module | /b.lil | pass | pass | **compile-error** | pass | two-module cycle reading an exported value — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" } |
| `compiler-lowers_javascript_short_circuit_and_strict_comparison_without_helpers` | `lowers_javascript_short_circuit_and_strict_comparison_without_helpers` (src/compiler.rs:11437) | executed | handoff | .host.js | pass | pass | n/a | n/a | `JS.or`/`JS.and` evaluation order and strict (in)equality; exports handed to host `check`, which replays runner.mjs |
| `compiler-materializes_mutable_reads_before_later_writes` | `materializes_mutable_reads_before_later_writes` (src/compiler.rs:19861) | executed | stdout |  | pass | pass | pass | pass | third program only (the first two are asserted on text): a comparison is taken before the global is written |
| `compiler-method_has_temp_does_not_clobber_a_live_value_argument` | `method_has_temp_does_not_clobber_a_live_value_argument` (src/compiler.rs:18848) | executed | stdout |  | pass | pass | n/a | n/a | temporary for `has` does not clobber the live `value` argument |
| `compiler-module_level_assignment_is_not_shadowed_inside_setter` | `module_level_assignment_is_not_shadowed_inside_setter` (src/compiler.rs:18802) | executed | stdout |  | pass | pass | n/a | n/a | module-level store inside a setter is not shadowed by a local |
| `compiler-nested_js_closure_keeps_copied_value_after_source_write` | `nested_js_closure_keeps_copied_value_after_source_write` (src/compiler.rs:18378) | executed | stdout |  | pass | pass | n/a | n/a | copy of a captured binding survives a write through a nested JS method |
| `compiler-nested_js_closure_stores_captured_outer_binding` | `nested_js_closure_stores_captured_outer_binding` (src/compiler.rs:19101) | executed | stdout |  | pass | pass | n/a | n/a | nested JS method stores into a captured outer binding |
| `compiler-nested_same_receiver_member_calls_keep_javascript_this` | `nested_same_receiver_member_calls_keep_javascript_this` (src/compiler.rs:11358) | executed | stdout |  | pass | pass | n/a | n/a | same-receiver member call inside a nested closure keeps JavaScript `this` |
| `compiler-object_has_own_direct_and_detached_calls_preserve_runtime_order` | `object_has_own_direct_and_detached_calls_preserve_runtime_order` (src/compiler.rs:19216) | executed | host+epilogue | .host.js | **runtime-error** | pass | n/a | n/a | argument evaluation order for direct and detached `objectHasOwn`. The harness defines no `objectHasOwn`: the old route lowered the extern by name (js_externs.rs) to `Object.hasOwn` — semantic: exit 1: ReferenceError: objectHasOwn is not defined; line 1: expected 'true', got '' |
| `compiler-ordinary_object_literal_preserves_javascript_prototype_semantics` | `ordinary_object_literal_preserves_javascript_prototype_semantics` (src/compiler.rs:17524) | executed | host+epilogue | .host.js .toml | pass | pass | n/a | n/a | `object{}` literal: own `__proto__` data key, literal keys bypass inherited setters, assignment hits them; `.toml` sets `assume_pristine_builtins = false` |
| `compiler-owned_plain_object_proof_forwards_only_proven_own_reads-escaped` | `owned_plain_object_proof_forwards_only_proven_own_reads` (src/compiler.rs:17557) | executed | host | .host.js .toml | pass | pass | n/a | n/a | program 3 of 3: an escaped object's own key is re-read after the host redefines it |
| `compiler-owned_plain_object_proof_forwards_only_proven_own_reads-missing` | `owned_plain_object_proof_forwards_only_proven_own_reads` (src/compiler.rs:17557) | executed | host+epilogue | .host.js .toml | pass | pass | n/a | n/a | program 2 of 3: a missing key reads through an inherited getter |
| `compiler-owned_plain_object_proof_forwards_only_proven_own_reads-owned` | `owned_plain_object_proof_forwards_only_proven_own_reads` (src/compiler.rs:17557) | executed | host+epilogue | .host.js .toml | pass | pass | n/a | n/a | program 1 of 3: effectful initializer runs exactly once; `.toml` sets `assume_pure_property_reads = false` |
| `compiler-parsed_peephole_applies_without_candidate_search` | `parsed_peephole_applies_without_candidate_search` (src/compiler.rs:16422) | executed | host | .host.js | pass | pass | n/a | n/a | global initialized from an extern, then unit-updated |
| `compiler-parsed_peephole_is_independently_configurable` | `parsed_peephole_is_independently_configurable` (src/compiler.rs:16346) | executed | stdout |  | pass | pass | pass | pass | global unit update through a function (`state=state+1`) |
| `compiler-permits_deferred_cyclic_module_value_reads` | `permits_deferred_cyclic_module_value_reads` (src/compiler.rs:20464) | executed | multi-module | /b.lil | pass | pass | **compile-error** | pass | deferred read of a cyclic module value through a closure — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" } |
| `compiler-preserves_branch_local_shadowing_while_linking` | `preserves_branch_local_shadowing_while_linking` (src/compiler.rs:20303) | text-exact | multi-module | /library.lil | pass | pass | pass | pass | asserted output `console.log(7)` (braceless branch-local declaration shadowing a function) |
| `compiler-preserves_effectful_calls_before_conditional_returns` | `preserves_effectful_calls_before_conditional_returns` (src/compiler.rs:20198) | executed | host+epilogue | .host.js | pass | pass | n/a | n/a | retained callback increments before its conditional return; host calls it twice |
| `compiler-private_global_store_survives_cross_function_conditional_selection-flag_false` | `private_global_store_survives_cross_function_conditional_selection` (src/compiler.rs:19937) | executed | host | .host.js | pass | pass | n/a | n/a | host `flag()` returns false |
| `compiler-private_global_store_survives_cross_function_conditional_selection-flag_true` | `private_global_store_survives_cross_function_conditional_selection` (src/compiler.rs:19937) | executed | host | .host.js | pass | pass | n/a | n/a | host `flag()` returns true |
| `compiler-production_nested_call_temp_does_not_reuse_a_module_callee_register` | `production_nested_call_temp_does_not_reuse_a_module_callee_register` (src/compiler.rs:19027) | executed | stdout |  | pass | pass | n/a | n/a | nested call temporary does not reuse the module callee's name |
| `compiler-reports_missing_exports_and_links_static_module_cycles` | `reports_missing_exports_and_links_static_module_cycles` (src/compiler.rs:20322) | text-exact | multi-module | /left.lil,right.lil | pass | pass | pass | pass | second half only, asserted `console.log(7)`; the first half is a diagnostic |
| `compiler-resolves_reexports_through_a_static_cycle` | `resolves_reexports_through_a_static_cycle` (src/compiler.rs:20423) | executed | multi-module | /b.lil,c.lil | pass | pass | **compile-error** | pass | re-export resolved through a cycle (three modules) — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" } |
| `compiler-rest_argument_alias_keeps_nested_mutation_and_invocation_identity-cleared_reader` | `rest_argument_alias_keeps_nested_mutation_and_invocation_identity_at_emission` (src/compiler_rest_capture_tests.rs:112)<br>`rest_argument_alias_keeps_nested_mutation_and_invocation_identity_in_configured_pipeline` (src/compiler_rest_capture_tests.rs:161) | executed | handoff | .host.js | pass | pass | n/a | n/a | same, cleared reader (CLEARED_SOURCE) |
| `compiler-rest_argument_alias_keeps_nested_mutation_and_invocation_identity-retained_reader` | `rest_argument_alias_keeps_nested_mutation_and_invocation_identity_at_emission` (src/compiler_rest_capture_tests.rs:112)<br>`rest_argument_alias_keeps_nested_mutation_and_invocation_identity_in_configured_pipeline` (src/compiler_rest_capture_tests.rs:161) | executed | handoff | .host.js | pass | pass | n/a | n/a | `JS.methodRest` `arguments` alias with nested mutation, retained reader (SOURCE); both tests share this program |
| `compiler-search_does_not_rank_a_nested_local_that_shadows_an_outer_binding_still_read` | `search_does_not_rank_a_nested_local_that_shadows_an_outer_binding_still_read` (src/compiler.rs:12077) | executed | host+epilogue | .host.js | pass | pass | n/a | n/a | nested local must not shadow a captured outer `callback`; host calls the kept `go` after the program |
| `compiler-size_first_search_keeps_js_string_of_a_jsvalue` | `size_first_search_keeps_js_string_of_a_jsvalue` (src/compiler.rs:12127) | executed | host+epilogue | .host.js | pass | pass | n/a | n/a | `JS.string(JsValue)` keeps ToString (host `toString` object) |
| `compiler-size_first_search_spreads_a_delimiter_not_the_live_match` | `size_first_search_spreads_a_delimiter_not_the_live_match` (src/compiler.rs:12172) | executed | host+epilogue | .host.js | pass | pass | n/a | n/a | `codePointLength` of the picked delimiter, not the live match |
| `compiler-snapshot_of_a_mutable_capture_survives_production_indirect_store` | `snapshot_of_a_mutable_capture_survives_production_indirect_store` (src/compiler.rs:18970) | executed | stdout |  | pass | pass | n/a | n/a | snapshot of a mutable capture survives an indirect store |
| `compiler-snapshot_of_a_record_field_survives_a_captured_rebind` | `snapshot_of_a_record_field_survives_a_captured_rebind` (src/compiler.rs:12014) | executed | stdout |  | pass | pass | **compile-error** | pass | saved field read survives a closure that rebinds the record parameter — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native callable payload type" } |
| `compiler-snapshot_of_a_record_field_survives_a_later_write` | `snapshot_of_a_record_field_survives_a_later_write` (src/compiler.rs:11994) | executed | stdout |  | pass | pass | **compile-error** | pass | a saved `Record` field read survives a later write (dot and computed read) — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native callable payload type" } |
| `compiler-snapshot_of_a_top_level_record_field_survives_a_captured_rebind` | `snapshot_of_a_top_level_record_field_survives_a_captured_rebind` (src/compiler.rs:12064) | executed | stdout |  | pass | pass | **compile-error** | pass | same, for a top-level record — semantic C: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native source type" } |
| `compiler-user_defined_host_alias_name_is_not_rewritten` | `user_defined_host_alias_name_is_not_rewritten` (src/compiler.rs:19182) | executed | stdout |  | pass | pass | pass | pass | user function named `objectHasOwn` is not treated as the host alias |
| `compiler-uses_imported_constructor_value_inside_exported_function_in_cycle` | `uses_imported_constructor_value_inside_exported_function_in_cycle` (src/compiler.rs:20383) | executed | multi-module | /b.lil | **compile-error** | pass | n/a | n/a | imported `export constructor` used as a value (`JS.construct`) across a cycle — semantic: error: direct module checking does not yet support nominal constructor exports |
| `compiler-validates_declared_pure_functions` | `validates_declared_pure_functions` (src/compiler.rs:17626) | text-exact | stdout |  | pass | pass | pass | pass | second program only, asserted `console.log(25)`; the other programs are diagnostics |

## Failures on the remaining compiler (semantic route)

- `compiler-constructor_export_preserves_explicit_inheritance`: compile error `direct module checking does not yet support nominal constructor exports` (plan: M8.2).
- `compiler-constructor_export_synthesizes_only_the_published_default_constructor`: compile error `direct module checking does not yet support nominal constructor exports` (plan: M8.2).
- `compiler-every_production_objective_preserves_async_function_boundaries`: **Miscompile.** `async int immediate(){return 1;}` — the call `immediate()` is folded to the constant `1`, so `immediate().then(...)` becomes `1.then(...)`: `TypeError: a.then is not a function`. Minimal repro: `async int f(){return 1;} f().then((int v)=>print(v));` emits `let a=1;a.then(...)`. A non-constant async body (`return x+1;`) is kept as `async`.
- `compiler-explicit_constructor_export_preserves_named_class_identity`: compile error `direct module checking does not yet support nominal constructor exports` (plan: M8.2).
- `compiler-object_has_own_direct_and_detached_calls_preserve_runtime_order`: `ReferenceError: objectHasOwn is not defined`. The semantic route emits the extern by name; the old route lowered it through its name-keyed host table (`js_externs.rs`, whose data the plan moves to the operation catalog, M8.4). Until then the case fails; if the catalog will not recognize `objectHasOwn`, add a declared binding (`globalThis.objectHasOwn = Object.hasOwn`) to the host prelude instead.
- `compiler-uses_imported_constructor_value_inside_exported_function_in_cycle`: compile error `direct module checking does not yet support nominal constructor exports`, raised for the `export constructor` in the imported module (plan: M8.2). The diagnostic points at `b.lil:2:1` (the `import` line), not at the `export constructor` on line 3.

C lane, semantic route (informational; the old tests never ran C):

- `compiler-cyclic_imports_observe_live_export_updates`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" }
- `compiler-initializes_each_module_once_in_a_static_cycle`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" }
- `compiler-links_two_module_cycle_with_exported_value`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" }
- `compiler-permits_deferred_cyclic_module_value_reads`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" }
- `compiler-resolves_reexports_through_a_static_cycle`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native exported ABI" }
- `compiler-snapshot_of_a_record_field_survives_a_captured_rebind`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native callable payload type" }
- `compiler-snapshot_of_a_record_field_survives_a_later_write`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native callable payload type" }
- `compiler-snapshot_of_a_top_level_record_field_survives_a_captured_rebind`: semantic compiler (native): Unsupported { unit: None, operation: None, span: Span { start: 0, end: 0 }, feature: "native source type" }

Legacy route failures (for comparison):

- `compiler-gzip_search_can_keep_distinct_locals_when_coalescing_adds_syntax`: error: generated JavaScript failed standards parser admission: Unexpected token (default Brotli objective; the test ran the gzip objective, where it passed)

## Skipped tests

### A. search, selection, codec, scheduler or metric internals (candidate budgets, frontiers, finalizers, thread pools, codec fixtures); no program behaviour asserted (95)

- `candidate_limit_is_shared_across_optimizer_variants` (src/compiler.rs:11531)
- `fresh_literal_factory_candidate_uses_a_two_slot_terminal_frontier` (src/compiler.rs:11568)
- `each_codec_objective_beats_the_other_objectives_reverse_artifact` (src/compiler.rs:11628)
- `canonical_brotli_scorer_matches_node_24_fixtures` (src/compiler.rs:11881)
- `canonical_gzip_scorer_uses_bundled_official_zlib_fixtures` (src/compiler.rs:11893)
- `configured_probe_seed_skips_terminal_emission` (src/compiler.rs:11934)
- `closed_record_projection_requires_two_competing_slots` (src/compiler.rs:12310)
- `one_slot_terminal_search_finalizes_the_seed_without_emitting_variants` (src/compiler.rs:12324)
- `byte_exhausted_terminal_search_preserves_the_exact_incumbent` (src/compiler.rs:12388)
- `one_slot_full_search_skips_probe_helper_interaction_work` (src/compiler.rs:12443)
- `explicit_compiler_threads_use_a_local_pool_and_omitted_threads_preserve_current_pool` (src/compiler.rs:12524)
- `compiler_resource_counts_preserve_exact_selected_javascript` (src/compiler.rs:12547)
- `finalizer_batches_are_contiguous_ordered_and_worker_bounded` (src/compiler.rs:12585)
- `terminal_codec_pool_caps_nested_remap_parallelism` (src/compiler.rs:12611)
- `parallel_brotli_finalizer_matches_serial_exact_result` (src/compiler.rs:12637)
- `parallel_brotli_finalizer_preserves_exact_tie_breaking` (src/compiler.rs:12703)
- `parallel_brotli_finalizer_matches_serial_errors` (src/compiler.rs:12761)
- `finalizer_reuses_exact_selected_model_declaration_scores` (src/compiler.rs:12824)
- `optional_candidate_raw_cap_is_inclusive_and_skips_codec_work` (src/compiler.rs:12863)
- `selected_score_batch_applies_raw_cap_before_incumbent_lookup` (src/compiler.rs:12907)
- `oversized_initial_base_can_spawn_a_shorter_quote_descendant_with_stable_ordinal` (src/compiler.rs:12934)
- `objective_stratified_frontier_is_deterministic_and_bounded` (src/compiler.rs:12976)
- `objective_ranking_shares_missing_codec_score_across_equal_context_bytes` (src/compiler.rs:13010)
- `alternate_gzip_scoring_flattens_one_declaration_family_across_workers` (src/compiler.rs:13080)
- `alternate_gzip_scoring_deduplicates_leaves_and_matches_serial_ranking` (src/compiler.rs:13125)
- `alternate_gzip_leaf_failure_marks_the_complete_family_without_losing_identities` (src/compiler.rs:13201)
- `selected_brotli_batch_shares_one_exact_ledger_without_collapsing_contexts` (src/compiler.rs:13252)
- `selected_brotli_batch_copies_the_complete_declaration_ledger` (src/compiler.rs:13304)
- `selected_score_batch_parallelizes_one_plans_declaration_leaves` (src/compiler.rs:13373)
- `selected_score_batch_matches_serial_ledger_and_owner_order` (src/compiler.rs:13430)
- `selected_score_batch_reports_the_first_declaration_leaf_error` (src/compiler.rs:13485)
- `selected_score_batch_reports_configured_root_failure_before_optional_results` (src/compiler.rs:13545)
- `configured_root_probe_batch_matches_serial_and_uses_full_codec_pool` (src/compiler.rs:13604)
- `selected_score_reuse_key_includes_bytes_model_and_declaration_semantics` (src/compiler.rs:13713)
- `selected_score_batch_respects_worker_cap_and_preserves_owner_order` (src/compiler.rs:13782)
- `selected_score_batch_reuses_only_a_matching_incumbent_and_propagates_failures` (src/compiler.rs:13827)
- `live_frontier_keeps_configured_and_distinct_but_deduplicates_identical_code` (src/compiler.rs:13898)
- `final_selection_preserves_identity_across_equal_two_context_artifacts` (src/compiler.rs:13945)
- `oversized_optional_proposal_does_not_perturb_fitting_identity` (src/compiler.rs:14010)
- `oversized_precomputed_proposal_conflicting_with_pinned_root_is_rejected` (src/compiler.rs:14075)
- `exact_precomputed_duplicate_of_pinned_root_is_inert_without_codec_work` (src/compiler.rs:14114)
- `wrong_model_precomputed_duplicate_of_pinned_root_is_rejected` (src/compiler.rs:14141)
- `oversized_precomputed_interaction_and_new_emission_are_equally_inert` (src/compiler.rs:14179)
- `oversized_precomputed_seed_still_validates_its_codec_ledger` (src/compiler.rs:14228)
- `aggregate_plan_arena_pins_configured_floor_and_skips_oversized_ranked_pins` (src/compiler.rs:14280)
- `aggregate_plan_arena_keeps_equal_bytes_from_distinct_contexts` (src/compiler.rs:14472)
- `aggregate_plan_arena_preserves_the_best_plan_before_local_regime_diversity` (src/compiler.rs:14511)
- `aggregate_plan_arena_does_not_reserve_byte_identical_local_regimes` (src/compiler.rs:14584)
- `javascript_plan_identity_is_scoped_to_its_context` (src/compiler.rs:14637)
- `structural_plan_budget_is_charged_before_emission_and_preserves_terminal_slots` (src/compiler.rs:14662)
- `priority_plan_reserve_is_free_for_duplicates_and_never_exceeds_the_hard_cap` (src/compiler.rs:14718)
- `priority_families_share_the_reserved_slice_and_release_unused_work` (src/compiler.rs:14785)
- `level_eight_priority_reserve_cannot_withhold_unconsumable_work` (src/compiler.rs:14848)
- `bounded_candidate_merge_scores_proposals_before_evicting_the_old_frontier` (src/compiler.rs:14900)
- `bounded_variant_sampling_spans_spelling_columns` (src/compiler.rs:14941)
- `alternate_objective_failure_cannot_fail_the_selected_build` (src/compiler.rs:14969)
- `configured_baseline_survives_a_one_candidate_final_frontier` (src/compiler.rs:15024)
- `one_plan_final_frontier_selects_its_codec_best_declaration_leaf` (src/compiler.rs:15066)
- `entropy_probe_preparation_matches_one_and_four_threads_exactly` (src/compiler.rs:15106)
- `entropy_probe_preparation_uses_the_active_parallel_pool` (src/compiler.rs:15144)
- `identifier_alphabet_search_remains_serial_and_preserves_order_and_errors` (src/compiler.rs:15178)
- `unused_letter_binding_remap_can_leave_the_initial_live_character_set` (src/compiler.rs:15231)
- `unused_letter_binding_remap_can_recover_a_two_name_brotli_interaction` (src/compiler.rs:15252)
- `exact_two_binding_search_has_a_fixed_trial_ceiling` (src/compiler.rs:15282)
- `unused_binding_remaps_never_rename_an_ambient_one_byte_host` (src/compiler.rs:15315)
- `higher_effort_retains_the_lower_effort_two_binding_brotli_winner` (src/compiler.rs:15346)
- `short_binding_remap_can_collapse_a_two_character_local` (src/compiler.rs:15401)
- `identifier_alphabet_search_can_leave_the_initial_live_character_set` (src/compiler.rs:15419)
- `final_peephole_keeps_the_exact_codec_baseline_as_a_candidate` (src/compiler.rs:15442)
- `candidate_raw_growth_limit_is_exact_and_tunable` (src/compiler.rs:15519)
- `compressed_cost_models_keep_better_transfer_despite_raw_growth` (src/compiler.rs:15527)
- `optimizer_search_includes_the_reusable_helper_corner` (src/compiler.rs:15563)
- `javascript_search_adapts_local_name_reservation` (src/compiler.rs:15579)
- `terminal_scope_naming_challenges_the_actual_winner_with_codec_friendly_prefixes` (src/compiler.rs:15588)
- `terminal_candidate_budget_charges_retained_and_challenger_slots_and_bytes` (src/compiler.rs:15617)
- `terminal_family_reserves_are_released_at_most_once` (src/compiler.rs:15653)
- `terminal_codec_probe_budget_is_a_hard_compilation_wide_call_ceiling` (src/compiler.rs:15674)
- `candidate_search_off_skips_terminal_live_letter_work` (src/compiler.rs:15816)
- `selected_canonical_peephole_uses_only_reserved_terminal_work` (src/compiler.rs:15860)
- `search_off_finalization_scores_the_artifact_it_returns` (src/compiler.rs:15943)
- `zero_structural_proposal_budget_skips_optional_emission_before_codegen` (src/compiler.rs:16096)
- `parsed_peephole_leaves_share_the_terminal_codec_budget` (src/compiler.rs:16125)
- `terminal_challenger_emission_obeys_the_shared_slot_and_byte_tail` (src/compiler.rs:16207)
- `terminal_string_pooling_challenges_the_actual_winner_with_sparse_thresholds` (src/compiler.rs:16321)
- `codec_layout_windows_match_the_exact_encoders` (src/compiler.rs:16511)
- `explained_compilation_reports_selection_costs` (src/compiler.rs:16520)
- `raw_objective_selects_a_shared_helper_over_duplicated_inlining` (src/compiler.rs:16674)
- `compressor_search_selects_eager_pure_helper_substitution_when_smaller` (src/compiler.rs:16718)
- `outlined_ir_probe_carries_its_helper_interaction_into_a_small_finalist_budget` (src/compiler.rs:16755)
- `deferred_inlining_probe_carries_single_static_use_into_a_two_slot_budget` (src/compiler.rs:16866)
- `compressor_scores_helper_and_dense_table_choices_as_one_cartesian_family` (src/compiler.rs:16956)
- `codec_scoring_selects_a_better_function_layout_without_raw_growth` (src/compiler.rs:17105)
- `codec_scoring_selects_proven_private_function_subsumption` (src/compiler.rs:17149)
- `applies_javascript_priority_without_changing_native_policy` (src/compiler.rs:17186)
- `repeated_compilation_is_byte_deterministic` (src/compiler.rs:17345)

### B. asserts only the shape of emitted JavaScript or C text (contains / not contains / ordering) (74)

- `emits_typed_foreign_modules_as_native_esm_imports` (src/compiler.rs:10663)
- `preserves_side_effect_foreign_esm_imports` (src/compiler.rs:10698)
- `tree_shakes_unused_foreign_imports_through_barrel_reexports` (src/compiler.rs:10714)
- `tree_shakes_unused_foreign_imports_from_a_closed_js_module_entry` (src/compiler.rs:10757)
- `keeps_ambient_dom_host_names_that_are_not_foreign_imports` (src/compiler.rs:10793)
- `inlines_known_dom_host_calls_without_keeping_the_import` (src/compiler.rs:10809)
- `inlines_document_and_clone_host_calls` (src/compiler.rs:10846)
- `lowers_first_class_javascript_literals_without_a_host_import` (src/compiler.rs:10883)
- `preserves_explicit_null_this_in_first_class_javascript_calls` (src/compiler.rs:10913)
- `lowers_first_class_javascript_primitives` (src/compiler.rs:10938)
- `lowers_first_class_javascript_method_invocation` (src/compiler.rs:10998)
- `keeps_return_separated_from_an_inlined_nullish_operand` (src/compiler.rs:11025)
- `inlines_js_invoke_wrappers_to_direct_members` (src/compiler.rs:11056)
- `restores_direct_method_calls_only_for_the_same_receiver` (src/compiler.rs:11095)
- `rematerializes_same_receiver_member_calls_across_other_reads` (src/compiler.rs:11128)
- `sibling_javascript_members_keep_the_receiver` (src/compiler.rs:11208)
- `rematerializes_same_receiver_method_calls_on_this` (src/compiler.rs:11287)
- `keeps_this_on_same_receiver_member_calls_inside_nested_closures` (src/compiler.rs:11321)
- `lowers_first_class_javascript_string_and_regex_operations` (src/compiler.rs:11408)
- `compiles_inlined_aggregate_accumulators_with_scalar_replacement` (src/compiler.rs:11683)
- `ordinary_record_candidate_obeys_joint_search_allowlists` (src/compiler.rs:11820)
- `keeps_a_saved_previous_value_readable_across_its_own_update` (src/compiler.rs:11955) — its comment describes a runtime bug (loop body ran once); `steps(n)` is a good candidate for a hand-written case
- `candidate_search_keeps_a_valid_structured_for_in_baseline` (src/compiler.rs:15499)
- `source_written_i32_normalization_survives_every_javascript_objective` (src/compiler.rs:15988)
- `dead_source_written_i32_normalization_does_not_keep_dead_code_alive` (src/compiler.rs:16046)
- `search_off_module_merges_adjacent_generated_declarations` (src/compiler.rs:16394)
- `module_search_keeps_helpers_named_but_still_scores_dense_tables` (src/compiler.rs:17007)
- `nested_arrows_emit_captured_parameter_defaults` (src/compiler.rs:17237)
- `compiles_v01_control_flow` (src/compiler.rs:17250)
- `reconstructs_short_circuit_control_flow_without_a_state_machine` (src/compiler.rs:17260)
- `compiles_nested_control_flow_after_cfg_inlining` (src/compiler.rs:17268)
- `does_not_fold_array_length_across_mutation` (src/compiler.rs:17279)
- `keeps_array_length_stable_across_fill` (src/compiler.rs:17290)
- `inlines_disjoint_top_level_control_flow_regions` (src/compiler.rs:17300)
- `compiles_source_to_native_c` (src/compiler.rs:17314)
- `compiles_mutable_capture_cells_to_native_c` (src/compiler.rs:17321)
- `keeps_effectful_initializer_when_global_is_unread` (src/compiler.rs:17672)
- `emits_typed_host_objects_as_direct_stable_javascript` (src/compiler.rs:17679)
- `extern_class_members_stay_exact_when_owned_fields_reuse_the_name` (src/compiler.rs:17714)
- `closed_world_can_release_extern_class_fields_without_renaming_host_length` (src/compiler.rs:17769)
- `preserves_effectful_host_reads_and_eliminates_trusted_pure_calls` (src/compiler.rs:17804)
- `preserves_host_method_receivers_and_supports_callable_fields` (src/compiler.rs:17827)
- `treats_host_operations_as_aliasing_barriers` (src/compiler.rs:17847)
- `remaps_nominal_types_nested_in_imported_unions` (src/compiler.rs:17894)
- `emits_reusable_esm_with_mangled_live_exports` (src/compiler.rs:17981)
- `strip_console_removes_print_and_keeps_effectful_arguments` (src/compiler.rs:18011)
- `known_host_externs_lower_to_javascript_builtins` (src/compiler.rs:18034)
- `frequent_host_math_uses_a_mangled_builtin_alias` (src/compiler.rs:18062)
- `frequent_typeof_uses_a_shared_helper` (src/compiler.rs:18088)
- `default_config_strips_console` (src/compiler.rs:18114)
- `host_throw_emits_javascript_throw` (src/compiler.rs:18125)
- `host_array_push_and_has_own_drop_typescript_wrappers` (src/compiler.rs:18141)
- `known_host_window_and_timeout_drop_typescript_wrappers` (src/compiler.rs:18177)
- `known_host_function_and_window_predicates_share_compact_aliases` (src/compiler.rs:18228)
- `known_host_predicates_fold_on_fresh_objects` (src/compiler.rs:18251) — asserts fragments (`console.log(0)`, `"object"`), not the complete output
- `known_host_iterator_and_console_drop_typescript_wrappers` (src/compiler.rs:18281)
- `object_literal_symbol_assign_is_not_a_block` (src/compiler.rs:18321)
- `repeated_window_roots_share_one_binding` (src/compiler.rs:19138)
- `static_host_function_taken_as_value_needs_no_bind` (src/compiler.rs:19160)
- `brotli_selects_direct_object_has_own_but_retains_a_detached_alias` (src/compiler.rs:19263)
- `host_undefined_global_name_is_not_value_proof` (src/compiler.rs:19319)
- `applies_fine_grained_optimizer_and_mangling_config` (src/compiler.rs:19331)
- `can_mangle_public_esm_export_names` (src/compiler.rs:19781)
- `aliases_exported_host_globals_at_an_esm_boundary` (src/compiler.rs:19802)
- `materializes_aggregate_abi_for_exported_functions` (src/compiler.rs:19843)
- `collapses_unobserved_byte_array_buffer_construction` (src/compiler.rs:20008)
- `folds_fixed_typed_array_and_subarray_lengths` (src/compiler.rs:20018) — asserts `console.log(10)` appears, not the complete output
- `compiles_nested_capturing_closures_after_inlining` (src/compiler.rs:20028)
- `compiles_mutable_captures_as_shared_lexical_bindings` (src/compiler.rs:20036)
- `materializes_call_results_when_captured_by_closures` (src/compiler.rs:20175)
- `compiles_inferred_generics_to_optimized_javascript` (src/compiler.rs:20231)
- `compiles_union_values_and_heterogeneous_arrays` (src/compiler.rs:20240)
- `compiles_union_type_guards_and_narrowed_calls` (src/compiler.rs:20269)
- `a_string_element_read_keeps_its_guard_where_the_difference_shows` (src/compiler.rs:20793)

### C. diagnostic: the program is expected to be rejected (8)

- `foreign_imports_require_matching_extern_contracts` (src/compiler.rs:11511)
- `absolute_javascript_nesting_limit_rejects_every_oversized_candidate` (src/compiler.rs:16498)
- `renders_source_location` (src/compiler.rs:17228)
- `constructor_export_with_fields_respects_the_javascript_syntax_floor` (src/compiler.rs:17507)
- `reports_javascript_only_host_objects_for_native_targets` (src/compiler.rs:17816)
- `rejects_eager_cyclic_module_value_reads` (src/compiler.rs:20484)
- `rejects_conflicting_extern_contracts_across_modules` (src/compiler.rs:20520)
- `attributes_purity_errors_to_the_dependency_module` (src/compiler.rs:20567)

### D. asserts on old-route IR or an IR predicate (3)

- `ordinary_record_candidate_requires_closed_non_inherited_keys` (src/compiler.rs:11741)
- `only_canonical_source_i32_normalization_creates_an_obligation` (src/compiler.rs:16023)
- `lowered_source_operations_carry_node_ids` (src/compiler.rs:16594)

### E. JavaScript snippet, not LilScript (parser admission, declaration variants, artifact validation) (10)

- `gzip_scores_top_level_declaration_variants_exactly` (src/compiler.rs:11864)
- `declaration_leaves_consume_one_structural_plan_slot` (src/compiler.rs:12484)
- `generated_javascript_admission_rejects_invalid_code_before_codec` (src/compiler.rs:15708)
- `declaration_variants_are_admitted_before_scoring` (src/compiler.rs:15731)
- `observed_export_names_must_match_the_typed_abi` (src/compiler.rs:15744)
- `observed_javascript_must_retain_lowering_obligations` (src/compiler.rs:15773)
- `final_javascript_cannot_introduce_an_unclassified_static_property` (src/compiler.rs:15793)
- `terminal_cleanup_reopens_canonical_peephole_on_unprepared_finalist` (src/compiler.rs:16058)
- `standards_parser_rejects_malformed_conditional_sequence` (src/compiler.rs:4882)
- `standards_parser_accepts_script_and_module_artifacts` (src/compiler.rs:4888)

### F. configuration or policy unit test (5)

- `ssa_candidate_crossproduct_obeys_exact_allowlists` (src/compiler.rs:10608)
- `startup_guard_uses_independent_saturating_limits` (src/compiler.rs:16462)
- `javascript_priorities_rank_transfer_and_runtime_shape_independently` (src/compiler.rs:16625)
- `profile_template_lists_stable_function_and_loop_keys` (src/compiler.rs:16653)
- `truthy_nullable_checks_follow_the_priority_unless_configured` (src/compiler.rs:20812)

### G. bundle / chunk manifest layout (preserve-modules, split) (9)

- `configured_bundle_uses_requested_local_pool_without_changing_output` (src/compiler.rs:19388)
- `configured_all_bundle_lowers_once_and_matches_separate_outputs` (src/compiler.rs:19426)
- `preserves_surviving_dependency_functions_as_esm_chunks` (src/compiler.rs:19464)
- `preserve_modules_disables_ownerless_region_outlining` (src/compiler.rs:19517)
- `splits_only_shared_modules_that_meet_size_policy` (src/compiler.rs:19559)
- `split_does_not_force_a_costlier_first_shared_chunk` (src/compiler.rs:19614)
- `split_rejects_more_mandatory_lazy_chunks_than_max_chunks` (src/compiler.rs:19662)
- `split_emits_with_the_winning_joint_chunk_symbol_options` (src/compiler.rs:19698)
- `static_chunks_reference_host_globals_directly` (src/compiler.rs:19813)

### H. runtime parity only: two builds compared with each other, no expected value in the test (1)

- `a_nullable_object_test_is_strict_under_performance_first` (src/compiler.rs:20825) — hand evaluation gives `a:b:c:d` for the probe `step('a')..step('d')`; not harvested because the test states no value

### I. exact output asserted, but its stdout depends on a host value the test never supplies (1)

- `links_typed_host_interfaces_from_a_module_without_wrappers` (src/compiler.rs:20545) — asserts `console.log(document.title)`; stdout needs a host `document` the test never defines
