#!/usr/bin/env python3
"""Run the affected checks once, against an explicitly built library test binary."""
import json
import os
from pathlib import Path
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(__file__).resolve().parent
names = [line.removesuffix(': test') for line in subprocess.check_output([str(binary), '--list'], text=True).splitlines() if line.endswith(': test')]
prefixes = (
    'program::facts_tests::',
    'program::rules::evaluate::tests::',
    'program::rules::evaluate::scalar_tests::',
)
leaf_names = {
    'q2_exact_value_analysis_releases_answers_and_propagates_nested_refusals',
    'q2_source_edits_publish_exact_storage_across_grafts_signatures_and_aggregates',
    'q2_source_edit_refusals_do_not_publish_partial_programs_or_leak_reservations',
    'cold_and_reused_rule_views_preserve_decisions_bytes_and_execution',
    'constant_calls_are_evaluated_for_each_argument_tuple',
    'finite_call_sets_fold_a_common_branch_without_sampling_overflow',
    'finite_values_are_refined_on_each_branch_path',
    'primitive_truthiness_in_finite_paths_matches_javascript',
    'completed_constant_loops_preserve_control_flow_and_iteration_state',
    'constant_loops_keep_effects_and_calls_that_exhaust_evaluation',
    'constant_loop_evaluation_preserves_observable_exceptions',
    'constant_call_folding_preserves_argument_effects_and_mutable_captures',
    'known_methods_fold_only_with_the_builtin_contract',
    'constant_scalar_methods_fold_through_nested_pure_calls',
    'constant_numeric_conversion_needs_no_pristine_host_assumption',
    'bounded_constant_calls_preserve_nontermination_and_throwing_methods',
    'a_settled_root_condition_chooses_its_branch_only_when_sealed',
    'exact_values_cross_settled_cells_parameters_and_results',
    'a_parameter_every_call_passes_the_same_constant_becomes_it',
    's3_native_inlined_closure_owns_its_captured_bank_after_return',
}
selected = [name for name in names if name.startswith(prefixes) or name.startswith('program::rules::') and name.rsplit('::', 1)[-1] in leaf_names]
missing = leaf_names - {name.rsplit('::', 1)[-1] for name in selected}
assert not missing, missing
assert all(any(name.startswith(prefix) for name in selected) for prefix in prefixes)
environment = os.environ.copy()
if len(sys.argv) > 2:
    environment['LILSCRIPT_NATIVE_CLANG'] = str(Path(sys.argv[2]).resolve())
result = subprocess.run([str(binary), '--test-threads=1', '--exact', *selected], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=environment)
(root / 'focused.log').write_text(result.stdout)
(root / 'qualification.json').write_text(json.dumps({'algorithm':59, 'local_facts':16, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused debug-library checks; no full library, release, ratchet or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-6:]))
raise SystemExit(result.returncode)
