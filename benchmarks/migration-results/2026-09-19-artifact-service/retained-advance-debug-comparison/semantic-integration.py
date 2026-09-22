#!/usr/bin/env python3
"""Execute the public architecture driver and qualify its actual artifacts.

This tool never builds the compiler. Build/pin provenance belongs to the parent
receipt. GNU time measures the whole child; external artifact execution and
codec verification occur afterwards. No failed case is removed from a report.
"""
import argparse
from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
import time
import tomllib


JS_MODULES = {
    'entry.lil', 'editable.lil', 'products.lil', 'marked-api.lil',
    'marked/rules.lil', 'marked/str-slice.lil',
    'factory/entry.lil', 'factory/barrel.lil', 'factory/factory.lil',
    'factory/state.lil', 'factory/boot.lil', 'factory/public.lil',
}
NATIVE_MODULES = {'native-entry.lil', 'products.lil'}
NATIVE_FLAGS = ['-std=c11', '-fno-fast-math', '-ffp-contract=off', '-O2']


def require(condition, message):
    # Qualification remains active under PYTHONOPTIMIZE/python -O too.
    if not condition:
        raise ValueError(message)


def nonnegative_integer(value):
    return type(value) is int and value >= 0


def json_equal(left, right):
    # Python structural equality equates true/false with 1/0. Keep the fixed
    # observation's JSON types, as the other artifact evidence owners do.
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def binary_provenance(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'sha256': sha(path)}


def local_file(fixture, name):
    require(isinstance(name, str) and bool(name) and not Path(name).is_absolute()
            and '..' not in Path(name).parts, 'manifest file must be fixture-relative')
    path = (fixture / name).resolve()
    require(path.is_relative_to(fixture.resolve()), 'manifest file escapes the fixture')
    return path


def required_modules(fixture, native=False):
    """Validate the declared scaling grammar, never derive it from driver output."""
    manifest_path = fixture / 'scaling-manifest.json'
    if not manifest_path.exists():
        return set(NATIVE_MODULES if native else JS_MODULES)
    manifest = json.loads(manifest_path.read_text())
    require(manifest['schema_version'] == 1 and manifest['classification'] == 'synthetic-scaling',
            'unknown scaling manifest contract')
    require(manifest['shape'] in ('large-function', 'many-modules'), 'unknown scaling shape')
    size = manifest['size']
    require(type(size) is int and 0 < size <= 8192, 'bounded positive scaling size required')
    generated = {f'scaling/step-{index:05}.lil' for index in range(size)} if manifest['shape'] == 'many-modules' else set()
    javascript = JS_MODULES | generated
    require(manifest['javascript_modules'] == sorted(javascript), 'scaling JavaScript module set differs from the declared grammar')
    require(manifest['native_modules'] == sorted(NATIVE_MODULES), 'scaling changed the portable module set')
    rows = manifest['files']
    require(isinstance(rows, list), 'scaling file rows required')
    names = [row['file'] for row in rows]
    actual = {str(path.relative_to(fixture)) for path in fixture.rglob('*')
              if path.is_file() and path != manifest_path}
    require(len(names) == len(set(names)) and set(names) == actual,
            'scaling manifest must cover every archived/generated file exactly once')
    generated_rows = set()
    for row in rows:
        path = local_file(fixture, row['file'])
        require(path.stat().st_size == row['bytes'] and sha(path) == row['sha256'], 'scaling file bytes changed')
        require(row['role'] in ('base-copy', 'original-template', 'generated-source'), 'unknown scaling file role')
        if row['role'] == 'generated-source':
            generated_rows.add(row['file'])
            require('source_file' not in row and 'source_sha256' not in row, 'generated source cannot claim byte-exact copying')
        else:
            original = local_file(fixture / 'provenance/base', row['source_file'])
            require(sha(original) == row['source_sha256'] == row['sha256'], 'original template provenance differs from copied bytes')
    require(generated_rows == generated | {'editable.lil'}, 'unexpected generated source outside the scaling grammar')
    edit = manifest['edit_target']
    require(edit['file'] == 'editable.lil' and edit['function'] == 'editTarget'
            and type(edit['baseline_value']) is int and edit['baseline_value'] == 11
            and type(edit['edited_value']) is int and edit['edited_value'] == 12,
            'scaling changed the fixed edit observation contract')
    require(edit['original_source_sha256'] == sha(fixture / 'provenance/base/editable.lil'),
            'scaling edit template provenance changed')
    return set(NATIVE_MODULES if native else javascript)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


@dataclass(frozen=True)
class InputFile:
    path: Path
    bytes: int
    sha256: str


@dataclass(frozen=True)
class IntegrationInputs:
    kind: str
    root: Path
    entry: Path
    config: Path
    modules: tuple[Path, ...]
    setup: Path | None
    host: Path | None
    golden: Path
    allowed_modes: tuple[str, ...]
    files: tuple[InputFile, ...]
    manifest: InputFile | None
    complete_inventory: bool

    def describe(self):
        return {'kind': self.kind, 'root': str(self.root), 'entry': str(self.entry),
                'config': str(self.config), 'modules': [str(path) for path in self.modules],
                'setup': str(self.setup) if self.setup else None,
                'host': str(self.host) if self.host else None, 'golden': str(self.golden),
                'allowed_modes': list(self.allowed_modes),
                'files': [{'path': str(item.path), 'bytes': item.bytes, 'sha256': item.sha256}
                          for item in self.files],
                'manifest': {'path': str(self.manifest.path), 'bytes': self.manifest.bytes,
                             'sha256': self.manifest.sha256} if self.manifest else None,
                'complete_inventory': self.complete_inventory}


def input_file(path):
    path = Path(path).resolve(strict=True)
    payload = path.read_bytes()
    return InputFile(path, len(payload), hashlib.sha256(payload).hexdigest())


def validate_inputs(inputs, mode):
    require(mode in inputs.allowed_modes, 'input contract does not declare mode: ' + mode)
    rows = inputs.files + ((inputs.manifest,) if inputs.manifest else ())
    for expected in rows:
        require(input_file(expected.path) == expected, 'declared input changed: ' + str(expected.path))
    if inputs.complete_inventory:
        actual = {path.resolve() for path in inputs.root.rglob('*') if path.is_file()}
        require(actual == {item.path for item in inputs.files}, 'complete original/scaling fixture inventory changed')


def resolve_inputs(fixture, mode, manifest=None):
    """Resolve one declared input set; neither report nor emitted bytes select it."""
    root = Path(fixture).resolve(strict=True)
    if manifest is None:
        native = mode == 'native'
        # Keep the original/scaling grammar and its full inventory validation.
        modules = tuple(sorted(local_file(root, name) for name in required_modules(root, native)))
        files = tuple(input_file(path) for path in sorted(root.rglob('*')) if path.is_file())
        result = IntegrationInputs(
            'original-or-scaling', root, root / ('native-entry.lil' if native else 'entry.lil'),
            root / 'config.toml', modules, None if native else root / 'setup.js',
            None if native else root / 'host.js', root / ('native.expected.out' if native else 'expected.json'),
            ('native',) if native else ('direct', 'search', 'retain', 'advance'), files, None, True)
    else:
        selected = input_file(manifest)
        def unique(pairs):
            result = {}
            for key, value in pairs:
                require(key not in result, 'duplicate input manifest key: ' + key)
                result[key] = value
            return result
        declaration = json.loads(selected.path.read_text(), object_pairs_hook=unique)
        require(type(declaration) is dict and set(declaration) == {
            'schema_version', 'kind', 'entry', 'config', 'setup', 'host', 'golden',
            'allowed_modes', 'javascript_modules', 'files', 'provenance'}, 'unknown declared input schema')
        require(type(declaration['schema_version']) is int and declaration['schema_version'] == 1
                and declaration['kind'] == 'integrated-observation', 'unsupported input contract kind/version')
        roles = {'entry': 'integrated-observation/entry.lil',
                 'config': 'integrated-architecture/config.toml',
                 'setup': 'integrated-architecture/setup.js',
                 'host': 'integrated-observation/host.js',
                 'golden': 'integrated-observation/expected.json'}
        require(all(declaration[name] == relative for name, relative in roles.items()),
                'companion entry/shared-config/shared-setup/host/golden roles differ')
        require(declaration['allowed_modes'] == ['direct', 'search'], 'companion declares unsupported modes')
        modules = {'integrated-architecture/' + name for name in JS_MODULES} | {roles['entry']}
        require(declaration['javascript_modules'] == sorted(modules), 'companion must declare the exact thirteen original module paths')
        expected_names = modules | set(roles.values())
        rows = declaration['files']
        require(type(rows) is list and all(type(row) is dict and set(row) == {'file', 'bytes', 'sha256'} for row in rows),
                'declared input file records differ')
        require(len(rows) == len(expected_names) and {row['file'] for row in rows} == expected_names,
                'declared files must cover every source and role exactly once')
        files = []
        for row in rows:
            require(nonnegative_integer(row['bytes']) and type(row['sha256']) is str
                    and len(row['sha256']) == 64 and all(c in '0123456789abcdef' for c in row['sha256']),
                    'invalid input length/hash')
            actual = input_file(local_file(root, row['file']))
            require(actual.bytes == row['bytes'] and actual.sha256 == row['sha256'], 'declared source/input bytes differ: ' + row['file'])
            files.append(actual)
        require(type(declaration['provenance']) is dict and set(declaration['provenance']) == {'source_manifest_sha256', 'baseline_pin_sha256'},
                'input provenance declaration differs')
        for value in declaration['provenance'].values():
            require(type(value) is str and len(value) == 64 and all(c in '0123456789abcdef' for c in value), 'invalid provenance digest')
        result = IntegrationInputs('integrated-observation', root, local_file(root, roles['entry']),
            local_file(root, roles['config']), tuple(sorted(local_file(root, name) for name in modules)),
            local_file(root, roles['setup']), local_file(root, roles['host']), local_file(root, roles['golden']),
            ('direct', 'search'), tuple(sorted(files, key=lambda row: row.path)), selected, False)
        golden = json.loads(result.golden.read_text())
        require(type(golden) is list and len(golden) == 108 and golden[-1] == ['edit', 11],
                'companion fixed observation contract differs')
    required = set(result.modules) | {result.entry, result.config, result.golden}
    required.update(path for path in (result.setup, result.host) if path is not None)
    require(required <= {item.path for item in result.files}, 'input role missing from complete declared files')
    validate_inputs(result, mode)
    return result


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def execute(command, directory, label):
    started = time.monotonic_ns()
    try:
        run = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError as error:
        # Keep a failed invocation and its diagnostic just like a child failure.
        run = subprocess.CompletedProcess(command, 127, b'', str(error).encode())
    (directory / (label + '.stdout')).write_bytes(run.stdout)
    (directory / (label + '.stderr')).write_bytes(run.stderr)
    record = {'command': command, 'exit_code': run.returncode,
              'elapsed_ns': time.monotonic_ns() - started,
              'stdout_sha256': hashlib.sha256(run.stdout).hexdigest(),
              'stderr_sha256': hashlib.sha256(run.stderr).hexdigest()}
    # A later malformed report must not erase an already executed command.
    write(directory / (label + '.command.json'), record)
    return record, run


def wanted_value(label, mode, edits, retain_at):
    if label.startswith('retained-branch'):
        return 12 if retain_at % 2 else 11
    if label == 'final' and mode in ('retain', 'advance'):
        return 12 if edits % 2 else 11
    return 11


def qualify(folder, fixture, codec, mode, edits, retain_at, *, level=None,
            work=200_000_000, memory=256_000_000, native_compiler=None, inputs=None):
    inputs = inputs or resolve_inputs(fixture, mode)
    require(inputs.root == Path(fixture).resolve(), 'resolved input root differs from requested fixture')
    validate_inputs(inputs, mode)
    report = json.loads((folder / 'report.json').read_text())
    checks = []
    issues = []
    require(report['ledger_after_finish']['retained_bytes'] == 0, 'retained compiler bytes leaked')
    require(report['mode'].lower() == mode, 'reported mode differs from requested mode')
    expected_entry = inputs.entry
    require(Path(report['entry']).resolve() == expected_entry.resolve(), 'entry changed')
    require(Path(report['config']['path']).resolve() == inputs.config.resolve(), 'config path changed')
    require(report['config']['sha256'] == sha(inputs.config), 'config bytes changed')
    require(report['config']['level_override'] == level, 'level override changed')
    require(report['retain_at'] == retain_at, 'retention boundary changed')
    require(report['driver_ceilings'] == {'logical_work': work, 'retained_bytes': memory, 'checkpoints': 128}, 'driver ceilings changed')
    profile = tomllib.loads(inputs.config.read_text())['javascript']
    expected_effort = 0 if mode == 'native' else level if level is not None else profile['optimization_level']
    require(report['policy']['effort'] == expected_effort, 'resolved effort differs from requested effort')
    contract = report['policy']['contract']
    require(contract['target'] == ('native' if mode == 'native' else 'javascript'), 'resolved target changed')
    if mode != 'native':
        require(contract['execution'] == 'Module' and contract['preserve_root_exports'] is True, 'original module execution contract changed')
        require(contract['ecmascript'] == profile['ecmascript'], 'target edition changed')
        require(contract['strip_console'] == profile['strip_console'], 'print effect contract changed')
        require(contract['pristine_builtins'] == profile['assume_pristine_builtins'], 'ambient intrinsic contract changed')
        require(report['policy']['objective']['codec'].lower() == profile['cost_model'], 'selected codec changed')
        require(report['policy']['objective']['priority'] == 'SizeFirst', 'size-first eligibility contract changed')
    require(nonnegative_integer(report['cold_first_complete_ns']), 'missing measured cold first artifact time')
    labels = [item['label'] for item in report['artifacts']]
    required = {'baseline'} if mode == 'direct' else {
        'winner-Raw', 'winner-Gzip', 'winner-Brotli'} if mode == 'search' else set()
    if mode in ('retain', 'advance'):
        required = {'baseline', 'final', 'baseline-recheck'}
        if retain_at is not None:
            required |= {'retained-branch', 'retained-branch-recheck'}
        require(len(report['edits']) == edits, 'edit count changed')
        for number, edit in enumerate(report['edits'], 1):
            require(edit['value'] == (12 if number % 2 else 11), 'edit values do not follow the fixed sequence')
            copied = mode == 'retain' or number == 1 or (retain_at is not None and number == retain_at + 1)
            require(type(edit['copied_units']) is int and type(edit['reused_units']) is int
                    and (edit['copied_units'], edit['reused_units']) == (int(copied), int(not copied)),
                    'edit payload ownership differs from this fixed retained/advance fixture')
            require(nonnegative_integer(edit['copied_payload_bytes'])
                    and (edit['copied_payload_bytes'] > 0) == copied,
                    'copied payload bytes must be positive exactly when a payload is copied')
            require(nonnegative_integer(edit['edit_ns']) and nonnegative_integer(edit.get('release_ns', 0)), 'invalid measured edit/release duration')
    else:
        require(not report['edits'], 'unexpected source edits')
    require(len(set(labels)) == len(labels) and set(labels) == required, 'required artifact set changed')
    require((report['native'] is not None) == (mode == 'native'), 'native artifact set changed')
    expected_sources = set(inputs.modules)
    actual_sources = [Path(source['path']).resolve() for source in report['sources']]
    require(len(actual_sources) == len(set(actual_sources)) and set(actual_sources) == expected_sources, 'required original module set changed')
    require(report['source_shape']['modules'] == len(expected_sources), 'reported module count changed')
    for source in report['sources']:
        require(sha(source['path']) == source['sha256'] and Path(source['path']).stat().st_size == source['bytes'], 'discovered input changed')
    if report['native']:
        require(report['native']['file'] == 'native.c', 'unexpected native artifact path')
        source = folder / report['native']['file']
        require(sha(source) == report['native']['sha256'] and source.stat().st_size == report['native']['bytes'], 'native C bytes changed')
        require(report['ledger_before_finish']['codec_work'] == 0, 'native request performed codec work')
        compiler = native_compiler or binary_provenance('/usr/bin/cc')
        require(binary_provenance(compiler['path']) == compiler, 'native compiler binary changed')
        version_check, version = execute([compiler['path'], '--version'], folder, 'native-compiler-version')
        checks.append(version_check)
        require(version.returncode == 0 and bool(version.stdout), 'native compiler version unavailable')
        compiled, result = execute([compiler['path'], *NATIVE_FLAGS, str(source),
                                    '-lm', '-o', str(folder / 'native')], folder, 'native-compile')
        compiled['compiler'] = {**compiler, 'version': version.stdout.decode(errors='replace')}
        compiled['qualification'] = 'One C O2 driver-artifact execution with canonical FP flags; the separate library gate supplies both compilers/O0/O2/UBSan.'
        checks.append(compiled)
        if result.returncode:
            issues.append('native C compile failed')
        else:
            executable = folder / 'native'
            executable_sha256 = sha(executable)
            compiled['executable_sha256'] = executable_sha256
            compiled['executable_bytes'] = executable.stat().st_size
            observed, result = execute([str(folder / 'native')], folder, 'native-execute')
            observed['executable_sha256'] = executable_sha256
            checks.append(observed)
            if result.returncode or result.stderr or result.stdout != inputs.golden.read_bytes():
                issues.append('native execution differs from fixed golden')
            require(sha(executable) == executable_sha256, 'native executable changed during execution')
        require(sha(source) == report['native']['sha256'], 'compiled native source changed')
        require(binary_provenance(compiler['path']) == compiler, 'native compiler binary changed')
        validate_inputs(inputs, mode)
        return checks, issues
    setup = inputs.setup.read_text()
    host = inputs.host.read_text()
    expected = json.loads(inputs.golden.read_text())
    require(expected[-1] == ['edit', 11], 'fixed edit oracle changed')
    # Source files remain separate; the test host loads the measured .mjs path.
    script = "import fs from 'node:fs';import{pathToFileURL}from'node:url';\nconst events=[];\n" + setup
    script += "\nconst library=await import(pathToFileURL(process.argv[1]).href);\n" + host
    script += '\nprocess.stdout.write(JSON.stringify(events));\n'
    (folder / 'observe.mjs').write_text(script)
    paths = []
    for artifact in report['artifacts']:
        require(artifact['file'] == artifact['label'] + '.mjs', 'unexpected JavaScript artifact path')
        path = folder / artifact['file']
        require(sha(path) == artifact['sha256'] and path.stat().st_size == artifact['bytes'], 'JavaScript bytes changed')
        require(isinstance(artifact['details']['sizes'], list) and len(artifact['details']['sizes']) == 3
                and all(nonnegative_integer(value) for value in artifact['details']['sizes']), 'all three complete canonical scores are required')
        require(artifact['details']['sizes'][0] == artifact['bytes'], 'raw score differs from artifact bytes')
        paths.append(str(path))
        check, run = execute(['node', '--input-type=module', '-e', script, str(path)], folder,
                             'execute-' + artifact['label'])
        checks.append(check)
        wanted = list(expected)
        value = wanted_value(artifact['label'], mode, edits, retain_at)
        require(artifact['expected_edit_value'] == value, 'artifact edit expectation differs from the fixed edit sequence')
        wanted[-1] = ['edit', value]
        write(folder / ('expected-' + artifact['label'] + '.json'), wanted)
        try:
            good = run.returncode == 0 and not run.stderr and json_equal(json.loads(run.stdout), wanted)
        except (ValueError, UnicodeDecodeError):
            good = False
        if not good:
            issues.append('execution failed: ' + artifact['label'])
        require(sha(path) == artifact['sha256'], 'executed artifact changed')
    check, run = execute([str(codec), '--json', *paths], folder, 'canonical-codecs')
    checks.append(check)
    if run.returncode or run.stderr:
        issues.append('canonical codec verification failed')
    else:
        codec_report = json.loads(run.stdout)
        require(codec_report['schemaVersion'] == 1, 'unknown canonical codec schema')
        require(codec_report['codecs']['gzip9']['level'] == 9 and codec_report['codecs']['gzip9']['mtime'] == 0, 'gzip parameters changed')
        require(codec_report['codecs']['brotli11']['quality'] == 11 and codec_report['codecs']['brotli11']['lgwin'] == 22 and codec_report['codecs']['brotli11']['mode'] == 'generic', 'Brotli parameters changed')
        measured = codec_report['artifacts']
        require(len(measured) == len(report['artifacts']), 'canonical artifact count changed')
        for original, current in zip(report['artifacts'], measured):
            path = folder / original['file']
            require(Path(current['path']).resolve() == path.resolve(), 'canonical score belongs to a different path')
            scores = [current['raw'], current['gzip9'], current['brotli11']]
            require(all(nonnegative_integer(value) for value in scores), 'invalid canonical score')
            expected_scores = original['details']['sizes']
            if expected_scores != scores:
                issues.append('canonical score mismatch: ' + original['label'])
            require(sha(path) == original['sha256'], 'measured artifact changed')
    validate_inputs(inputs, mode)
    return checks, issues


def trial(args, root, mode, ordinal, native_compiler=None, inputs=None):
    folder = root / f'{ordinal:02}-{mode}'
    metrics = root / f'{ordinal:02}-{mode}.time'
    inputs = inputs or resolve_inputs(args.fixture, mode, getattr(args, 'input_manifest', None))
    validate_inputs(inputs, mode)
    command = [str(args.driver), str(inputs.entry), '--config', str(inputs.config),
               '--output', str(folder), '--mode', mode, '--edits', str(args.edits),
               '--work', str(args.work), '--memory', str(args.memory)]
    if args.level is not None:
        command += ['--level', str(args.level)]
    if args.retain_at is not None:
        command += ['--retain-at', str(args.retain_at)]
    timed = ['/usr/bin/time', '-o', str(metrics), '-f',
             'wall_s=%e\nuser_s=%U\nsystem_s=%S\npeak_rss_kib=%M\nexit=%x', *command]
    record, run = execute(timed, root, f'{ordinal:02}-{mode}-driver')
    record.update(mode=mode, ordinal=ordinal, directory=str(folder), qualified=False, issues=[])
    if metrics.exists():
        try:
            record['process'] = {key: float(value) for line in metrics.read_text().splitlines()
                                 if '=' in line for key, value in [line.split('=', 1)]}
        except ValueError as error:
            record['issues'].append('invalid process metrics: ' + str(error))
    if run.returncode:
        record['issues'].append('driver failed')
    else:
        try:
            process = record.get('process', {})
            require(set(process) == {'wall_s', 'user_s', 'system_s', 'peak_rss_kib', 'exit'}
                    and all(math.isfinite(value) and value >= 0 for value in process.values())
                    and process['exit'] == 0 and process['peak_rss_kib'] > 0,
                    'complete successful process timing/RSS metrics required')
            checks, issues = qualify(folder, args.fixture, args.codec, mode, args.edits, args.retain_at,
                                     level=args.level, work=args.work, memory=args.memory,
                                     native_compiler=native_compiler, inputs=inputs)
            record.update(checks=checks, report_sha256=sha(folder / 'report.json'))
            record['issues'].extend(issues)
            report = json.loads((folder / 'report.json').read_text())
            record['artifacts'] = [(item['label'], item['sha256']) for item in report['artifacts']]
            record['edit_ns'] = sum(item['edit_ns'] + item.get('release_ns', 0) for item in report['edits'][1:])
            record['copied_payload_bytes'] = sum(item['copied_payload_bytes'] for item in report['edits'])
            record['reused_units'] = sum(item['reused_units'] for item in report['edits'])
            record['post_fork_copied_payload_bytes'] = sum(item['copied_payload_bytes'] for item in report['edits'][1:])
            record['post_fork_reused_units'] = sum(item['reused_units'] for item in report['edits'][1:])
            record['counter_scope'] = {'copied_payload_bytes': 'all edits including the common first fork',
                                       'reused_units': 'all edits including the common first fork',
                                       'post_fork': 'edits after the common first fork; same interval as edit_ns'}
            record['qualified'] = not record['issues']
        except (AssertionError, KeyError, OSError, ValueError, TypeError, IndexError) as error:
            record['issues'].append(type(error).__name__ + ': ' + str(error))
    write(root / f'{ordinal:02}-{mode}.json', record)
    print(json.dumps({key: record[key] for key in ('mode', 'ordinal', 'qualified', 'issues')}), flush=True)
    return record


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--driver', type=Path, required=True)
    parser.add_argument('--codec', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--input-manifest', type=Path, help='Explicit supported companion input contract; default original/scaling route is unchanged')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--mode', choices=['direct', 'search', 'retain', 'advance', 'native', 'paired'], default='paired')
    parser.add_argument('--pairs', type=int, default=7)
    parser.add_argument('--level', type=int)
    parser.add_argument('--edits', type=int, default=32)
    parser.add_argument('--retain-at', type=int)
    parser.add_argument('--work', type=int, default=200_000_000)
    parser.add_argument('--memory', type=int, default=256_000_000)
    args = parser.parse_args()
    for name in ('driver', 'codec', 'fixture', 'output'):
        setattr(args, name, getattr(args, name).resolve())
    if args.pairs < 1 or args.pairs > 100 or args.edits < 1 or args.edits > 100_000:
        parser.error('bounded pair/edit counts required')
    if args.mode == 'paired' and args.edits < 2:
        parser.error('paired comparison needs at least two edits after the common initial source')
    if args.retain_at is not None and not 1 <= args.retain_at < args.edits:
        parser.error('retain-at must name a nonfinal edit')
    if args.level is not None and not 0 <= args.level <= 16:
        parser.error('level must be between 0 and 16')
    if args.input_manifest is not None:
        args.input_manifest = args.input_manifest.resolve(strict=True)
    selected_mode = 'retain' if args.mode == 'paired' else args.mode
    inputs = resolve_inputs(args.fixture, selected_mode, args.input_manifest)
    args.output.mkdir(parents=True, exist_ok=False)
    # Freeze this trial's declared controls before observing either arm.
    native_compiler = binary_provenance('/usr/bin/cc') if args.mode == 'native' else None
    contract = {'schema': 1, 'arguments': {key: str(value) if isinstance(value, Path) else value
                for key, value in vars(args).items()}, 'driver_sha256': sha(args.driver),
                'codec_sha256': sha(args.codec), 'runner_sha256': sha(__file__),
                'required_modules': sorted(str(path.relative_to(inputs.root)) for path in inputs.modules),
                'input_contract': inputs.describe(),
                'native_compiler': native_compiler, 'native_flags': NATIVE_FLAGS if native_compiler else None,
                'fixture': {str(item.path.relative_to(inputs.root)): item.sha256 for item in inputs.files},
                'method': 'Alternating retained/advance order; all trials kept. Compiler build/pin and worker provenance are supplied by the enclosing receipt. No general speed or competitor claim.'}
    write(args.output / 'contract.json', contract)
    runs, pairs, issues = [], [], []
    for ordinal in range(args.pairs if args.mode == 'paired' else 1):
        modes = (['retain', 'advance'] if ordinal % 2 == 0 else ['advance', 'retain']) if args.mode == 'paired' else [args.mode]
        current = {}
        for mode in modes:
            record = trial(args, args.output, mode, ordinal, native_compiler, inputs=inputs)
            current[mode] = record
            runs.append(record)
            issues.extend(record['issues'])
        if args.mode == 'paired' and all(record['qualified'] for record in current.values()):
            a, b = current['retain'], current['advance']
            if a['artifacts'] != b['artifacts']:
                issues.append(f'pair {ordinal}: actual artifact bytes differ')
                pairs.append({'ordinal': ordinal, 'qualified': False, 'reason': 'unequal artifacts'})
            elif a['edit_ns'] <= 0 or b['edit_ns'] <= 0:
                issues.append(f'pair {ordinal}: no positive measured post-fork edit duration')
                pairs.append({'ordinal': ordinal, 'qualified': False, 'reason': 'missing positive duration'})
            else:
                pairs.append({'ordinal': ordinal, 'qualified': True,
                              'advance_over_retain_edit_time': b['edit_ns'] / a['edit_ns'],
                              'advance_over_retain_peak_rss': b['process']['peak_rss_kib'] / a['process']['peak_rss_kib']})
    try:
        unchanged = contract['driver_sha256'] == sha(args.driver) and contract['codec_sha256'] == sha(args.codec)
        unchanged &= contract['runner_sha256'] == sha(__file__)
        if native_compiler:
            unchanged &= native_compiler == binary_provenance(native_compiler['path'])
        validate_inputs(inputs, selected_mode)
    except (OSError, ValueError) as error:
        unchanged = False
        issues.append('frozen input unavailable: ' + str(error))
    if not unchanged:
        issues.append('frozen input changed during trial')
    summary = {'complete': True, 'qualified': not issues, 'runs': runs, 'pairs': pairs, 'issues': issues}
    ratios = [pair['advance_over_retain_edit_time'] for pair in pairs if pair['qualified']]
    if ratios:
        summary['edit_ratio_summary'] = {'median': statistics.median(ratios), 'min': min(ratios), 'max': max(ratios),
                                         'geometric_mean': math.exp(statistics.mean(map(math.log, ratios))),
                                         'scope': 'Observed paired samples; no confidence interval or speed gate is inferred.'}
    write(args.output / 'summary.json', summary)
    return 0 if summary['qualified'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
