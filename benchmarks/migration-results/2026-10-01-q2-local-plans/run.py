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
prefixes = ('program::dataflow::tests::', 'program::rules::call_tests::')
leaf_names = {
    'q2_local_dirty_normalization_preserves_edits_and_runtime_with_dense_rules',
    'q2_local_dirty_admission_refuses_cleanly_and_tracks_newly_created_bodies',
    'q2_stable_local_plans_replay_identical_logical_work_and_peak_storage',
    'q2_source_edits_publish_exact_storage_across_grafts_signatures_and_aggregates',
    'q2_source_edit_refusals_do_not_publish_partial_programs_or_leak_reservations',
    'cold_and_reused_rule_views_preserve_decisions_bytes_and_execution',
    'a_false_branch_leaves_with_the_function_only_it_called',
    'a_loop_whose_test_is_false_never_runs',
    'recursion_stays_callable_and_terminal_returns_inline',
    'a_folded_exit_leaves_one_return_that_inlines',
    'an_argument_is_not_read_after_something_observable',
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
(root / 'qualification.json').write_text(json.dumps({'algorithm':60, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused debug-library checks; no full library, release, ratchet or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-6:]))
raise SystemExit(result.returncode)
