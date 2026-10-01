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
    'program::rules::call_tests::',
    'js::print::number_spelling_tests::',
    'text_measure::tests::',
)
leaf_names = {
    'q2_legality_probes_distinguish_decline_from_refusal_and_drop_temporaries',
    'q2_source_edits_publish_exact_storage_across_grafts_signatures_and_aggregates',
    'q2_source_edit_refusals_do_not_publish_partial_programs_or_leak_reservations',
    'cold_and_reused_rule_views_preserve_decisions_bytes_and_execution',
    'a_false_branch_leaves_with_the_function_only_it_called',
    'conditional_values_keep_the_operand_that_runs',
    'a_loop_whose_test_is_false_never_runs',
    'the_default_fold_bound_keeps_large_constants_as_expressions',
    'a_fold_counts_what_the_output_may_name_at_one_character',
    'nested_constant_structures_fold_over_rounds',
    'bitwise_identities_of_an_int_are_the_int',
    'a_function_called_once_moves_into_its_caller',
    'a_repeated_nonempty_body_stays_available_to_objective_judging',
    'recursion_stays_callable_and_terminal_returns_inline',
    'a_copied_body_reads_the_storage_its_function_captured',
    'an_arrow_held_in_a_local_is_inlined_at_its_calls',
    'an_argument_is_not_read_after_something_observable',
    'a_result_leaves_a_scoped_copy_through_private_storage',
    'an_unread_parameter_leaves_every_call',
    'a_parameter_every_call_passes_the_same_constant_becomes_it',
    'a_result_no_call_uses_leaves',
    'a_published_signature_stays',
    'a_signature_changes_for_every_function_that_shares_it',
    'default_transport_keeps_observed_argument_count_and_signed_zero',
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
(root / 'qualification.json').write_text(json.dumps({'algorithm':57, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused debug-library checks; no full library, release, ratchet or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-6:]))
raise SystemExit(result.returncode)
