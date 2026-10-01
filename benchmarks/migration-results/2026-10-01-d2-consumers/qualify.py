#!/usr/bin/env python3
"""D2 production artifacts, independent consumers and canonical file costs."""
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
BINARY = Path(sys.argv[1]).resolve()
CODEC = Path('/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec')
NODE = Path('/home/azureuser/.nvm/versions/node/v20.19.0/bin/node')
ESBUILD = Path('/home/azureuser/motionlil/node_modules/esbuild/bin/esbuild')
WORK = Path('/home/azureuser/lilscript-work/d2-consumers-qualification')
ENV = {k: v for k, v in os.environ.items()
       if not k.startswith('LILSCRIPT_') and k != 'NODE_OPTIONS'}
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
fixtures = HERE / 'fixtures'
fixtures.mkdir(exist_ok=True)
(fixtures / 'data.lil').write_text('export struct Point{int x;}\nexport const Point[] values=[Point{7}];\nexport const Record<Point[]> table=record{left:values,right:values};\n')
(fixtures / 'feature.lil').write_text('extern void note(string message);\nnote("feature");\nexport int read(){return 19;}\n')
(fixtures / 'main.lil').write_text('import {values,table} from "./data";\nexport {values,table};\nexport async int load(){auto m=await import("./feature");return m.read();}\n')
report = {
    'base': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip(),
    'algorithm': 67, 'search': 33,
    'compiler': {'path': str(BINARY), 'sha256': sha(BINARY)},
    'codec': {'path': str(CODEC), 'sha256': sha(CODEC)},
    'node': {'path': str(NODE), 'sha256': sha(NODE)},
    'consumer_bundler': {'path': str(ESBUILD), 'sha256': sha(ESBUILD),
                         'version': subprocess.check_output([ESBUILD, '--version'], text=True).strip()},
    'fixtures': {p.name: sha(p) for p in sorted(fixtures.glob('*.lil'))},
    'rows': [], 'complete': False,
}
previous_path = HERE / 'artifacts.json'
if previous_path.exists():
    previous = json.loads(previous_path.read_text())
    if all(previous.get(key) == report[key] for key in ['compiler', 'fixtures', 'codec', 'node']):
        report['rows'] = [row for row in previous['rows'] if row.get('oracle')]


def save():
    (HERE / 'artifacts.json').write_text(json.dumps(report, indent=2) + '\n')


def run(args):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    result = subprocess.run([str(a) for a in args], cwd=REPO, env=ENV,
                            text=True, capture_output=True, timeout=300)
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    return result, {'wall_seconds': time.monotonic() - start,
                    'cpu_seconds': after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime}


for maps in ['off', 'external']:
    for objective in ['raw', 'gzip', 'brotli']:
        if any(row['maps'] == maps and row['objective'] == objective for row in report['rows']):
            continue
        folder = WORK / maps / objective
        folder.mkdir(parents=True, exist_ok=True)
        config = folder / 'config.toml'
        config.write_text(f"objective.codecs='{objective}'\neffort.level=13\n[policy]\nversion=3\n[target.javascript]\nformat='esm'\n[delivery]\nmode='single'\nentry_names='main.mjs'\nannotations='all'\nsource_maps='{maps}'\n")
        result, cost = run([BINARY, fixtures / 'main.lil', '--config', config,
                            '--target', 'js-module', '--out-dir', folder, '--explain', 'json'])
        (folder / 'explain.json').write_text(result.stderr)
        row = {'maps': maps, 'objective': objective, 'config': config.read_text(),
               'config_sha256': sha(config), 'exit_code': result.returncode, **cost}
        report['rows'].append(row)
        save()
        assert result.returncode == 0, result.stderr[-4000:]
        explanation = json.JSONDecoder().raw_decode(result.stderr[result.stderr.index('{'):])[0]
        manifest = json.loads((folder / 'lilscript.manifest.json').read_text())
        output = manifest['outputs'][0]
        entry = folder / output['entries'][0]['file']
        oracle = f"""
import {{readFileSync}} from 'node:fs';
import {{SourceMap}} from 'node:module';
let events=[];globalThis.note=value=>events.push(value);
const m=await import({json.dumps(entry.as_uri())});
if(events.length)throw Error('eager lazy initializer');
if(m.values!==m.table.left||m.table.left!==m.table.right||m.values[0].x!==7)throw Error('graph');
for(const value of [m.values,m.values[0],m.table])if(!Object.isFrozen(value))throw Error('freeze');
if(Object.getPrototypeOf(m.table)!==null)throw Error('record prototype');
if(await m.load()!==19||await m.load()!==19||events.join()!=='feature')throw Error('once-only loading');
const code=readFileSync({json.dumps(str(entry))},'utf8');
if({json.dumps(maps)}==='external'){{
  const url=code.match(/sourceMappingURL=([^\\n]+)/)[1];
  const data=JSON.parse(readFileSync(new URL(url,{json.dumps(entry.as_uri())}),'utf8'));
  if(data.file!=={json.dumps(entry.name)}||data.version!==3)throw Error('map metadata');
  const offset=code.indexOf('note('),lines=code.slice(0,offset).split('\\n');
  const at=new SourceMap(data).findEntry(lines.length-1,lines.at(-1).length);
  if(!at.originalSource.endsWith('feature.lil')||at.originalLine!==1||at.originalColumn!==0)throw Error(JSON.stringify(at));
}}
console.log('ok');
"""
        probe, _ = run([NODE, '--input-type=module', '-e', oracle])
        assert probe.returncode == 0 and probe.stdout == 'ok\n', (probe.stderr, probe.stdout)
        metric = {'raw': 'raw', 'gzip': 'gzip9', 'brotli': 'brotli11'}[objective]
        files = []
        for listed in output['files']:
            path = folder / listed['file']
            assert path.stat().st_size == listed['bytes'] and sha(path) == listed['sha256']
            measurement = json.loads(subprocess.check_output([CODEC, '--json', path], env=ENV, text=True))['artifacts'][0]
            assert measurement[metric] == listed['codec_bytes']
            files.append({'name': listed['file'], 'sha256': sha(path),
                          **{key: measurement[key] for key in ['raw', 'gzip9', 'brotli11']}})
        stage = explanation['search']['terminal']['objectives'][0]
        assert stage['after'] == output['rows_total']
        assert output['codec_total'] == sum(file[metric] for file in files)
        row.update(files=files, bytes=stage['after'], judged=stage['judged'], oracle=True,
                   manifest_sha256=sha(folder / 'lilscript.manifest.json'),
                   explanation_sha256=sha(folder / 'explain.json'))
        save()
        print(maps, objective, row['bytes'], round(row['cpu_seconds'], 3), flush=True)
report['complete'] = True
save()
