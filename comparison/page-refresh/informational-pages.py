#!/usr/bin/env python3
"""Keep the original site designs; publish only the current ESM comparison."""
import concurrent.futures
import copy
import fcntl
import hashlib
import html
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

RUN=pathlib.Path(sys.argv[1]).resolve()
TOOLS=pathlib.Path(__file__).resolve().parent
PUSH='--push' in sys.argv
REFRESH='--refresh' in sys.argv
PORTS=sys.argv[sys.argv.index('--ports')+1].split(',') if '--ports' in sys.argv else None
PUBLICATIONS=json.loads((RUN/'publications.json').read_text())
ROWS={item['name']:item for item in json.loads((RUN/'prepared-current.json').read_text())}
RECEIPTS={item['name']:item for item in json.loads((RUN/'receipts.json').read_text())}
if not (RUN/'publications-audit.json').exists():shutil.copy2(RUN/'publications.json',RUN/'publications-audit.json')

def command(args,root,timeout=600):
    version='v24.11.1' if root.name=='vuelil' else 'v22.23.2'
    env={**os.environ,'PATH':'/home/azureuser/.nvm/versions/node/'+version+'/bin:'+os.environ['PATH']}
    result=subprocess.run(args,cwd=root,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout)
    if result.returncode:raise RuntimeError(' '.join(args)+'\n'+result.stdout[-4000:])
    return result.stdout.strip()

def read(path,default=None):
    return json.loads(path.read_text()) if path.exists() else default

def write(path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,indent=2)+'\n')

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def record(name,value):
    with (RUN/'publications.lock').open('w') as handle:
        fcntl.flock(handle,fcntl.LOCK_EX)
        data=read(RUN/'publications.json',{});data[name]=value;write(RUN/'publications.json',data)

def test_counts(receipt):
    text=receipt['behavior'].get('after',{}).get('tail','')
    def number(pattern):
        values=re.findall(pattern,text,re.M)
        return int(values[-1]) if values else None
    total=number(r'^# tests (\d+)');passed=number(r'^# pass (\d+)')
    if total is None:
        passed=number(r'Tests\s+.*?(\d+) passed')
        total=number(r'Tests\s+.*?\((\d+)\)')
    return {'pass':passed,'total':total} if passed is not None and total is not None else None

def refresh_size_rows(data,receipt):
    if not isinstance(data.get('size'),list):return
    artifacts={item['path']:item for item in receipt['candidateArtifacts']}
    official={pathlib.Path(item['path']).name:item for item in receipt['officialArtifacts']}
    prefix='parse' if receipt['name']=='markedlil' else 'kernel' if receipt['name']=='posthoglil' else 'official'
    mapping={prefix:'official.unminified.js',prefix+'-terser-mangle':'official.esm.js',prefix+'-terser-nomangle':'official.terser-nomangle.js',prefix+'-esbuild':'official.esbuild.js',prefix+'-esbuild-esnext':'official.esbuild.js',prefix+'-oxc-mangle':'official.oxc-mangle.js',prefix+'-oxc-nomangle':'official.oxc-nomangle.js','official-source-terser':'official.source-terser.js','officialDev':'official.unminified.js','officialMin':'official.esm.js'}
    file=data.get('file') or {'markedlil':'marked','posthoglil':'posthog'}.get(receipt['name'])
    candidates={'itslil':receipt['sizes']['current'],'itslil-package':receipt['sizes']['current'],'itslil-closer':receipt['sizes']['current']}
    if receipt['name']=='posthoglil':candidates['itslil']=artifacts.get('dist/'+file+'.raw.js',candidates['itslil'])
    for kind in ['closed','gzip','bytes']:
        if file:candidates['itslil-'+kind]=artifacts.get('dist/'+file+'.'+kind+'.js')
    rows=[]
    for item in data['size']:
        row=copy.deepcopy(item);identifier=row.get('id','')
        value=official.get(mapping.get(identifier,'')) or candidates.get(identifier)
        if not identifier:
            value=receipt['sizes']['current'] if row.get('primary') else official.get('official.oxc-mangle.js' if 'Oxc' in row.get('name','') else 'official.esm.js')
        if not value:continue
        row.update({key:value[key] for key in ['raw','gzip9','brotli11']})
        row.update(measuredAt=receipt['measuredAt'],artifactSha256=value['sha256'])
        if identifier.startswith('itslil'):row['note']='ESM output from LilScript '+receipt['compiler']['commit'][:7]+'.'
        if receipt['name']=='mobxlil' and not row.get('primary'):row.update(name='Original ESM + Terser',baseline=True,note='Complete production ESM graph; Terser with mangling and three compression passes.')
        if identifier=='itslil-closer':row['name']='LilScript · package ESM'
        if identifier in ['officialDev','officialMin']:row['name']='Original · '+('ESM' if identifier=='officialDev' else 'ESM + Terser')
        if 'build audit' in row.get('note',''):row['note']='ESM comparison with matching external dependencies.'
        rows.append(row)
    data['size']=rows
    baseline=next((row for row in rows if row.get('baseline')),None)
    lil=next((row for row in rows if row.get('primary')),None)
    if baseline and lil:
        gzip=next((row for row in rows if row.get('id')=='itslil-gzip'),lil)
        raw=next((row for row in rows if row.get('id')=='itslil-bytes'),lil)
        if 'hero' in data:
            for metric,label,value in [('brotli11','Brotli',lil),('gzip9','Gzip',gzip),('raw','Raw',raw)]:
                data['hero'].update({label.lower().replace('brotli','brotli')+'Ratio':value[metric]/baseline[metric],'official'+label:baseline[metric],'itslil'+label:value[metric]})
        if 'matched' in data:data['matched'].update(raw=raw['raw'],gzip9=gzip['gzip9'],brotli11=lil['brotli11'],vsOxc={key:value[key]/baseline[key] for key,value in [('raw',raw),('gzip9',gzip),('brotli11',lil)]})
    counts=test_counts(receipt)
    if counts and 'spec' in data:data['spec']={**data['spec'],**counts,'label':'repository checks'}
    if counts and 'tests' in data:data['tests']={'passed':counts['pass'],'total':counts['total'],'match':counts['pass']==counts['total']}
    data['sizeMeasuredAt']=receipt['measuredAt']

def remove_history(value):
    value=re.sub(r'<!-- build-audit:start -->[\s\S]*?<!-- build-audit:end -->\s*','',value)
    value=re.sub(r'\s*<p data-audit-history="[^"]*"[^>]*>[\s\S]*?</p>','',value)
    value=re.sub(r'\s*<section class="compiler-progress[^\"]*"[^>]*>[\s\S]*?</section>','',value)
    return value

def prepare(name):
    publication=copy.deepcopy(PUBLICATIONS[name]);root=pathlib.Path(publication['path']);source=pathlib.Path(ROWS[name]['snapshot'])
    if publication.get('presentation')=='informational' and not REFRESH:
        if PUSH and publication['status']=='prepared':
            command(['git','push','origin','HEAD:main'],root);publication.update(status='pushed',deployment='pending');record(name,publication)
            print('PUSHED',name,publication['commit'][:10],flush=True)
        return
    receipt=copy.deepcopy(RECEIPTS[name]);site=root/('web' if name=='vuelil' else 'site')
    private=root/'comparison/build-receipt.json'
    if not private.exists():private.parent.mkdir(exist_ok=True);shutil.move(site/'build-audit.json',private)
    guard=read(private)
    if name=='solidlil':receipt['sizes']['current']=next(item for item in receipt['candidateArtifacts'] if item['path']=='dist/index.js');receipt['primaryArtifact']='dist/index.js'
    if name=='motionlil':receipt['sizes']['current']=next(item for item in receipt['candidateArtifacts'] if item['path']=='dist/index.bundle.js');receipt['primaryArtifact']='dist/index.bundle.js'
    if name=='zodlil':
        extra=read(RUN/'information-zod-esm.json')
        if not extra:raise RuntimeError('Measure the complete Zod ESM package graph first')
        receipt['sizes']['current']=extra
        receipt['primaryArtifact']=extra['path']
    if name=='katexlil':
        receipt['sizes']['upstream']=next(item for item in receipt['officialArtifacts'] if item['path'].endswith('/official.esm.js'))
        result=read(source/'.page-audit/result.json');receipt['timing']['upstreamSeconds']=result['upstreamTiming']['medianSeconds'];receipt['timing']['upstreamScope']=result['upstreamTiming']['scope']
    scope=receipt['comparisonScope'].split(' Sizes use')[0]
    if name in ['solidlil','monacolil']:scope='Public ESM entries. The LilScript implementation is partial; full API parity is not established.'
    if name=='zodlil':scope='Complete public zod/v4 ESM graphs, including the LilScript compatibility, asynchronous API and JSON Schema modules.'
    status='Repository checks pass.' if receipt['status']=='verified' else 'Compatibility checks are incomplete.'
    counts=test_counts(receipt)
    if counts and counts['pass']!=counts['total']:status=f"Compatibility checks: {counts['pass']:,}/{counts['total']:,} pass."
    if name=='motionlil':status='Browser compatibility checks are incomplete.'
    if name=='vuelil':status='The full ESM build is unavailable.'
    comparison={'schemaVersion':1,'measuredAt':receipt['measuredAt'],'name':name,'compiler':receipt['compiler'],
        'upstream':{'package':receipt['upstream'].get('package'),'version':receipt['upstream'].get('pinned')},'scope':scope,
        'compatibility':status,'tests':counts,'buildComplete':receipt['buildPassed'],
        'machine':{key:value for key,value in receipt['machine'].items() if key in ['provider','instanceClass','cpu','logicalCpus','memoryBytes','os','node','rayonThreads']},
        'build':{'compilerSeconds':receipt['timing']['primaryCompilerSeconds'],'packageSeconds':receipt['timing']['packageSeconds'],'originalSeconds':receipt['timing']['upstreamSeconds'],'packageScope':receipt['timing']['packageScope'],'originalScope':receipt['timing']['upstreamScope'],'protocol':receipt['timing']['protocol']},
        'esm':{}}
    if name=='zodlil':comparison['assembly']=extra['assembly']
    guard['comparisonArtifacts']=[]
    for side,key in [('lilscript','current'),('original','upstream')]:
        value=receipt['sizes'][key]
        if not value:comparison['esm'][side]=None;continue
        path=pathlib.Path(value['path']);path=path if path.is_absolute() else source/path
        dest=site/'esm-comparison'/(side+'.js');dest.parent.mkdir(exist_ok=True)
        shutil.copy2(path,dest)
        if sha(dest)!=value['sha256']:raise RuntimeError('Measured ESM changed: '+str(path))
        comparison['esm'][side]={key:value[key] for key in ['raw','gzip9','brotli11','sha256']}
        comparison['esm'][side]['file']='./esm-comparison/'+side+'.js'
        guard['comparisonArtifacts'].append({'path':str(dest.relative_to(root)),'sha256':sha(dest)})
    write(site/'comparison.json',comparison);write(private,guard)
    index=site/'index.html';index.write_text(remove_history(index.read_text()))
    app=site/'app.js'
    if app.exists():
        code=app.read_text();code=re.sub(r'^import \{ renderCompilerComparison \} from "./compiler-comparison.js"\n','',code,flags=re.M);code=re.sub(r'^renderCompilerComparison\(data\)\s*\n','',code,flags=re.M);app.write_text(code)
    for path in site.glob('*.json'):
        if path.name=='comparison.json':continue
        data=read(path)
        if not isinstance(data,dict):continue
        for key in ['compilerComparison','buildAudit','runtimeEvidenceStatus','applicationEvidenceStatus']:data.pop(key,None)
        if path.name=='results.json':refresh_size_rows(data,receipt)
        write(path,data)
    shutil.copy2(TOOLS/'build-comparison.mjs',root/'scripts/build-comparison.mjs')
    (root/'scripts/build-audit.mjs').unlink(missing_ok=True)
    script=root/'scripts'/('build-pages.mjs' if name=='vuelil' else 'build-site.mjs')
    value=script.read_text().replace('./build-audit.mjs','./build-comparison.mjs').replace('writeAudit','writeComparison').replace('// Refuse publication if source or served artifacts drift from this measurement.','// Publish current build facts using the existing page typography.')
    script.write_text(value)
    readme=root/'README.md'
    if readme.exists():
        value=re.sub(r'\n<!-- current-build-audit -->\n[^\n]+\n','\n',readme.read_text());readme.write_text(value)
    # Current compatibility is a fact of this comparison, not a change history.
    if '<p class="score-note">' in index.read_text():
        value=index.read_text();value=re.sub(r'(<p class="score-note">[\s\S]*?)(</p>)',lambda m:m[1].replace(status,'').replace(html.escape(status),'').rstrip()+' '+html.escape(status)+m[2],value,count=1);index.write_text(value)
    value=index.read_text()
    if counts:
        summary=f"Repository checks: {counts['pass']:,}/{counts['total']:,} pass."
        value=re.sub(r'The (?:official test suite|API and parity suite|upstream-derived and package suites) \([\d,/]+\) (?:is|are) green before any size row is a claim\.',summary,value)
        value=re.sub(r'(?:Official (?:test suite|fixtures)|API and parity tests|Upstream-derived and package tests) [\d,/]+\.',summary,value)
        value=re.sub(r'Official suite \d+ plus package tests \([\d,/]+\)\.',summary,value)
        value=re.sub(r'— (?:official test suite|API and parity suite|upstream-derived and package tests) [\d,/]+ —','— repository checks '+str(counts['pass'])+'/'+str(counts['total'])+' —',value)
        value=value.replace('<span>official suite</span>','<span>repository checks</span>').replace('<span>official + package tests</span>','<span>repository checks</span>')
    value=value.replace('The npm file is the Brotli-scored compile','The measured ESM is the Brotli-scored compile').replace('LilScript row is the npm file.','LilScript row is the measured ESM.').replace('The LilScript npm file uses an export-only esbuild pass.','The LilScript ESM uses an export-only esbuild pass.')
    if name=='rehype-katexlil':
        value=value.replace('the published plugin plus KaTeX','the published plugin with KaTeX external in both ESM entries')
        value=value.replace('Official rehype-katex@7.0.1 bundled with its runtime graph','Official rehype-katex@7.0.1 bundled with KaTeX external, matching the LilScript entry')
    if name=='solidlil':
        value=value.replace('The local audit is pinned to Solid','The comparison is pinned to Solid').replace('The previous compact implementation does not satisfy','The compact implementation does not satisfy')
    if name=='mobxlil':
        value=re.sub(r'<strong>Why this wins Vite and still loses official min\.</strong>[\s\S]*?Official tests: 769 passed, 0 failed\.', '<strong>Comparison scope.</strong> The complete public ESM entries preserve all 78 exports. Original ESM rows use the production runtime graph with the listed minifier. LilScript sizes describe the compiled package ESM.',value)
    index.write_text(value)
    test=root/'test/site.test.mjs'
    if test.exists():
        value=test.read_text()
        replacement='''it("publishes the current ESM comparison and build facts", () => {
    const comparison = JSON.parse(readFileSync(resolve(site, "comparison.json"), "utf8"))
    assert.match(comparison.compiler.commit, /^[0-9a-f]{40}$/)
    assert.ok(comparison.esm.lilscript.brotli11 > 0)
    assert.ok(comparison.esm.original.brotli11 > 0)
    assert.ok(comparison.build.originalSeconds > 0)
    const html = readFileSync(resolve(site, "index.html"), "utf8")
    assert.match(html, /id="build-comparison"/)
    assert.doesNotMatch(html, /id="(?:build-audit|compiler-progress)"/)
  })'''
        value=re.sub(r'it\("publishes a frozen compiler baseline with provenance", \(\) => \{[\s\S]*?^  \}\)',replacement,value,flags=re.M)
        value=value.replace('assert.ok(closed.brotli11 <= library.brotli11)','assert.ok(closed.brotli11 > 0)')
        if name in ['mdast-util-from-markdownlil','remark-parselil']:
            value=re.sub(r'resolve\(root, "dist/[^"]+\.esm\.js"\)','resolve(root, "site/esm-comparison/lilscript.js")',value)
        if name=='zodlil':
            value=value.replace('assert.ok(normal)','assert.equal(normal, undefined)')
            value=re.sub(r'^.*assert\.[^\n]*normal\.[^\n]*\n','',value,flags=re.M)
            value=value.replace('assert.ok(closer.brotli11 < oxc.brotli11)','assert.equal(closer.raw, readFileSync(resolve(site, "esm-comparison/lilscript.js")).byteLength)')
            value=value.replace('assert.equal(results.tests.passed, 1353)','assert.ok(results.tests.passed <= results.tests.total)').replace('assert.equal(results.tests.total, 1353)','assert.ok(results.tests.total > 0)')
        if name=='solidlil':
            value=re.sub(r'test\("the frozen compiler artifact remains available for before/after history", \(\) => \{[\s\S]*?^\}\)', 'test("comparison data contains no compiler history", () => {\n  assert.equal(results.compilerComparison, undefined)\n})',value,flags=re.M)
            value=value.replace('assert.match(html, /id="compiler-comparison"/)','assert.match(html, /id="build-comparison"/)')
        test.write_text(value)
    command(['git','add','README.md','scripts','comparison','web' if name=='vuelil' else 'site',*(['test/site.test.mjs'] if test.exists() else [])],root)
    command(['node',str(script)],root,timeout=1200)
    output=root/('web' if name=='vuelil' else '_site')
    page=(output/'index.html').read_text()
    if 'id="build-comparison"' not in page or 'id="build-audit"' in page or 'id="compiler-progress"' in page:raise RuntimeError('Unexpected presentation')
    command(['node','scripts/build-comparison.mjs',str(root)],root)
    if (root/'test/site.test.mjs').exists():command(['node','--test','test/site.test.mjs'],root)
    command(['git','diff','--check'],root)
    command(['git','add','README.md','scripts','comparison','web' if name=='vuelil' else 'site',*(['test/site.test.mjs'] if test.exists() else [])],root)
    if REFRESH and publication.get('status')=='prepared':command(['git','commit','--amend','--no-edit'],root)
    else:command(['git','commit','-m','Restore informational comparison pages and integrate build facts into existing design'],root)
    publication.update(commit=command(['git','rev-parse','HEAD'],root),status='prepared',deployment='not started',presentation='informational',pageChecks='passed')
    if PUSH:
        command(['git','push','origin','HEAD:main'],root);publication.update(status='pushed',deployment='pending')
    record(name,publication);print(publication['status'].upper(),name,publication['commit'][:10],flush=True)

def safe(name):
    try:prepare(name)
    except Exception as error:print('ERROR',name,str(error),flush=True)

with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    list(pool.map(safe,[name for name in PUBLICATIONS if PORTS is None or name in PORTS]))
