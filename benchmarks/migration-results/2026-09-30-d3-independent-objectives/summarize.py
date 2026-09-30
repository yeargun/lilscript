#!/usr/bin/env python3
"""Verify complete single-objective output preservation for the API change."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
OLD = HERE.parent / '2026-09-30-q2-statement-mentions'
WORK = Path('/home/azureuser/lilscript-work/measurements')


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    identity = read(HERE / 'identity.json')
    pin = Path('/home/azureuser/lilscript-work/bin/d3-independent-objectives-1')
    for name, expected in identity['binaries'].items():
        assert digest(pin / name) == expected
    before, after = [read(path / 'ratchet.json') for path in [OLD, HERE]]
    assert before['table'].keys() == after['table'].keys()
    assert after['items'] == 642 and not after['failures']
    generic = 0
    for item, lanes in before['table'].items():
        for metric, lane in {'raw': 'raw', 'gzip9': 'gzip', 'brotli11': 'brotli'}.items():
            rows = [report['table'][item][metric] for report in [before, after]]
            hashes = [digest(WORK / path.name / 'ratchet/out/compiler' / item / (lane + '.js'))
                      for path in [OLD, HERE]]
            assert all(row['state'] == 'pass' for row in rows)
            assert rows[0]['size'] == rows[1]['size'] and hashes[0] == hashes[1], (item, metric)
            assert all(hash_.startswith(row['sha256']) for hash_, row in zip(hashes, rows))
            generic += 1
    before, after = [read(path / 'ports.json') for path in [OLD, HERE]]
    assert not after['regressions'] and len(after['ports']) == len(before['ports']) == 3
    ports = 0
    for a, b in zip(before['ports'], after['ports']):
        assert a['port'] == b['port']
        def artifacts(port):
            return {row['path']: (row['sha256'], row['raw'], row['gzip9'], row['brotli11'])
                    for row in port['artifacts']}
        assert artifacts(a) == artifacts(b), a['port']
        ports += len(b['artifacts'])
    assert generic == 1926 and ports == 20
    result = {'schema': 1, **identity, 'generic_artifacts_unchanged': generic,
              'port_artifacts_unchanged': ports, 'summarizer_sha256': digest(Path(__file__)),
              'reports': {str(path / name): digest(path / name) for path in [OLD, HERE]
                          for name in ['ratchet.json', 'ports.json']}}
    (HERE / 'comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
