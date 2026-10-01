#!/usr/bin/env python3
"""Freeze objective ordering from C3 training; never read evaluation scores."""
from collections import defaultdict
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parent
source = root.parent / '2026-10-01-c3-complete/training.json'
training = json.loads(source.read_text())
assert training['complete'] and training['split'] == 'training'
data = defaultdict(lambda: defaultdict(lambda: {'attempts': 0, 'saved': 0, 'families': set()}))
for row in training['rows']:
    assert row['split'] == 'training'
    receipt = row['modes']['on']['explanation']
    raw = Path(receipt['path']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == receipt['sha256']
    text = raw.decode()
    report = json.JSONDecoder().raw_decode(text[text.index('{'):])[0]
    for stage in report['search']['terminal']['objectives']:
        for trial in stage['trials']:
            item = data[row['objective']][trial['challenger']]
            if trial['outcome'] in ('kept', 'rejected', 'pruned', 'identical'):
                item['attempts'] += 1
            if trial['outcome'] == 'kept':
                item['saved'] -= trial['delta']
                item['families'].add(row['family'])
result = {'training_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
          'selection': 'At least two generic source families with a kept win; descending summed exact savings; tie by name.',
          'scope': 'Ordering hypothesis only; preserves prior incumbent and uses fresh exact artifact judgment. No default seed or pruning-margin change.',
          'automatic_effort': 14, 'explicit_on_effort': 13, 'objectives': {}}
for codec, values in sorted(data.items()):
    ordered = sorted(values.items(), key=lambda pair: (-pair[1]['saved'], pair[0]))
    result['objectives'][codec] = {
        'promoted': [name for name, value in ordered if value['saved'] > 0 and len(value['families']) >= 2],
        'evidence': {name: {**value, 'families': sorted(value['families'])} for name, value in ordered},
    }
(root / 'objective-prior.json').write_text(json.dumps(result, indent=2) + '\n')
print({codec: value['promoted'] for codec, value in result['objectives'].items()})
