#!/usr/bin/env python3
"""Reproduce and qualify retained effort winners under finite work ceilings."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'tests/cases/objective_judged_spellings.lil'
EXPECTED = SOURCE.with_suffix('.out')
BUDGETS = [2_000_000, 5_000_000, 10_000_000]
CODECS = {'raw': 'raw', 'gzip': 'gzip9', 'brotli': 'brotli11'}


def identity(path):
    path = Path(path).resolve()
    return {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['previous', 'candidate', 'node', 'work', 'json']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    args.work = args.work.resolve()
    args.work.mkdir(parents=True, exist_ok=False)
    if args.json.exists():
        raise ValueError('keep the existing evidence and choose a new report')
    spec = importlib.util.spec_from_file_location('cost', ROOT / 'scripts/compile-cost.py')
    cost = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cost)
    report = {'schema': 1, 'complete': False, 'source': identity(SOURCE), 'expected': identity(EXPECTED),
              'tools': {key: identity(getattr(args, key)) for key in ['previous', 'candidate', 'node']},
              'driver': identity(__file__), 'rows': []}
    expected = EXPECTED.read_text()
    args.json.parent.mkdir(parents=True, exist_ok=True)
    try:
        for codec, metric in CODECS.items():
            for budget in BUDGETS:
                config = args.work / f'{codec}-{budget}.toml'
                config.write_text(f'policy.version=3\neffort.level=15\nobjective.codecs="{codec}"\n')
                row = {'codec': codec, 'work_ceiling': budget, 'results': {}}
                report['rows'].append(row)
                for label in ['previous', 'candidate']:
                    stem = f'{label}-{codec}-{budget}'
                    output = args.work / (stem + '.mjs')
                    command = [getattr(args, label), SOURCE, '--config', config, '--target', 'js-module',
                               '--mode', 'production', '--logical-work', str(budget), '--explain', 'json', '-o', output]
                    compiled = subprocess.run(list(map(str, command)), capture_output=True, text=True, timeout=120)
                    (args.work / (stem + '.compile.log')).write_text(compiled.stderr)
                    if compiled.returncode:
                        raise RuntimeError(f'{stem}: compile failed, see retained log')
                    explanation = next(doc for doc in cost.json_documents(compiled.stderr) if 'inputs' in doc)
                    selected = explanation['artifacts'][explanation['winners'][list(CODECS).index(codec)]]
                    actual = subprocess.run([args.node, output], capture_output=True, text=True, timeout=30)
                    (args.work / (stem + '.stdout')).write_text(actual.stdout)
                    if actual.returncode or actual.stdout != expected:
                        raise RuntimeError(f'{stem}: independent runtime golden differs: {actual.stderr}')
                    checkpoints = explanation['search']['terminal']['effort_checkpoints']
                    lost = [point for point in checkpoints if point['objectives'][0][1] < selected[metric]]
                    resources = {key: explanation['resources'][key]
                                 for key in ['baseline_work', 'optional_work', 'retained_bytes_after_handoff']}
                    if resources['baseline_work'] + resources['optional_work'] > budget or resources['retained_bytes_after_handoff']:
                        raise RuntimeError('resource ceiling or lifetime violation')
                    row['results'][label] = {'selected_bytes': selected[metric], 'artifact': identity(output),
                        'checkpoints': checkpoints, 'lost_completed_winners': lost,
                        'stop': explanation['search'].get('stop'), 'resources': resources, 'runtime': 'passed'}
                    if label == 'candidate' and lost:
                        raise RuntimeError(f'{stem}: a completed smaller winner was discarded')
                row['delta'] = row['results']['candidate']['selected_bytes'] - row['results']['previous']['selected_bytes']
                print(codec, budget, row['delta'], flush=True)
        if not any(row['results']['previous']['lost_completed_winners'] for row in report['rows']):
            raise RuntimeError('the previous compiler did not reproduce the defect')
        for value in [*report['tools'].values(), report['source'], report['expected'], report['driver']]:
            if identity(value['path']) != value:
                raise RuntimeError('experiment input changed')
        report['complete'] = True
    finally:
        args.json.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
