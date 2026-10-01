#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import sys
binary = Path(sys.argv[1]).resolve()
root = Path(__file__).resolve().parent
names = [line.removesuffix(': test') for line in subprocess.check_output([str(binary), '--list'], text=True).splitlines() if line.endswith(': test')]
prefixes = ('js::tables::tests::',)
leaves = {
    'independent_baselines_precede_optional_work_and_survive_exhaustion',
    'independent_objective_callback_unwind_restores_allowance_and_releases_handoffs',
    'two_objective_budget_counts_only_requested_baselines_and_shares',
    'the_stage_keeps_a_challenger_that_shrinks_the_artifact_and_rejects_one_that_grows_it',
    'a_vetoed_family_is_never_formed',
    'each_objective_vetoes_reconstruction_before_forming_search_candidates',
    'independent_search_off_preserves_each_objectives_direct_spelling',
    'q1_schedule_keeps_negative_estimates_and_bounded_cross_family_moves',
}
selected = [name for name in names if name.startswith(prefixes) or name.rsplit('::', 1)[-1] in leaves or name.rsplit('::',1)[-1].startswith('q3_')]
missing = leaves - {name.rsplit('::', 1)[-1] for name in selected}
assert not missing, missing
result = subprocess.run([str(binary), '--test-threads=1', '--exact', *selected], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
(root / 'focused.log').write_text(result.stdout)
(root / 'qualification.json').write_text(json.dumps({'algorithm':64, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused search, data, worker, independent-objective, ownership and permission checks; no full library or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-25:]))
raise SystemExit(result.returncode)
