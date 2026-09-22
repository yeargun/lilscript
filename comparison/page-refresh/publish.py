#!/usr/bin/env python3
"""Prepare, validate and optionally publish each library's reviewable Pages refresh."""
import concurrent.futures
import copy
import fcntl
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import threading
from receipts import load, make_receipt, digest

RUN=pathlib.Path(sys.argv[1]).resolve()
TOOLS=pathlib.Path(__file__).resolve().parent
PUSH='--push' in sys.argv
REFRESH='--refresh' in sys.argv
WANTED=sys.argv[sys.argv.index('--ports')+1].split(',') if '--ports' in sys.argv else None
rows=load(RUN/'prepared-current.json',[])
rows=[row for row in rows if WANTED is None or row['name'] in WANTED]
NODE='/home/azureuser/.nvm/versions/node/v22.23.2/bin'
ENV={**os.environ,'PATH':NODE+':'+os.environ['PATH']}
PUBLICATIONS=load(RUN/'publications.json',{})
lock=threading.Lock()


def record_publication(name, value):
    with lock, (RUN/'publications.lock').open('w') as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        latest=load(RUN/'publications.json',{})
        latest[name]=value
        temporary=RUN/('publications.'+str(os.getpid())+'.tmp')
        temporary.write_text(json.dumps(latest,indent=2)+'\n')
        temporary.replace(RUN/'publications.json')
        PUBLICATIONS[name]=value


def command(args,cwd=None,timeout=600):
    env=ENV if not cwd or pathlib.Path(cwd).name!='vuelil' else {**ENV,'PATH':'/home/azureuser/.nvm/versions/node/v24.11.1/bin:'+os.environ['PATH']}
    proc=subprocess.run(args,cwd=cwd,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout)
    if proc.returncode:raise RuntimeError(' '.join(args)+':\n'+proc.stdout[-3000:])
    return proc.stdout.strip()


def refresh_results(root,receipt):
    if receipt['name']=='playcanvaslil' and receipt['status']=='verified':
        report=root/'reports/results.json';data=load(report,{})
        values={item['path']:item for item in receipt['candidateArtifacts']}
        for item in data['artifacts']:
            item.update({key:values[item['path']][key] for key in ['raw','gzip9','brotli11','sha256']})
        for world in ['open','closed']:
            candidate=next(item for item in data['artifacts'] if item['id']==world+'-lilscript')
            baseline=next(item for item in data['artifacts'] if item['id']==world+'-official')
            data['comparison'][world]={key:{'candidate':candidate[key],'baseline':baseline[key],'differencePercent':round((candidate[key]/baseline[key]-1)*100,2)} for key in ['raw','gzip9','brotli11']}
        data.update(generatedAt=receipt['measuredAt'],buildAudit='./build-audit.json',compiler={'revision':receipt['compiler']['commit'],'dirty':False,'binarySha256':receipt['compiler']['sha256']},codecBinarySha256=receipt['codecSha256'])
        data['portSourceSha256']={path:digest(root/path) for path in data['portSourceSha256']}
        report.write_text(json.dumps(data,indent=2)+'\n')
        (root/'LILSCRIPT_REVISION').write_text(receipt['compiler']['commit']+'\n')
    if receipt['name']=='solidlil' and receipt['status']=='verified':
        report=root/'site/results.json';data=load(report,{})
        values={item['path']:item for item in receipt['candidateArtifacts']}
        for name in data.get('artifacts',{}):
            item=values.get('dist/'+name+'.js')
            if item:data['artifacts'][name].update(raw=item['raw'],gzip=item['gzip9'],brotli=item['brotli11'],sha256=item['sha256'])
        data.update(artifactMeasuredAt=receipt['measuredAt'],buildAudit='./build-audit.json',applicationEvidenceStatus='Historical application bundles; see generatedAt. Only the direct runtime artifacts were remeasured for this compiler.')
        report.write_text(json.dumps(data,indent=2)+'\n')
    if receipt['name']=='cnlil' and receipt['status']=='verified':
        report=root/'reports/sizes.json';data=load(report,{})
        current=receipt['sizes']['current'];upstream=receipt['sizes']['upstream']
        if current and upstream:
            data.update({'generatedAt':receipt['measuredAt'],'boundary':receipt['comparisonScope'],
                         'lil':{key:current[key] for key in ['raw','gzip9','brotli11']},
                         'official':{key:upstream[key] for key in ['raw','gzip9','brotli11']},
                         'ratio':{key:current[key]/upstream[key] for key in ['raw','gzip9','brotli11']}})
            report.write_text(json.dumps(data,indent=2)+'\n')
    path=root/'site/results.json'
    if not path.exists() or receipt['status']!='verified':return
    data=load(path,{})
    if not isinstance(data.get('size'),list):return
    artifacts={item['path']:item for item in receipt['candidateArtifacts']}
    official={pathlib.Path(item['path']).name:item for item in receipt['officialArtifacts']}
    prefix='parse' if receipt['name']=='markedlil' else 'kernel' if receipt['name']=='posthoglil' else 'official'
    mapping={prefix:'official.unminified.js',prefix+'-terser-mangle':'official.esm.js',prefix+'-terser-nomangle':'official.terser-nomangle.js',prefix+'-esbuild':'official.esbuild.js',prefix+'-esbuild-esnext':'official.esbuild.js',prefix+'-oxc-mangle':'official.oxc-mangle.js',prefix+'-oxc-nomangle':'official.oxc-nomangle.js','official-source-terser':'official.source-terser.js','officialDev':'official.unminified.js','officialMin':'official.esm.js'}
    filename=data.get('file') or pathlib.Path(receipt['primaryArtifact']).name.removesuffix('.esm.js')
    candidates={'itslil':receipt['sizes']['current'],'itslil-package':receipt['sizes']['current']}
    if receipt['name'] in ['markedlil','posthoglil']:
        candidates['itslil']=artifacts.get('dist/'+filename+'.raw.js',candidates['itslil'])
    for suffix,part in [('closed','closed'),('gzip','gzip'),('bytes','bytes')]:
        candidates['itslil-'+suffix]=artifacts.get('dist/'+filename+'.'+part+'.js')
    refreshed=[]
    for item in data['size']:
        row=copy.deepcopy(item);identifier=row.get('id','')
        value=official.get(mapping.get(identifier,'')) or candidates.get(identifier)
        if not identifier:
            if row.get('primary'):value=receipt['sizes']['current']
            elif 'Oxc' in row.get('name',''):value=official.get('official.oxc-mangle.js')
            elif 'Terser' in row.get('name',''):value=official.get('official.esm.js')
        if not value:continue
        row.update({key:value[key] for key in ['raw','gzip9','brotli11']})
        row['measuredAt']=receipt['measuredAt'];row['artifactSha256']=value['sha256']
        if identifier in ['itslil','itslil-package']:
            direct=identifier=='itslil' and receipt['name'] in ['markedlil','posthoglil']
            row['note']=('Fresh direct compiler output, before the package banner; compiler ' if direct else 'Fresh packaged ESM; compiler ')+receipt['compiler']['commit'][:10]+'.'
        if identifier in ['officialDev','officialMin']:
            row['name']='Original · '+('unminified ESM graph' if identifier=='officialDev' else 'esbuild + Terser ESM')
            row['note']='Fresh pinned upstream comparison graph; the build audit states its external dependencies.'
        refreshed.append(row)
    if refreshed and any(item.get('primary') or item.get('id')=='itslil' for item in refreshed):data['size']=refreshed
    baseline=next((row for row in data['size'] if row.get('baseline')),None)
    if not baseline:
        baseline=next((row for row in data['size'] if row.get('id')==prefix+'-terser-mangle'),None)
        if baseline:baseline['baseline']=True
    lil=next((row for row in data['size'] if row.get('primary') or row.get('id')=='itslil'),None)
    if lil and baseline:
        gz=next((row for row in data['size'] if row.get('id')=='itslil-gzip'),lil)
        raw=next((row for row in data['size'] if row.get('id')=='itslil-bytes'),lil)
        if 'hero' in data:
            data['hero'].update({'brotliRatio':lil['brotli11']/baseline['brotli11'],'officialBrotli':baseline['brotli11'],'itslilBrotli':lil['brotli11'],'gzipRatio':gz['gzip9']/baseline['gzip9'],'officialGzip':baseline['gzip9'],'itslilGzip':gz['gzip9'],'rawRatio':raw['raw']/baseline['raw'],'officialRaw':baseline['raw'],'itslilRaw':raw['raw']})
            package=next((row for row in data['size'] if row.get('id')=='itslil-package'),None)
            if package:data['hero'].update(packageBrotli=package['brotli11'],packageRaw=package['raw'])
        if 'matched' in data:
            data['matched'].update({'raw':raw['raw'],'gzip9':gz['gzip9'],'brotli11':lil['brotli11'],'vsOxc':{'raw':raw['raw']/baseline['raw'],'gzip9':gz['gzip9']/baseline['gzip9'],'brotli11':lil['brotli11']/baseline['brotli11']}})
    data['sizeMeasuredAt']=receipt['measuredAt']
    data['buildAudit']='./build-audit.json'
    data['runtimeEvidenceStatus']='Historical; runtime throughput was not republished as a measurement of this compiler.'
    path.write_text(json.dumps(data,indent=2)+'\n')


def prepare(row):
    name=row['name'];source=pathlib.Path(row['snapshot'])
    receipt=make_receipt(RUN,row)
    if not receipt:print('PENDING',name,flush=True);return
    existing=PUBLICATIONS.get(name,{})
    if existing.get('commit') and existing.get('status') in ['pushed','prepared'] and not REFRESH:
        if existing['status']=='prepared' and PUSH:
            command(['node','scripts/build-audit.mjs',existing['path']],existing['path'])
            command(['git','push','origin','HEAD:main'],existing['path'])
            existing.update(status='pushed',deployment='pending')
            record_publication(name,existing)
            print('PUSHED',name,existing['commit'][:10],flush=True)
        return existing
    # Failed and unverified candidates can update the audit without replacing the public runtime.
    promote=receipt['status']=='verified'
    root=RUN/'publish'/name
    if not root.exists():
        root.parent.mkdir(exist_ok=True)
        command(['git','clone','--quiet','--shared',str(source),str(root)])
        command(['git','remote','set-url','origin',row.get('remote','https://github.com/yeargun/'+name+'.git')],root)
        command(['git','checkout','--quiet','-B','refresh-comparison-pages-20260910',row['baseCommit'] if promote else row['publicCommit']],root)
    if promote:
        tracked=command(['git','ls-files','--','dist','artifacts','packages'],source).splitlines()
        for relative in tracked:
            path=source/relative
            # Vue is handled through its own package/artifact build; for other ports dist is the shipping boundary.
            if not relative.startswith('dist/') and name!='vuelil':continue
            if path.is_file():
                (root/relative).parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,root/relative)
        for relative in ['package.json','package-lock.json','jest.config.cjs','scripts/lib/browser-bench.mjs']+(['scripts/build.mjs'] if name=='playcanvaslil' else []):
            if (source/relative).is_file():shutil.copy2(source/relative,root/relative)
    site=root/('web' if name=='vuelil' else 'site')
    site.mkdir(exist_ok=True)
    if promote and (source/'site/results.json').exists():
        shutil.copy2(source/'site/results.json',site/'results.json')
    # Preserve source receipts for the published historical lanes, and visibly date old runtime evidence.
    index=site/'index.html'
    if index.exists() and 'data-audit-history' not in index.read_text():
        value=index.read_text()
        note='<p data-audit-history="2026-09-10" style="max-width:1160px;margin:18px auto;padding:12px 20px;font:14px/1.6 system-ui;color:#344b54;background:#edf3f4">The build audit above records the current compiler check. '+('Runtime throughput and application benchmarks below retain their earlier measurement dates.' if promote else 'The earlier public runtime and its historical measurements are retained while the fresh candidate remains unverified.')+'</p>'
        value=value.replace('<section class="performance',note+'\n<section class="performance',1) if '<section class="performance' in value else re.sub(r'(<main\b[^>]*>)',r'\1\n'+note,value,count=1)
        index.write_text(value)
    (root/'scripts').mkdir(exist_ok=True)
    shutil.copy2(TOOLS/'build-audit.mjs',root/'scripts/build-audit.mjs')
    script=root/'scripts'/('build-pages.mjs' if name=='vuelil' else 'build-site.mjs')
    text=script.read_text()
    if './build-audit.mjs' not in text:
        text+='\n// Refuse publication if source or served artifacts drift from this measurement.\n'
        text+='await import("./build-audit.mjs").then(({writeAudit}) => writeAudit({root: projectRoot, output: webRoot}));\n' if name=='vuelil' else 'await import("./build-audit.mjs").then(({writeAudit}) => writeAudit({root, output}));\n'
        script.write_text(text)
    refresh_results(root,receipt)
    readme=root/'README.md'
    if readme.exists() and '<!-- current-build-audit -->' not in readme.read_text():
        value=readme.read_text();first=value.find('\n')
        note=f'\n\n<!-- current-build-audit -->\n**Build audit, 2026-09-10:** [{receipt["status"]}; compiler, machine, build times, version gaps and behavior checks](https://yeargun.github.io/{name}/#build-audit). The [JSON receipt]({"web" if name=="vuelil" else "site"}/build-audit.json) records the current comparison; older benchmark prose retains its original scope.\n'
        readme.write_text(value[:first]+note+value[first:])
    # All measured artifacts must still be the bytes on which tests ran.
    receipt['publicationArtifacts']=[]
    if promote:
        tracked=set(command(['git','ls-files','--','dist','artifacts','packages'],root).splitlines())
        for item in receipt['candidateArtifacts']:
            if item['path'] in tracked and (root/item['path']).exists():
                if digest(root/item['path'])!=item['sha256']:raise RuntimeError('Measured artifact changed during validation: '+item['path'])
                receipt['publicationArtifacts'].append({'path':item['path'],'sha256':item['sha256']})
    else:
        for relative in command(['git','ls-files','--','dist','artifacts','packages'],root).splitlines():
            if (root/relative).is_file() and pathlib.Path(relative).suffix in ['.js','.mjs','.cjs']:
                receipt['publicationArtifacts'].append({'path':relative,'sha256':digest(root/relative)})
    receipt['publicationPolicy']='Promote freshly tested distribution' if promote else 'Retain previous public distribution; publish the diagnostic audit'
    receipt['publicationBase']=row['baseCommit'] if promote else row['publicCommit']
    command(['git','add','README.md','scripts','site' if name!='vuelil' else 'web','package.json','package-lock.json'],root)
    if promote:
        command(['git','add','-u'],root)
    receipt['publicationSourceFingerprint']=command(['node','scripts/build-audit.mjs',str(root),'--fingerprint'],root)
    (site/'build-audit.json').write_text(json.dumps(receipt,indent=2)+'\n')
    command(['npm','ci','--no-audit','--no-fund'],root)
    if name=='playcanvaslil':command(['node','scripts/analyze-compression.mjs'],root)
    if name=='vuelil' and not (root/'upstream/vue/.git').exists():
        setup=root/'tooling/setup-upstream.mjs'
        command(['node',str(setup)],root,timeout=1200)
    build=command(['node',str(script)],root,timeout=1200)
    output=root/('web' if name=='vuelil' else '_site')
    if 'id="build-audit"' not in (output/'index.html').read_text():raise RuntimeError('Built page is missing its audit panel')
    command(['node','scripts/build-audit.mjs',str(root)],root)
    if (root/'test/site.test.mjs').exists():command(['node','--test','test/site.test.mjs'],root)
    command(['git','diff','--check'],root)
    command(['git','add','README.md','scripts','site' if name!='vuelil' else 'web'],root)
    if promote:command(['git','add','-u'],root)
    if REFRESH and existing.get('status')=='prepared':command(['git','commit','--amend','--no-edit'],root)
    else:command(['git','commit','-m','Refresh comparison pages with compiler drift and measured build provenance'],root)
    commit=command(['git','rev-parse','HEAD'],root)
    publication={'status':'prepared','commit':commit,'base':receipt['publicationBase'],'path':str(root),'promotedDistribution':promote,'pageChecks':'passed','deployment':'not started'}
    if PUSH:
        command(['git','push','origin','HEAD:main'],root)
        publication.update(status='pushed',deployment='pending')
    record_publication(name,publication)
    print(publication['status'].upper(),name,commit[:10],flush=True)
    return publication


def safe(row):
    try:return prepare(row)
    except Exception as error:
        print('ERROR',row['name'],str(error),flush=True)
        record_publication(row['name'],{'status':'error','error':str(error),'path':str(RUN/'publish'/row['name'])})


with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    list(pool.map(safe,rows))
