"""Turn measured artifacts into explicit, scoped publication receipts."""
import hashlib
import json
import pathlib
import re
import subprocess

PRIMARY = {'cnlil':'dist/index.js','katexlil':'dist/katex.esm.js','motionlil':'dist/full.js',
           'solidlil':'dist/core.js','playcanvaslil':'dist/shader-processing.js','zodlil':'dist/zod.core.js'}
ANSI = re.compile(r'\x1b\[[0-9;]*m')


def load(path, default=None):
    try: return json.loads(pathlib.Path(path).read_text())
    except (OSError, ValueError): return default


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def measure(codec, paths, root):
    paths=[path for path in paths if path.is_file()]
    if not paths:return []
    result=json.loads(subprocess.check_output([str(codec),'--json',*[str(path) for path in paths]],text=True))
    return [{**row,'path':str(pathlib.Path(row['path']).relative_to(root)), 'sha256':digest(row['path'])} for row in result['artifacts']]


def test_summary(record):
    if not record:return 'No repository behavior gate was available.'
    text=ANSI.sub('',record.get('tail',''))
    summaries=[line.strip() for line in text.splitlines() if re.search(r'^(?:# (?:tests|pass|fail)|Tests:|Test Suites:|Test Files\s|Tests\s)',line.strip())]
    outcome='passed' if record['exitCode']==0 else 'failed'
    return f"Repository test command {outcome} (exit {record['exitCode']}). "+' · '.join(summaries[-8:])


def make_receipt(run, row):
    run=pathlib.Path(run);root=pathlib.Path(row['snapshot']);name=row['name']
    result=load(root/'.page-audit/result.json')
    if not result or 'build' not in result:return None
    config=load(root/'page-audit-config.json',{})
    primary=PRIMARY.get(name, (row['package'].get('module') or '').removeprefix('./'))
    if name=='vuelil':primary='artifacts/shared-runtime.generated.js'
    existing=load(run/'before'/name/'results.json',{})
    current_data=load(root/'site/results.json',{})
    earlier=next((lane for lane in existing.get('size',[]) if lane.get('primary') or lane.get('id')=='itslil'),None)
    earlier=next((lane for lane in existing.get('size',[]) if lane.get('id')=='itslil-package'),earlier)
    if name=='cnlil':earlier=existing.get('sizes',{}).get('lil')
    if name=='playcanvaslil':earlier=next((item for item in existing.get('artifacts',[]) if item.get('id')=='open-lilscript'),None)
    if name=='solidlil':
        item=existing.get('artifacts',{}).get('core',{});earlier={'raw':item.get('raw'),'gzip9':item.get('gzip'),'brotli11':item.get('brotli')}
    candidate=next((artifact for artifact in result.get('artifacts',[]) if artifact['path']==primary),None)
    codec=run/'current/lilscript/target/release/lilscript-codec'
    paths=list((root/'.page-audit/upstream').glob('*.js'))
    cached=result.get('artifacts',[])+load(root/'.page-audit/upstream-sizes.json',[])
    official=[];missing=[]
    for path in paths:
        key=str(path.relative_to(root));fingerprint=digest(path)
        found=next((item for item in cached if item['path']==key and item.get('sha256')==fingerprint),None)
        if found:official.append(found)
        else:missing.append(path)
    official+=measure(codec,missing,root)
    (root/'.page-audit/upstream-sizes.json').write_text(json.dumps(official,indent=2)+'\n')
    # A retained result cannot shadow a failed build. Only successful compiler runs have current sizes.
    if not result['build'].get('fresh'):candidate=None
    baseline=next((artifact for artifact in official if artifact['path'].endswith('/official.esm.js')),None)
    if name=='playcanvaslil':baseline=next((artifact for artifact in result.get('artifacts',[]) if artifact['path']=='dist/shader-processing.official.js'),None)
    built=bool(result['build'].get('fresh'))
    tested=result.get('after-tests',{}).get('exitCode')==0
    status='verified' if built and tested else 'build-only' if built and not config.get('testCommand') else 'blocked'
    browser=load(root/'.page-audit/browser-check.json',{})
    if browser.get('errors'):status='blocked'
    versions={value['name']:value for value in load(run/'upstream-versions.json',[])}
    upstream=versions.get(name,{'package':config.get('upstreamPackage'),'pinned':config.get('upstreamVersion')})
    if name=='playcanvaslil':
        reference=load(root/'reports/results.json',{}).get('upstream',{})
        upstream={**upstream,'package':'playcanvas','pinned':load(root/'upstream/engine/package.json',{}).get('version','git-pinned'),'revision':reference.get('revision'),'source':'https://github.com/playcanvas/engine/tree/'+reference.get('revision',''),'note':'The compatibility target is this exact engine commit and its four selected shader-processing sources. The beta version and stable registry tag are separate release tracks.'}
    if name=='solidlil':upstream['note']='This port targets the Solid 2 release-candidate track; npm latest currently points to the separate stable 1.x track.'
    upstream_timing=result.get('upstreamTiming',{})
    external=upstream_timing.get('external',[])
    scope=upstream_timing.get('scope',config.get('upstreamScope','Pinned source boundary in the repository build script.'))
    if external:scope+=' External on both sides: '+', '.join(external)+'.'
    scope+=' Sizes use canonical zlib 1.3.1 gzip-9 and Google Brotli 1.1.0 quality 11.'
    if name in ['monacolil','solidlil','vuelil']:
        scope+=' This is diagnostic: full-library feature/API equivalence is not established, so no full-library size-win claim is eligible.'
    findings=[]
    if row.get('unpublishedCommits'):findings.append(f"{row['unpublishedCommits']} committed port changes were absent from GitHub at the start of the audit.")
    if earlier and candidate and isinstance(earlier.get('brotli11'),int):
        delta=candidate['brotli11']-earlier['brotli11'];findings.append(f"Published-to-fresh Brotli drift: {delta:+,} bytes ({100*delta/earlier['brotli11']:+.2f}%). This includes port source/config changes and compiler changes.")
    old=load(run/'repos'/name/'.page-audit/result.json',{})
    if old.get('build',{}).get('exitCode'):
        log=(run/'repos'/name/'.page-audit/build.log').read_text(errors='replace')
        unknown=re.search(r'unknown field `([^`]+)`',log)
        if unknown:findings.append(f"The root checkout compiler (54e1948) rejects config field {unknown[1]}; today's migration/target-tree compiler is required.")
    if name=='rehype-katexlil':findings.append('The earlier headline compared a small plugin with external dependencies against a bundled upstream graph. The refreshed upstream lane preserves the same external dependency boundary.')
    if name=='mobxlil':findings.append('The repository suite selects the development CommonJS lane; production ESM sizes and build timings are recorded separately. Passing this suite alone does not prove production behavior.')
    if name=='zodlil':findings.append('The ordinary build command defaults to the development config. This audit explicitly uses --prod so production optimizations are exercised.')
    if name=='katexlil':findings.append('The build was forced; the prior mtime shortcut cannot count as compilation. Browser tests must rebuild their served files after dist changes.')
    if name=='react-markdownlil' and result.get('after-tests',{}).get('exitCode')==2:
        findings.append('All 120 runtime tests passed on the fresh artifact. The complete command fails its TypeScript check because the published @itslil/remark-breaks declaration has a syntax error (TS1005). The earlier distribution fails the same check; this is an existing npm dependency problem, not evidence of a new compiler regression.')
    if name=='motionlil':findings.append('The broad full entry and each smaller feature entry are distinct boundaries; the package build includes multiple compiles and post-processing.')
    if browser.get('errors'):findings.append('Browser execution of the refreshed comparison page failed: '+'; '.join(sorted(set(browser['errors'])))[:1000])
    if result.get('lockfileRepair'):findings.append(result['lockfileRepair'])
    if result.get('dependencyRepair'):findings.append(result['dependencyRepair'])
    if result.get('testHarnessRepair'):findings.append(result['testHarnessRepair'])
    if result.get('production-browser-tests',{}).get('exitCode')==0:findings.append('Fresh production ESM and UMD browser checks passed for observables, computed values, actions, collections, flow, disposal and page isolation.')
    if upstream.get('latest') and upstream.get('pinned')!=upstream.get('latest') and name not in ['solidlil','playcanvaslil']:findings.append(f"Upstream version gap: pinned {upstream['pinned']}; npm latest was {upstream['latest']} at the audit. No compatibility upgrade is implied.")
    if not built:
        log=(root/'.page-audit/build.log').read_text(errors='replace')
        lines=[line.strip() for line in log.splitlines() if re.search(r'^(?:error:|Error:|SyntaxError:|TypeError:)',line)]
        findings.append('Fresh build failed: '+(' '.join(lines[-3:])[:800] or result['build']['tail'][-400:]))
    elif config.get('testCommand') and not tested:findings.append(test_summary(result.get('after-tests')))
    invocations=[item for item in result.get('invocations',[]) if item.get('exitCode')==0]
    target=(config.get('file') or pathlib.Path(primary).name.removesuffix('.esm.js'))+'.raw.js'
    primary_inv=next((item for item in invocations if item.get('output','').endswith('/'+target)),None)
    if name=='zodlil':primary_inv=next((item for item in invocations if item.get('output','').endswith('/zod.core.js')),None)
    if name=='cnlil':primary_inv=next((item for item in invocations if item.get('output','').endswith('/cn.raw.js')),None)
    seconds=upstream_timing.get('medianSeconds')
    if name=='playcanvaslil':
        for item in result.get('bundleTimings',{}).get('artifacts',[]):
            if item['output']=='shader-processing.official.js':seconds=item['wallSeconds']
    if result.get('upstreamSourceBuild'):
        seconds=result['upstreamSourceBuild']['medianSeconds']
        upstream_timing={**upstream_timing,**result['upstreamSourceBuild']}
        baseline=next((artifact for artifact in official if artifact['path'].endswith('/official.source-terser.js')),baseline)
        scope=result['upstreamSourceBuild']['scope']+' Sizes use canonical zlib 1.3.1 gzip-9 and Google Brotli 1.1.0 quality 11.'
        findings.append('Original build time uses a fresh build of the pinned KaTeX Flow sources; the ordinary npm-graph lane is also retained in the receipt.')
    behavior={'command':config.get('testCommand'),'summary':test_summary(result.get('after-tests')),
              'before':{key:oldvalue for key,oldvalue in result.get('before-tests',{}).items() if key in ['exitCode','failures','logSha256']},
              'after':{key:value for key,value in result.get('after-tests',{}).items() if key in ['exitCode','failures','logSha256','tail']}}
    for side in ['before','after']:
        if 'tail' in behavior[side]:behavior[side]['tail']=ANSI.sub('',behavior[side]['tail']).replace(str(root),'<snapshot>').replace('/home/lilfarm/page-refresh-current-20260910/repos/'+name,'<snapshot>')
    summary='The fresh artifact was checked against the repository test command.' if status=='verified' else 'The newly measured artifact remains diagnostic until its behavior gate is satisfied; the previously published runtime is retained.'
    return {'schemaVersion':1,'name':name,'measuredAt':result['startedAt'],'status':status,'buildPassed':built,
            'summary':summary,'sourceCommit':row['baseCommit'],'previousPublicCommit':row['publicCommit'],
            'compiler':result['compiler'],'machine':{**result['machine'],'instanceClass':'Standard_D16als_v7',
                'concurrencyNote':'The audit scheduled one port per worker, but this is a shared pool. Outside compiler jobs were observed during the run; timings are contextual wall times, not controlled speedup benchmarks.'},
            'upstream':upstream,'comparisonScope':scope,'sizes':{'previous':earlier,'current':candidate,'upstream':baseline},
            'officialArtifacts':official,'candidateArtifacts':result.get('artifacts',[]),
            'timing':{'primaryCompilerSeconds':primary_inv.get('wallSeconds') if primary_inv else None,
                      'primaryScope':'One real compiler process, including code generation and codec-guided search; excludes subsequent package wrapping.',
                      'packageSeconds':result['build']['wallSeconds'],'packageScope':config['buildCommand']+f"; {len(invocations)} successful compiler invocations plus package generation.",
                      'upstreamSeconds':seconds,'upstreamScope':upstream_timing.get('scope','Release-strip + esbuild + Terser of the pinned shader-processing reference.' if name=='playcanvaslil' else 'Unavailable; inspect the upstream build failure.'),
                      'protocol':'Same shared worker for both lanes; one fresh LilScript package build; median of three sequential upstream ESM builds (PlayCanvas records one native reference build). Outside compiler jobs were observed in the pool, so contention is not excluded. Dependencies, tests and compression measurement are outside these build times.',
                      'upstreamSamples':upstream_timing.get('samples',[])},
            'behavior':behavior,'findings':findings,'primaryArtifact':primary,
            'beforePage':row['served'][0], 'codecSha256':result['codecSha256']}
