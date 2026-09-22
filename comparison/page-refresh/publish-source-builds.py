#!/usr/bin/env python3
"""Publish recorded source builds inside the established comparison pages."""
import ast,copy,hashlib,html,json,os,pathlib,re,shutil,statistics,subprocess,sys
RUN=pathlib.Path(sys.argv[1]).resolve();OLD=pathlib.Path('/tmp/lilscript-page-refresh-20260910');TOOLS=pathlib.Path(__file__).parent.resolve()
JOBS=json.loads((RUN/'jobs.json').read_text());PUB=json.loads(pathlib.Path(os.environ.get('PUBLICATIONS_FILE',OLD/'publications.json')).read_text())
PORTS=sys.argv[sys.argv.index('--ports')+1].split(',') if '--ports' in sys.argv else list(JOBS)
PUSH='--push' in sys.argv

def read(p):return json.loads(p.read_text())
def write(p,j):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(j,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def cmd(args,root,timeout=600):
 env={**os.environ,'PATH':'/home/azureuser/.nvm/versions/node/v24.11.1/bin:'+os.environ['PATH']}
 r=subprocess.run(args,cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=timeout)
 if r.returncode:raise RuntimeError(' '.join(args)+'\n'+r.stdout[-5000:])
 return r.stdout.strip()
def summarize(samples):
 good=[s['wallSeconds'] for s in samples if s['exitCode']==0]
 return {'complete':len(samples)==3 and len(good)==3,'medianSeconds':statistics.median(good) if good else None,'minimumSeconds':min(good) if good else None,'maximumSeconds':max(good) if good else None,'samples':[{k:s[k] for k in ['command','wallSeconds','cpuSeconds','exitCode','timedOut','log','logSha256']} for s in samples]}
def counts(step):
 text=re.sub(r'\x1b\[[0-9;]*m','',step.get('tail',''))
 def last(key):
  found=re.findall(r'^(?:# |ℹ )?'+key+r' (\d+)',text,re.M)
  return int(found[-1]) if found else None
 total=last('tests');passed=last('pass')
 if total is not None and passed is not None:return {'pass':passed,'total':total}
 return None
# Reuse the size-lane mapping from the informational renderer without executing its publisher.
module=ast.parse((TOOLS/'informational-pages.py').read_text());ns={'copy':copy,'pathlib':pathlib,'re':re}
for node in module.body:
 if isinstance(node,ast.FunctionDef) and node.name in ['test_counts','refresh_size_rows']:
  exec(compile(ast.Module(body=[node],type_ignores=[]),'<size-lane-renderer>','exec'),ns)

def prepare(name):
 job=JOBS[name];root=pathlib.Path(PUB[name]['path']);site=root/('web' if name=='vuelil' else 'site');out=RUN/'results'/name
 result=read(out/'result.json');
 if result['portSource']['commit']!=job['portSource']['commit'] or result['nativeBuilds'][0]['command']!=job['build']:raise RuntimeError('Waiting for the final configured source build')
 esm=read(out/'esm.json') if (out/'esm.json').exists() else None
 if not esm or esm['source']['commit']!=job['source']['commit']:raise RuntimeError('No matching source-built original ESM for '+name)
 data=read(site/'comparison.json');olddata=copy.deepcopy(data);guard=read(root/'comparison/build-receipt.json')
 css={str(p.relative_to(root)):sha(p) for p in site.rglob('*.css')}
 original=summarize(result['nativeBuilds']);lil=summarize(result['lilscriptBuilds']);step=result['steps'].get('lilscript-tests',{});checks=counts(step)
 if name=='motionlil' and (out/'lilscript-tests.log').exists():
  testlog=re.sub(r'\x1b\[[0-9;]*m','',(out/'lilscript-tests.log').read_text())
  checks={key:sum(map(int,re.findall(r'^(?:# |ℹ )?'+label+r' (\d+)',testlog,re.M))) for key,label in [('pass','pass'),('total','tests')]}
 status='Repository checks completed successfully.' if step.get('exitCode')==0 else 'Compatibility checks are incomplete.'
 if checks:status=f"Runtime checks: {checks['pass']:,}/{checks['total']:,} pass."+(' Declaration or package checks fail.' if step.get('exitCode') and checks['pass']==checks['total'] else '')
 if not lil['complete']:status='The compiler did not complete this package build.'
 if name=='motionlil':status=f"Recorded runtime checks: {checks['pass']}/{checks['total']} pass, including backend, interpolation and lifecycle comparisons against the original source-built Motion ESM. Performance results are gated on matching native backends and sampled values." if checks else 'Runtime validation has not completed.'
 scope='Both timings cover each repository’s package build command; their output formats and build checks can differ.'
 if name=='posthoglil':scope='Original timing covers the complete PostHog SDK and its workspace builds; LilScript and the ESM byte comparison cover the selected kernel only.'
 if name=='playcanvaslil':scope='Original timing covers the complete engine release ESM build; LilScript and the byte comparison cover the shader-processing core only.'
 if name in ['rehypelil','rehype-stringifylil','remarklil','remark-parselil','remark-gfmlil','remark-mathlil','rehype-katexlil']:scope='The original command builds the upstream monorepo’s generated types/checks; the byte comparison includes the selected package runtime graph.'
 sourceBuild={'original':original,'lilscript':lil,'scopeNote':scope,'cleanOutputs':True,'installationExcluded':True}
 data.update(schemaVersion=2,measuredAt=result.get('finishedAt',result['startedAt']),compiler=result['compiler'],upstream={k:job['source'][k] for k in ['package','version','repository','commit']},machine=result['machine'],buildComplete=lil['complete'],sourceBuild=sourceBuild,compatibility=status,tests=checks)
 data['scope']=olddata['scope'].split('; dependency installation')[0].replace('; this is a comparison bundle build, not the upstream vendor release pipeline','')+'. Original ESM assembled from the pinned Git source build.'
 if name=='motionlil':data['scope']=job.get('comparisonScope',job['esmScope']+'. LilScript default public ESM entry; React-specific entries are excluded.')+' Canonical gzip-9 and Brotli-11 compression.'
 data['build']={'compilerSeconds':None,'packageSeconds':lil['medianSeconds'] if lil['complete'] else result['lilscriptBuilds'][-1]['wallSeconds'] if result['lilscriptBuilds'] else None,'originalSeconds':original['medianSeconds'],'originalEsmSeconds':esm['medianSeconds'],'packageScope':job['lilscriptBuild'],'originalScope':' '.join(job['build']),'protocol':result['protocol']}
 artifactMap={a['path']:a for a in result.get('lilscriptArtifacts',[])}
 primary=next((a['path'] for a in guard['candidateArtifacts'] if olddata['esm'].get('lilscript') and a['sha256']==olddata['esm']['lilscript']['sha256']),None)
 module=read(pathlib.Path(job['portSource']['snapshot'])/'package.json').get('module')
 if module:primary=module.removeprefix('./')
 if name=='motionlil':primary=job.get('comparisonEntry','dist/index.bundle.js')
 if name=='playcanvaslil':primary='dist/shader-processing.js'
 lilArtifact=artifactMap.get(primary) if lil['complete'] else None
 lilPath=RUN/'artifacts'/name/primary.removeprefix('dist/') if primary else None
 if name=='zodlil' and lil['complete']:
  lilArtifact=read(out/'lilscript-esm.json');lilPath=out/'lilscript.esm.js';data['assembly']=lilArtifact['assembly']
 originalArtifact=next(a for a in esm['artifacts'] if a['path']=='original.esm.js')
 guard['comparisonArtifacts']=[]
 for lane,artifact,path in [('original',originalArtifact,out/'original.esm.js'),('lilscript',lilArtifact,lilPath)]:
  target=site/'esm-comparison'/(lane+'.js');target.parent.mkdir(exist_ok=True)
  if artifact is None:
   data['esm'][lane]=None;target.unlink(missing_ok=True);continue
  if sha(path)!=artifact['sha256']:raise RuntimeError('Measured artifact changed '+str(path))
  shutil.copy2(path,target);data['esm'][lane]={k:artifact[k] for k in ['raw','gzip9','brotli11','sha256']};data['esm'][lane]['file']='./esm-comparison/'+lane+'.js'
  guard['comparisonArtifacts'].append({'path':str(target.relative_to(root)),'sha256':sha(target)})
 # Full raw evidence and dependency locks remain in the repository. Public JSON includes commands and samples.
 records=root/'comparison/source-build';records.mkdir(parents=True,exist_ok=True)
 for p in out.iterdir():
  if p.is_file() and (p.suffix=='.log' or p.name.endswith(('lock.json','lock.yaml','yarn.lock')) or p.name in ['result.json','esm.json','compiler-invocations.jsonl']):shutil.copy2(p,records/p.name)
 evidence=copy.deepcopy(result);evidence.pop('workerIp',None);evidence['source']=job['source'];evidence['esm']=esm;evidence['sourceBuild']=sourceBuild;evidence['prerequisiteAdjustments']=job.get('prerequisiteAdjustments');evidence['portSource']=job['portSource'];evidence['portSource'].pop('snapshot',None)
 evidence['status']='build-failed' if not lil['complete'] else 'checks-failed' if step.get('exitCode',1) else 'complete';evidence['recordRepository']='https://github.com/yeargun/'+name+'/tree/main/comparison/source-build'
 write(site/'source-build.json',evidence);write(site/'comparison.json',data)
 guard['comparisonArtifacts'] += [{'path':str(p.relative_to(root)),'sha256':sha(p)} for p in [site/'source-build.json',site/'comparison.json']]
 guard['sourceBuildReceipt']='comparison/source-build/result.json';guard['currentMeasurement']={'compiler':result['compiler'],'source':job['portSource'],'upstream':job['source'],'compatibility':status}
 # Use the same established rows, colors and markup with measured bytes.
 resultsPath=site/'results.json'
 if resultsPath.exists() and lilArtifact:
  results=read(resultsPath)
  receipt={'name':name,'candidateArtifacts':result.get('lilscriptArtifacts',[]),'officialArtifacts':[{**a,'path':a['path'].replace('original.','official.')} for a in esm['artifacts']],'sizes':{'current':lilArtifact},'measuredAt':data['measuredAt'],'compiler':data['compiler'],'behavior':{'after':{'tail':step.get('tail','')}}}
  ns['refresh_size_rows'](results,receipt)
  if checks and 'spec' in results:results['spec']={**results['spec'],**checks,'label':'runtime checks'}
  write(resultsPath,results)
 index=site/'index.html';s=index.read_text()
 # Replace existing numeric static fallbacks; dynamic renderers use the same JSON.
 for lane in ['lilscript','original']:
  before=olddata['esm'].get(lane);after=data['esm'].get(lane)
  if before and after:
   replacements={f"{before[k]:,} B":f"{after[k]:,} B" for k in ['raw','gzip9','brotli11']}
   for a,b in replacements.items():s=s.replace(a,b)
 if olddata['esm'].get('lilscript') and data['esm'].get('lilscript'):
  before=olddata['esm']['lilscript']['brotli11']/olddata['esm']['original']['brotli11'];after=data['esm']['lilscript']['brotli11']/data['esm']['original']['brotli11'];s=s.replace(f'{before:.2f}<span>× Brotli',f'{after:.2f}<span>× Brotli')
 s=re.sub(r'(?:(?:Browser )?[Cc]ompatibility checks (?:are incomplete|: [\d,/]+ pass)|[Rr]epository checks[: ].*?pass)\.',status,s)
 if name=='motionlil':
  s=s.replace('published package runtime','current compiled runtime')
  maximum=max(data['esm']['original']['brotli11'],data['esm']['lilscript']['brotli11'])
  for cls,lane in [('bar-motion','original'),('bar-lil','lilscript')]:s=re.sub(r'(<div class="'+cls+r'" style="width:)[^"]+',lambda m:m[1]+f"{100*data['esm'][lane]['brotli11']/maximum:.3f}%",s)
 index.write_text(s)
 shutil.copy2(TOOLS/'build-comparison.mjs',root/'scripts/build-comparison.mjs')
 guard['publicationSourceFingerprint']=cmd(['node','--input-type=module','-e','import {sourceFingerprint} from "./scripts/build-comparison.mjs"; console.log(sourceFingerprint(process.cwd()))'],root)
 write(root/'comparison/build-receipt.json',guard)
 if {str(p.relative_to(root)):sha(p) for p in site.rglob('*.css')}!=css:raise RuntimeError('Stylesheet changed')
 write(RUN/'prepared'/f'{name}.json',{'name':name,'css':css,'compatibility':status,'compiler':data['compiler'],'sourceBuild':sourceBuild})
 print('PREPARED',name,flush=True)

for name in PORTS:
 try:prepare(name)
 except Exception as e:print('ERROR',name,e,flush=True)
