#!/usr/bin/env python3
"""Verify the level gate's measured artifacts and retained work receipts."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
OLD = HERE.parent / '2026-09-30-s1-primitive-assumptions'
WORK = Path('/home/azureuser/lilscript-work/measurements')
METRICS = {'raw': 'raw', 'gzip9': 'gzip', 'brotli11': 'brotli'}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def main():
    previous, candidate = (read(d / 'ratchet.json') for d in [OLD, HERE])
    assert previous['table'].keys() == candidate['table'].keys()
    assert candidate['items'] == 642 and not candidate['failures']
    for field in ['delivery', 'sets', 'filter', 'barsSha256']:
        assert previous[field] == candidate[field]
    summary = {'schema': 1, 'source': '79c1d607',
               'scope': 'development corpus and reference ports; no held-out qualification',
               'summarizer_sha256': digest(__file__),
               'reports': {str(d / name): digest(d / name) for d in [OLD, HERE]
                           for name in ['ratchet.json', 'ports.json']},
               'generic': {'compared': 1926, 'changed': []}, 'ports': {'changed': []}, 'cpu': {}}
    for report in [previous, candidate]:
        assert digest(report['compiler']['source']) == report['compiler']['sha256']
        for configuration in report['configurations']:
            assert digest(configuration['path']) == configuration['sha256']
    for item, lanes in previous['table'].items():
        for metric, lane in METRICS.items():
            before, after = lanes[metric], candidate['table'][item][metric]
            assert before['state'] == after['state'] == 'pass'
            hashes = [digest(WORK / d.name / 'ratchet/out/compiler' / item / (lane + '.js'))
                      for d in [OLD, HERE]]
            assert all(h.startswith(row['sha256']) for h, row in zip(hashes, [before, after]))
            assert after['size'] <= before['size'], (item, metric, 'growth')
            if hashes[0] != hashes[1]:
                summary['generic']['changed'].append({'item': item, 'metric': metric,
                    'before': before['size'], 'after': after['size'], 'sha256': hashes})
    before, after = (read(d / 'ports.json') for d in [OLD, HERE])
    assert not after['regressions']
    assert [p['port'] for p in before['ports']] == [p['port'] for p in after['ports']]
    count = 0
    for a, b in zip(before['ports'], after['ports']):
        old = {row['path']: row for row in a['artifacts']}
        assert old.keys() == {row['path'] for row in b['artifacts']}
        for row in b['artifacts']:
            count += 1
            previous = old[row['path']]
            if previous['sha256'] != row['sha256']:
                summary['ports']['changed'].append({'port': b['port'], 'path': row['path'],
                    'coordinates': {k: [previous[k], row[k]] for k in METRICS},
                    'sha256': [previous['sha256'], row['sha256']]})
    assert count == 20
    summary['ports']['compared'] = count
    for level in [13, 14]:
        path = HERE / f'compile-cost-{level}.json'
        report = read(path)
        assert report['complete'] and report['rounds'] == 3
        summary['reports'][str(path)] = digest(path)
        rows = []
        for row in report['workloads']:
            old, new = (row['warmup'][role]['explain'] for role in ['previous', 'candidate'])
            assert old['inputs'] == new['inputs'], (row['id'], 'input drift')
            assert old['javascript_policy']['effort'] == new['javascript_policy']['effort'] == level
            policy = new['javascript_policy']['objective']['search']
            assert policy['deferred_naming_starts'] == 'auto'
            assert policy['deferred_naming_starts_enabled'] == (level >= 14)
            assert policy['deferred_naming_polish'] is True
            a, b = (r['search']['terminal']['objectives'][0] for r in [old, new])
            if level == 13:
                assert old['search'] == new['search'], (row['id'], 'ordinary search changed')
                for key in ['baseline_work', 'optional_work', 'codec_work', 'peak_retained_bytes']:
                    assert old['resources'][key] == new['resources'][key], (row['id'], key)
                assert row['warmup']['previous']['sha256'] == row['warmup']['candidate']['sha256']
            else:
                assert b['after'] <= a['after']
            cpu = {role: row['summary'][role]['cpu_seconds'] for role in ['previous', 'candidate']}
            rows.append({'id': row['id'], 'codec': a['codec'], 'size': [a['after'], b['after']],
                         'judged': [a['judged'], b['judged']],
                         'codec_probes': [a['codec_probes'], b['codec_probes']],
                         'optional_work': [old['resources']['optional_work'], new['resources']['optional_work']],
                         'peak_retained_bytes': [old['resources']['peak_retained_bytes'], new['resources']['peak_retained_bytes']],
                         'cpu': cpu, 'ratio': row['cpu_ratio']})
        summary['cpu'][str(level)] = rows
    (HERE / 'comparison.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
