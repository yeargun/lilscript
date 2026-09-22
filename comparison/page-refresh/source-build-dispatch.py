#!/usr/bin/env python3
import concurrent.futures,json,os,pathlib,queue,shlex,subprocess,sys
run=pathlib.Path(sys.argv[1]).resolve();tools=pathlib.Path(__file__).resolve().parent;old=pathlib.Path('/tmp/lilscript-page-refresh-20260910');remote='source-perf-20260910';jobs=json.loads((run/'jobs.json').read_text())
ssh=['ssh','-o','BatchMode=yes','-o','StrictHostKeyChecking=accept-new','-o','UserKnownHostsFile='+str(old/'known_hosts'),'-o','ConnectTimeout=15','-o','ServerAliveInterval=30'];os.environ['RSYNC_RSH']=shlex.join(ssh)
def call(args,**kw):return subprocess.run(args,check=True,**kw)
ports=sys.argv[sys.argv.index('--ports')+1].split(',') if '--ports' in sys.argv else list(jobs)
hosts=sys.argv[sys.argv.index('--hosts')+1].split(',') if '--hosts' in sys.argv else ['10.1.0.20','10.1.0.21','10.1.0.22','10.1.0.17','10.1.0.18']
q=queue.Queue();priority=['motionlil','monacolil','jquerylil','remarklil','micromarklil','posthoglil','rehypelil','katexlil','vuelil']
for n in sorted(ports,key=lambda n:priority.index(n) if n in priority else len(priority)):
 if '--retry' in sys.argv or not (run/'results'/n/'result.json').exists():q.put(n)
(run/'logs').mkdir(exist_ok=True)
def worker(ip):
 host='lilfarm@'+ip
 call(ssh+[host,'mkdir -p '+remote+'/tools '+remote+'/ports '+remote+'/upstream '+remote+'/lilscript/target/release'])
 call(['rsync','-a',str(tools)+'/',host+':'+remote+'/tools/'],stdout=subprocess.DEVNULL)
 call(['rsync','-a',str(run/'jobs.json'),host+':'+remote+'/'])
 call(['rsync','-a',str(run/'compiler/target/release/lilscript'),str(run/'compiler/target/release/lilscript-codec'),host+':'+remote+'/tools/'])
 call(['rsync','-a','--exclude=.git','--exclude=target',str(run/'compiler')+'/',host+':'+remote+'/lilscript/'],stdout=subprocess.DEVNULL)
 call(ssh+[host,'chmod +x '+remote+'/tools/compiler-wrapper.py; cp '+remote+'/tools/lilscript '+remote+'/tools/lilscript-codec '+remote+'/lilscript/target/release/; npm install --prefix '+remote+'/toolchain node@24.11.1 corepack@0.34.5 esbuild@0.28.1 terser@5.51.2 --no-audit --no-fund >/dev/null 2>&1'])
 # Some port source graphs import their sibling source files. Synchronize only isolated snapshots.
 for n in jobs:
  call(['rsync','-a','--exclude=.git','--exclude=node_modules','--exclude=_site','--exclude=.page-audit','--exclude=target',str(pathlib.Path(jobs[n]['portSource']['snapshot']))+'/',host+':'+remote+'/ports/'+n+'/'],stdout=subprocess.DEVNULL)
 call(ssh+[host,'ln -sfn ../lilscript '+remote+'/ports/lilscript'])
 while True:
  try:n=q.get_nowait()
  except queue.Empty:return
  try:
   print('START',n,ip,flush=True)
   call(['rsync','-a','--exclude=node_modules',str(run/'upstream'/n)+'/',host+':'+remote+'/upstream/'+n+'/'],stdout=subprocess.DEVNULL)
   with (run/'logs'/(n+'.log')).open('w') as log:
    p=subprocess.run(ssh+[host,'python3 '+remote+'/tools/source-build-worker.py '+remote+' '+n],stdout=log,stderr=subprocess.STDOUT)
   (run/'results'/n).mkdir(parents=True,exist_ok=True)
   call(['rsync','-a',host+':'+remote+'/results/'+n+'/',str(run/'results'/n)+'/'])
   (run/'artifacts'/n).mkdir(parents=True,exist_ok=True)
   call(['rsync','-a',host+':'+remote+'/ports/'+n+'/dist/',str(run/'artifacts'/n)+'/'])
   result=json.loads((run/'results'/n/'result.json').read_text());result['workerIp']=ip;(run/'results'/n/'result.json').write_text(json.dumps(result,indent=2)+'\n')
   print('DONE',n,result.get('status'),flush=True)
  except Exception as e:print('ERROR',n,str(e),flush=True)
  finally:q.task_done()
with concurrent.futures.ThreadPoolExecutor(max_workers=len(hosts)) as pool:
 for f in concurrent.futures.as_completed([pool.submit(worker,ip) for ip in hosts]):
  try:f.result()
  except Exception as e:print('WORKER ERROR',str(e),flush=True)
