#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import sys
binary = Path(sys.argv[1]).resolve()
root = Path(__file__).resolve().parent
names = [line.removesuffix(': test') for line in subprocess.check_output([str(binary), '--list'], text=True).splitlines() if line.endswith(': test')]
prefixes = ('program::rules::admission_tests::', 'js::rules::contract_tests::', 'program::javascript::reprint_tests::')
leaves = {
    'q2_formation_reuse_preserves_each_objectives_admission_and_bytes',
    'q2_normalization_reuse_preserves_logical_search_and_objective_bytes',
    'cold_and_reused_rule_views_preserve_decisions_bytes_and_execution',
    's3_native_inlined_closure_owns_its_captured_bank_after_return',
    'default_transport_preserves_callee_guards_and_argument_effects',
    'host_annotations_do_not_authorize_common_coercions',
    'semantic_phase_timing_is_observational_and_covers_refusal',
}
selected = [name for name in names if name.startswith(prefixes) or name.rsplit('::', 1)[-1] in leaves]
missing = leaves - {name.rsplit('::', 1)[-1] for name in selected}
assert not missing, missing
assert all(any(name.startswith(prefix) for name in selected) for prefix in prefixes)
environment = os.environ.copy()
environment['LILSCRIPT_NATIVE_CLANG'] = '/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'
result = subprocess.run([str(binary), '--test-threads=1', '--exact', *selected], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=environment)
(root / 'focused.log').write_text(result.stdout)
(root / 'qualification.json').write_text(json.dumps({'algorithm':63, 'tests':selected, 'count':len(selected), 'exit_code':result.returncode, 'scope':'Focused debug-library checks; no full library or fleet run.'}, indent=2)+'\n')
print('\n'.join(result.stdout.splitlines()[-14:]))
raise SystemExit(result.returncode)
