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
prefixes = ('program::classes::tests::',)
leaf_names = {
    'q2_stable_local_plans_replay_identical_logical_work_and_peak_storage',
    'q2_source_edits_publish_exact_storage_across_grafts_signatures_and_aggregates',
    'q2_source_edit_refusals_do_not_publish_partial_programs_or_leak_reservations',
    'cold_and_reused_rule_views_preserve_decisions_bytes_and_execution',
    'immutable_scalar_cells_and_common_primitive_operations_forward',
    'host_annotations_do_not_authorize_common_coercions',
    'forwarding_preserves_snapshots_writes_and_argument_evaluation',
    'default_transport_preserves_callee_guards_and_argument_effects',
    'default_transport_keeps_observed_argument_count_and_signed_zero',
    'constant_call_folding_preserves_argument_effects_and_mutable_captures',
    'constant_scalar_methods_fold_through_nested_pure_calls',
    'a_settled_root_condition_chooses_its_branch_only_when_sealed',
    'exact_values_cross_settled_cells_parameters_and_results',
    'a_script_keeps_its_root_bindings',
    's2_field_constants_respect_alias_writers',
    's2_open_script_roots_remain_observable',
    's2_early_capture_and_throwing_initializers_keep_failure_order',
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
(root / 'qualification.json').write_text(json.dumps({'algorithm':61, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused debug-library checks; no full library, release, ratchet or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-6:]))
raise SystemExit(result.returncode)
