#!/usr/bin/env python3
"""Author deterministic, explicitly synthetic scaling copies of the integration fixture.

This does not build or execute a compiler. Original input files are archived
byte-for-byte under provenance/base; generated sources are labeled separately.
Every added arithmetic step contributes to editTarget's return value, and every
generated module is imported and called. Both shapes preserve its 11 -> 12 edit.
"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = ROOT / 'src/semantic_program/fixtures/integrated-architecture'
MAX_SIZE = 8192
JS_MODULES = {
    'entry.lil', 'editable.lil', 'products.lil', 'marked-api.lil',
    'marked/rules.lil', 'marked/str-slice.lil',
    'factory/entry.lil', 'factory/barrel.lil', 'factory/factory.lil',
    'factory/state.lil', 'factory/boot.lil', 'factory/public.lil',
}
NATIVE_MODULES = {'native-entry.lil', 'products.lil'}
ORACLE_FILES = {'setup.js', 'host.js', 'expected.json', 'native.expected.out',
                'config.toml', 'origin.json'}
EDITABLE = (b'// This public result is the paired retained/exclusive owner edit observation.\n'
            b'export int editTarget(){return 11;}\n')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encoded_json(value):
    return (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()


def read_base(fixture):
    if not fixture.is_dir():
        raise ValueError('fixture must be an existing directory')
    files = {}
    for path in sorted(fixture.rglob('*')):
        if path.is_symlink():
            raise ValueError('fixture must contain ordinary files/directories, not symlinks')
        name = path.relative_to(fixture).as_posix()
        if name.split('/')[0] in ('provenance', 'scaling', 'scaling-manifest.json'):
            raise ValueError('input must be the original fixture, not an existing scaling copy')
        if path.is_file():
            files[name] = path.read_bytes()
        elif not path.is_dir():
            raise ValueError('fixture contains a non-regular file')
    missing = (JS_MODULES | NATIVE_MODULES | ORACLE_FILES) - files.keys()
    if missing:
        raise ValueError('missing fixture files: ' + ', '.join(sorted(missing)))
    if files['editable.lil'] != EDITABLE:
        raise ValueError('editTarget template changed; review the generator before scaling it')
    expected = json.loads(files['expected.json'])
    if not isinstance(expected, list) or len(expected) != 60 or expected[-1] != ['edit', 11]:
        raise ValueError('the fixed 60-event golden ending in editTarget() == 11 changed')
    return files


def step_name(index):
    return f'shapeStep{index:05}'


def step_file(index):
    return f'scaling/step-{index:05}.lil'


def generated_sources(shape, size):
    comment = '// Authored synthetic scaling; original templates are in provenance/base.\n'
    if shape == 'large-function':
        body = [comment, 'export int editTarget() {\n', '    int value = 11;\n']
        for _ in range(size):
            body.extend(('    value = value + 1;\n', '    value = value - 1;\n'))
        body.extend(('    return value;\n', '}\n'))
        return {'editable.lil': ''.join(body).encode()}

    files = {'editable.lil': (
        comment + f'import {{{step_name(0)}}} from "./{step_file(0)}";\n'
        + f'export int editTarget(){{return {step_name(0)}(11);}}\n').encode()}
    for index in range(size):
        children = [child for child in (2 * index + 1, 2 * index + 2) if child < size]
        body = [comment]
        for child in children:
            body.append(f'import {{{step_name(child)}}} from "./step-{child:05}.lil";\n')
        body.append(f'export int {step_name(index)}(int value) {{\n')
        for child in children:
            body.append(f'    value = {step_name(child)}(value);\n')
        body.extend(('    value = value + 1;\n', '    value = value - 1;\n',
                     '    return value;\n', '}\n'))
        files[step_file(index)] = ''.join(body).encode()
    return files


def generate(fixture, output, shape, size):
    if type(size) is not int or not 1 <= size <= MAX_SIZE:
        raise ValueError(f'size must be an integer from 1 through {MAX_SIZE}')
    if shape not in ('large-function', 'many-modules'):
        raise ValueError('unknown scaling shape')
    fixture = Path(fixture).resolve(strict=True)
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise ValueError('output must be a NEW directory; existing paths are never overwritten')
    if output.resolve().is_relative_to(fixture):
        raise ValueError('output must be outside the original fixture')
    original = read_base(fixture)
    generated = generated_sources(shape, size)
    payloads = {}
    rows = []
    for name, data in original.items():
        digest = sha(data)
        archived = 'provenance/base/' + name
        payloads[archived] = data
        rows.append({'file': archived, 'bytes': len(data), 'sha256': digest,
                     'role': 'original-template', 'source_file': name,
                     'source_sha256': digest})
        if name not in generated:
            payloads[name] = data
            rows.append({'file': name, 'bytes': len(data), 'sha256': digest,
                         'role': 'base-copy', 'source_file': name,
                         'source_sha256': digest})
    for name, data in generated.items():
        payloads[name] = data
        rows.append({'file': name, 'bytes': len(data), 'sha256': sha(data),
                     'role': 'generated-source'})
    javascript = JS_MODULES | (set(generated) - {'editable.lil'})
    base_files = [{'file': name, 'bytes': len(data), 'sha256': sha(data)}
                  for name, data in sorted(original.items())]
    manifest = {
        'schema_version': 1,
        'classification': 'synthetic-scaling',
        'shape': shape,
        'size': size,
        'generator': {'file': 'finer/tools/semantic-integration-shapes.py',
                      'sha256': sha(Path(__file__).read_bytes())},
        'original_fixture': {
            'archive': 'provenance/base',
            'files': base_files,
            'inventory_sha256': sha(encoded_json(base_files)),
            'inventory_encoding': 'UTF-8 JSON with sorted keys, indent=2, trailing newline',
            'origin_sha256': sha(original['origin.json']),
        },
        'edit_target': {
            'file': 'editable.lil', 'function': 'editTarget',
            'original_source_sha256': sha(original['editable.lil']),
            'baseline_value': 11, 'edited_value': 12,
            'patch': 'the sole Integer(11) in the editTarget semantic body',
        },
        'oracle': {'file': 'expected.json', 'sha256': sha(original['expected.json']),
                   'events': 60, 'final_event': ['edit', 11]},
        'javascript_modules': sorted(javascript),
        'native_modules': sorted(NATIVE_MODULES),
        'files': sorted(rows, key=lambda row: row['file']),
    }
    # mkdir is the ownership boundary: a racing/pre-existing destination cannot
    # be replaced. Only a directory successfully created here is cleaned up.
    output.mkdir(parents=True, exist_ok=False)
    try:
        for name, data in sorted(payloads.items()):
            path = output / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        (output / 'scaling-manifest.json').write_bytes(encoded_json(manifest))
    except BaseException:
        shutil.rmtree(output)
        raise
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, default=DEFAULT_FIXTURE,
                        help='original integrated-architecture fixture directory')
    parser.add_argument('--shape', choices=('large-function', 'many-modules'), required=True)
    parser.add_argument('--size', type=int, required=True,
                        help=f'consumed arithmetic pairs or imported modules (1..{MAX_SIZE})')
    parser.add_argument('--output', type=Path, required=True, help='NEW output fixture directory')
    args = parser.parse_args()
    try:
        manifest = generate(args.fixture, args.output, args.shape, args.size)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps({'manifest': str(args.output / 'scaling-manifest.json'),
                      'classification': manifest['classification'],
                      'shape': args.shape, 'size': args.size,
                      'javascript_modules': len(manifest['javascript_modules'])}, sort_keys=True))


if __name__ == '__main__':
    main()
