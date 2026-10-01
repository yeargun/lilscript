#!/usr/bin/env python3
import json
from pathlib import Path
import subprocess
import sys
binary = Path(sys.argv[1]).resolve()
root = Path(__file__).resolve().parent
names = [line.removesuffix(': test') for line in subprocess.check_output([str(binary), '--list'], text=True).splitlines() if line.endswith(': test')]
prefixes = ('program::javascript::reprint_tests::', 'admission_replay::tests::', 'js::cloning::tests::')
leaves = {
    'q2_formation_reuse_preserves_each_objectives_admission_and_bytes',
    'q2_normalization_reuse_preserves_logical_search_and_objective_bytes',
    'q2_dirty_target_schedule_matches_disabled_reuse_after_later_region_edits',
    'g3_optional_member_preserves_value_calls_and_outer_delete_references',
    'semantic_phase_timing_is_observational_and_covers_refusal',
}
selected = [name for name in names if name.startswith(prefixes) or name.rsplit('::', 1)[-1] in leaves]
missing = leaves - {name.rsplit('::', 1)[-1] for name in selected}
assert not missing, missing
assert all(any(name.startswith(prefix) for name in selected) for prefix in prefixes)
result = subprocess.run([str(binary), '--test-threads=1', '--exact', *selected], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
(root / 'focused.log').write_text(result.stdout)
(root / 'qualification.json').write_text(json.dumps({'algorithm':62, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused debug-library checks; no full library or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-12:]))
raise SystemExit(result.returncode)
