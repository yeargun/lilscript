#!/usr/bin/env python3
"""Verify byte preservation and compare the statement-cache work receipts."""
import copy
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
OLD = HERE.parent / '2026-09-30-q3-level14'
WORK = Path('/home/azureuser/lilscript-work/measurements')
METRICS = {'raw': 'raw', 'gzip9': 'gzip', 'brotli11': 'brotli'}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def main():
    a, b = [read(d / 'ratchet.json') for d in [OLD, HERE]]
    assert a['table'].keys() == b['table'].keys() and b['items'] == 642 and not b['failures']
    for item, lanes in a['table'].items():
        for metric, lane in METRICS.items():
            rows = [r['table'][item][metric] for r in [a, b]]
            hashes = [digest(WORK / d.name / 'ratchet/out/compiler' / item / (lane + '.js'))
                      for d in [OLD, HERE]]
            assert all(row['state'] == 'pass' for row in rows)
            assert rows[0]['size'] == rows[1]['size'] and hashes[0] == hashes[1], (item, metric)
            assert all(h.startswith(r['sha256']) for h, r in zip(hashes, rows))
    old, new = [read(d / 'ports.json') for d in [OLD, HERE]]
    assert not new['regressions']
    count = 0
    for a, b in zip(old['ports'], new['ports']):
        assert a['port'] == b['port']
        artifacts = lambda port: {r['path']: (r['sha256'], r['raw'], r['gzip9'], r['brotli11'])
                                 for r in port['artifacts']}
        assert artifacts(a) == artifacts(b), a['port']
        count += len(b['artifacts'])
    assert count == 20
    report = read(HERE / 'compile-cost.json')
    assert report['complete'] and report['rounds'] == 3 and len(report['workloads']) == 9
    for compiler in report['compilers'].values():
        assert digest(compiler['path']) == compiler['sha256']
    result = {'schema': 1, 'source': '8bc28c1f', 'generic_artifacts_unchanged': 1926,
              'port_artifacts_unchanged': count, 'summarizer_sha256': digest(__file__),
              'reports': {str(d / name): digest(d / name) for d in [OLD, HERE]
                          for name in ['ratchet.json', 'ports.json']}, 'workloads': []}
    result['reports'][str(HERE / 'compile-cost.json')] = digest(HERE / 'compile-cost.json')
    for row in report['workloads']:
        before, after = [row['warmup'][role]['explain'] for role in ['previous', 'candidate']]
        assert before['inputs'] == after['inputs']
        policies = [copy.deepcopy(r['javascript_policy']) for r in [before, after]]
        assert [p.pop('algorithm') for p in policies] == [7, 8]
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
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
