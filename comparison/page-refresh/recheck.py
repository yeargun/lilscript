#!/usr/bin/env python3
"""Repair test prerequisites and rerun gates against preserved artifact sets."""
import hashlib
import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import time

ROOT=pathlib.Path(sys.argv[1]).resolve()
OUT=ROOT/'.page-audit'
TOOLS=pathlib.Path(__file__).resolve().parent
config=json.loads((ROOT/'page-audit-config.json').read_text())
result=json.loads((OUT/'result.json').read_text())
(OUT/'initial-result.json').write_text(json.dumps(result,indent=2)+'\n')
env={**os.environ,'LILSCRIPT_COMPILER':str(TOOLS/'compiler-wrapper.py'),
     'LILSCRIPT_CODEC':str(TOOLS/'lilscript-codec'),'LILSCRIPT_ROOT':str(ROOT.parent/'lilscript'),
     'PAGE_AUDIT_REAL_COMPILER':str(TOOLS/'lilscript'),'PAGE_AUDIT_INVOCATIONS':str(OUT/'recheck-invocations.jsonl'),
     'RAYON_NUM_THREADS':'16','LILSCRIPT_TIMING':'1'}


def run(label,command,timeout=1200):
    started=time.monotonic();log=OUT/(label+'.log')
    with log.open('w') as output:
        child=subprocess.Popen(['bash','-c',command],cwd=ROOT,env=env,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
        try:code=child.wait(timeout)
        except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);code=child.wait()
    text=log.read_text(errors='replace')
    import re
    record={'command':['bash','-c',command],'exitCode':code,'wallSeconds':time.monotonic()-started,
            'measuredAt':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),
            'tail':text[-5000:],'failures':sorted(set(re.findall(r'^\s*not ok \d+\s*-?\s*(.+)$',text,re.M))),
            'logSha256':hashlib.sha256(log.read_bytes()).hexdigest()}
    result[label]=record
    (OUT/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(config['name'],label,code,flush=True)
    return code==0


def update_site_sizes():
    site=ROOT/'site/results.json'
    if not site.exists() or not config.get('file'):return
    data=json.loads(site.read_text())
    paths={key:ROOT/'dist'/f"{config['file']}.{suffix}" for key,suffix in [('itslil','esm.js'),('itslil-closed','closed.js')]}
    paths={key:path for key,path in paths.items() if path.exists()}
    if not paths:return
    values=json.loads(subprocess.check_output([str(TOOLS/'lilscript-codec'),'--json',*[str(path) for path in paths.values()]],text=True))['artifacts']
    measured=dict(zip(paths,values))
    for item in data.get('size',[]):
        if item.get('id') in measured:item.update({key:measured[item['id']][key] for key in ['raw','gzip9','brotli11']})
    site.write_text(json.dumps(data,indent=2)+'\n')


name=config['name']
if name in ['mdast-util-from-markdownlil','remark-parselil']:
    package=config['upstreamPackage']+'@'+config['upstreamVersion']
    run('repair-test-dependency','npm install --save-dev --save-exact '+package+' --no-audit --no-fund',600)
    result['dependencyRepair']='Added the pinned official reference package as an explicit devDependency; tests imported it but npm ci did not install it.'
if name=='react-markdownlil':
    run('repair-test-dependency','npm install --save-dev --save-exact @itslil/remark-gfm@4.0.2 @itslil/rehype-katex@7.0.2 --no-audit --no-fund',600)
    result['dependencyRepair']='Added the explicit LilScript plugin fixtures imported by the official/parity tests.'
if name=='mobxlil':
    path=ROOT/'jest.config.cjs'
    source=path.read_text()
    entry='        "^\\\\.\\\\./\\\\.\\\\./dist/mobx\\\\.cjs\\\\.production\\\\.min\\\\.js$": "<rootDir>/dist/mobx.cjs.production.min.js",\n'
    if entry not in source:source=source.replace('    moduleNameMapper: {\n','    moduleNameMapper: {\n'+entry)
    path.write_text(source)
    result['testHarnessRepair']='Map the upstream tests\' nested production CJS import to this repository\'s rebuilt production distribution.'

if '--rebuild' in sys.argv:
    if (OUT/'invocations.jsonl').exists():(OUT/'invocations.jsonl').unlink()
    env['PAGE_AUDIT_INVOCATIONS']=str(OUT/'invocations.jsonl')
    if config.get('setup'):run('setup-retry',config['setup'],1800)
    for path in (ROOT/'dist').rglob('*') if (ROOT/'dist').exists() else []:
        if path.is_file() and path.suffix in ['.js','.mjs','.cjs','.map']:path.unlink()
    ok=run('build',config['buildCommand'],5400)
    inv=OUT/'invocations.jsonl';result['invocations']=[json.loads(line) for line in inv.read_text().splitlines()] if inv.exists() else []
    result['build']['fresh']=ok and bool(result['invocations'])
    result['build']['compilerWasInvoked']=bool(result['invocations'])
    if not ok:
        (OUT/'result.json').write_text(json.dumps(result,indent=2)+'\n');sys.exit(1)
    artifacts=[path for path in (ROOT/'dist').rglob('*') if path.is_file() and path.suffix in ['.js','.mjs','.cjs']]
    if name=='vuelil':artifacts += list((ROOT/'artifacts').glob('*.generated.js'))
    artifacts+=list((OUT/'upstream').glob('*.js'))
    measured=json.loads(subprocess.check_output([str(TOOLS/'lilscript-codec'),'--json',*[str(path) for path in artifacts]],text=True))
    result['artifacts']=[{**item,'path':str(pathlib.Path(item['path']).relative_to(ROOT)),'sha256':hashlib.sha256(pathlib.Path(item['path']).read_bytes()).hexdigest()} for item in measured['artifacts']]

if (ROOT/'dist').exists():shutil.copytree(ROOT/'dist',OUT/'fresh-dist',dirs_exist_ok=True)
if (OUT/'before-dist').exists() and '--skip-before' not in sys.argv:
    shutil.rmtree(ROOT/'dist',ignore_errors=True);shutil.copytree(OUT/'before-dist',ROOT/'dist')
    shutil.rmtree(ROOT/'_site',ignore_errors=True)
    update_site_sizes()
    run('before-tests-repaired',config['testCommand'])
    result['before-tests']=result['before-tests-repaired']
    shutil.rmtree(ROOT/'dist',ignore_errors=True);shutil.copytree(OUT/'fresh-dist',ROOT/'dist')
shutil.rmtree(ROOT/'_site',ignore_errors=True)
update_site_sizes()
run('after-tests',config['testCommand'])
if name=='mobxlil':run('production-browser-tests','node e2e/run.mjs',180)
result['artifactsChangedDuringTests']=[item['path'] for item in result.get('artifacts',[]) if not (ROOT/item['path']).exists() or hashlib.sha256((ROOT/item['path']).read_bytes()).hexdigest()!=item['sha256']]
(OUT/'result.json').write_text(json.dumps(result,indent=2)+'\n')
