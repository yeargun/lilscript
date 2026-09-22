#!/usr/bin/env python3
"""Measure clean package builds from pinned source trees on a single worker."""
import hashlib,json,os,pathlib,platform,resource,shutil,signal,statistics,subprocess,sys,threading,time
base=pathlib.Path(sys.argv[1]).resolve();name=sys.argv[2];config=json.loads((base/'jobs.json').read_text())[name]
port=base/'ports'/name;upstream=base/'upstream'/name;out=base/'results'/name;out.mkdir(parents=True,exist_ok=True)
def digest(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def heartbeat():
 while True:
  p=pathlib.Path.home()/'lil/.heartbeat';p.parent.mkdir(exist_ok=True);p.touch();time.sleep(30)
threading.Thread(target=heartbeat,daemon=True).start()
env={**os.environ,'CI':'1','COREPACK_ENABLE_DOWNLOAD_PROMPT':'0','HUSKY':'0','PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD':'1','PUPPETEER_SKIP_DOWNLOAD':'1','RAYON_NUM_THREADS':'8','NODE_OPTIONS':'--max-old-space-size=12288'}
nodebin=base/'toolchain/node_modules/.bin';env['PATH']=str(nodebin)+':'+env['PATH']
wrapper=base/'tools/compiler-wrapper.py';compiler=base/'tools/lilscript';codec=base/'tools/lilscript-codec'
env.update(LILSCRIPT_COMPILER=str(wrapper),MOTIONLIL_LILSCRIPT_BIN=str(wrapper),SOLIDLIL_LILSCRIPT_BIN=str(wrapper),LILSCRIPT_CODEC=str(codec),LILSCRIPT_ROOT=str(base/'lilscript'),PAGE_AUDIT_REAL_COMPILER=str(compiler),PAGE_AUDIT_INVOCATIONS=str(out/'compiler-invocations.jsonl'))
machine={'provider':'Azure','instanceClass':'Standard_D16als_v7','cpu':next(l.split(':',1)[1].strip() for l in pathlib.Path('/proc/cpuinfo').read_text().splitlines() if l.startswith('model name')),'logicalCpus':os.cpu_count(),'memoryBytes':os.sysconf('SC_PAGE_SIZE')*os.sysconf('SC_PHYS_PAGES'),'os':platform.freedesktop_os_release()['PRETTY_NAME'],'kernel':platform.release(),'node':subprocess.check_output(['node','--version'],env=env,text=True).strip(),'rayonThreads':8,'loadAtStart':os.getloadavg()}
result={'schemaVersion':1,'name':name,'startedAt':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'machine':machine,'compiler':{'commit':config['compilerCommit'],'sha256':digest(compiler)},'source':config['source'],'portSource':config['portSource'],'prerequisiteAdjustments':config.get('prerequisiteAdjustments'),'steps':{},'nativeBuilds':[],'lilscriptBuilds':[],'protocol':{'repetitions':3,'cleanOutputs':True,'dependencyInstallationTimedSeparately':True,'sourceArchives':'Pinned Git checkouts; original repository package build commands, followed by production ESM assembly.','workerUse':'Both implementations use the same machine and Node version. This shared build pool also runs unrelated jobs; timings are contextual wall times, with CPU time recorded separately.'}}
def save(): (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
def run(label,command,cwd,timeout=1800):
 print(name,label,flush=True);start=time.monotonic();usage=resource.getrusage(resource.RUSAGE_CHILDREN);log=out/(label+'.log')
 with log.open('w') as handle:
  proc=subprocess.Popen(command,cwd=cwd,env=env,stdout=handle,stderr=subprocess.STDOUT,start_new_session=True)
  try:code=proc.wait(timeout=timeout);timedout=False
  except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);code=proc.wait();timedout=True
 now=resource.getrusage(resource.RUSAGE_CHILDREN);entry={'command':command,'cwd':'upstream' if cwd==upstream else 'port','wallSeconds':time.monotonic()-start,'cpuSeconds':now.ru_utime+now.ru_stime-usage.ru_utime-usage.ru_stime,'exitCode':code,'timedOut':timedout,'log':log.name,'logSha256':digest(log),'tail':log.read_text(errors='replace')[-3500:]};result['steps'][label]=entry;save();return entry
save()
# Each installation is excluded from build time. Upstream lifecycle publication hooks are not invoked.
install=run('original-install',config['install'],upstream,1800)
if install['exitCode']!=0:result['status']='original-install-failed';save();sys.exit(1)
for setup in config.get('setup',[]):
 if run('original-setup-'+str(config['setup'].index(setup)),setup,upstream,1800)['exitCode']:result['status']='original-setup-failed';save();sys.exit(1)
if not (port/'node_modules').exists():
 step=run('lilscript-install',['npm','ci','--no-audit','--no-fund'],port,900)
 if step['exitCode']:
  if 'Missing:' in step['tail']:
   run('lilscript-lock-repair',['npm','install','--package-lock-only','--ignore-scripts','--no-audit','--no-fund'],port,900)
   step=run('lilscript-install-retry',['npm','ci','--no-audit','--no-fund'],port,900)
  if step['exitCode']:result['status']='lilscript-install-failed';save();sys.exit(1)
# Keep exact lockfile bytes for repositories that do not version a lock.
for label,root in [('original',upstream),('lilscript',port)]:
 for lock in ['package-lock.json','pnpm-lock.yaml','yarn.lock']:
  if (root/lock).exists():shutil.copy2(root/lock,out/(label+'-'+lock))
for repetition in range(3):
 # Alternating order reduces systematic warm-up and machine-load bias.
 for lane in (['original','lilscript'] if repetition%2==0 else ['lilscript','original']):
  if lane=='original':
   for path in config.get('clean',[]):
    target=upstream/path
    if target.is_dir():shutil.rmtree(target)
    elif target.exists():target.unlink()
   entry=run('original-build-'+str(repetition+1),config['build'],upstream)
   result['nativeBuilds'].append(entry)
   if entry['exitCode']:result['status']='original-build-failed';save();sys.exit(1)
  else:
   for path in (port/'dist').rglob('*'):
    if path.is_file() and path.suffix in ['.js','.cjs','.mjs','.map']:path.unlink()
   invocation=out/'compiler-invocations.jsonl';invocation.unlink(missing_ok=True)
   entry=run('lilscript-build-'+str(repetition+1),['bash','-c',config['lilscriptBuild']],port,5400)
   entry['invocations']=[json.loads(x) for x in invocation.read_text().splitlines()] if invocation.exists() else []
   result['lilscriptBuilds'].append(entry)
   if entry['exitCode'] or not entry['invocations']:
    result['status']='lilscript-build-failed';save()
    # Finish original source measurements even when the port cannot compile.
    for rest in range(repetition+1,3):
     for path in config.get('clean',[]):
      target=upstream/path
      if target.is_dir():shutil.rmtree(target)
      elif target.exists():target.unlink()
     extra=run('original-build-'+str(rest+1),config['build'],upstream);result['nativeBuilds'].append(extra)
     if extra['exitCode']:break
    break
  save()
 else:continue
 break
if all(x['exitCode']==0 for x in result['nativeBuilds']):
 run('original-esm',['node',str(base/'tools/source-esm.mjs'),str(base),name],upstream,1800)
 if (out/'esm.json').exists():result['esm']=json.loads((out/'esm.json').read_text())
if result['lilscriptBuilds'] and all(x['exitCode']==0 for x in result['lilscriptBuilds']):
 artifacts=[str(p) for p in (port/'dist').rglob('*') if p.is_file() and p.suffix in ['.js','.mjs','.cjs']]
 measured=subprocess.run([str(codec),'--json',*artifacts],capture_output=True,text=True)
 if measured.returncode==0:result['lilscriptArtifacts']=[{**x,'path':str(pathlib.Path(x['path']).relative_to(port)),'sha256':digest(x['path'])} for x in json.loads(measured.stdout)['artifacts']]
 if config.get('test'):run('lilscript-tests',['bash','-c',config['test']],port,1800)
for key in ['nativeBuilds','lilscriptBuilds']:
 values=result[key];ok=[x['wallSeconds'] for x in values if x['exitCode']==0]
 result[key+'Summary']={'samples':len(values),'successful':len(ok),'medianSeconds':statistics.median(ok) if ok else None,'minimumSeconds':min(ok) if ok else None,'maximumSeconds':max(ok) if ok else None}

if result['steps'].get('original-esm',{}).get('exitCode',0):result['status']='original-esm-failed'
if result['steps'].get('lilscript-tests',{}).get('exitCode',0):result['status']='lilscript-tests-failed'
result.setdefault('status','complete');result['finishedAt']=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime());result['machine']['loadAtEnd']=os.getloadavg();save();print(name,result['status'],flush=True)
