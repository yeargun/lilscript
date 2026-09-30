#!/usr/bin/env python3
"""Verify the complete reach-scope CPU comparison, including all search records."""
import copy
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def main():
    report = read(HERE / 'compile-cost.json')
    assert report['complete'] and report['rounds'] == 3 and len(report['workloads']) == 9
    for compiler in report['compilers'].values():
        assert digest(compiler['path']) == compiler['sha256']
    result = {'schema': 1, 'source': '82fdf87d11aa903ffcfb4565314ad56c7a07f4c7',
              'summarizer_sha256': digest(__file__),
              'reports': {'compile-cost.json': digest(HERE / 'compile-cost.json')}, 'workloads': []}
    for row in report['workloads']:
        before, after = [row['warmup'][role]['explain'] for role in ['previous', 'candidate']]
        assert before['inputs'] == after['inputs']
        policies = [copy.deepcopy(r['javascript_policy']) for r in [before, after]]
        assert [p.pop('algorithm') for p in policies] == [10, 13]
        assert policies[0] == policies[1], (row['id'], 'unexpected policy difference')
        assert before['search'] == after['search'], (row['id'], 'search opportunity changed')
        assert row['warmup']['previous']['sha256'] == row['warmup']['candidate']['sha256']
        for role, receipt in zip(['previous', 'candidate'], [before, after]):
            assert receipt['resources']['retained_bytes_after_handoff'] == 0
        old_cpu, new_cpu = [row['summary'][role]['cpu_seconds'] for role in ['previous', 'candidate']]
        phase = {role: row['warmup'][role]['phase_elapsed'] for role in ['previous', 'candidate']}
        result['workloads'].append({'id': row['id'], 'cpu_ratio': row['cpu_ratio'],
            'cpu': {'previous': old_cpu, 'candidate': new_cpu},
            'ranges_overlap': not (old_cpu['max'] < new_cpu['min'] or new_cpu['max'] < old_cpu['min']),
            'work': {key: [before['resources'][key], after['resources'][key]]
                     for key in ['baseline_work', 'optional_work', 'codec_work', 'peak_retained_bytes']},
            'phase_elapsed': phase})
    (HERE / 'comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    for row in result['workloads']:
        print(row['id'], row['cpu_ratio'], 'overlap', row['ranges_overlap'], 'peak', row['work']['peak_retained_bytes'])


if __name__ == '__main__':
    main()
