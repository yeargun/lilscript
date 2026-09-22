import concurrent.futures,json,os,pathlib,subprocess,sys
old=pathlib.Path('/tmp/lilscript-page-refresh-20260910');run=pathlib.Path('/tmp/lilscript-source-performance-20260910');pub=json.loads((old/'publications.json').read_text());names=sys.argv[1].split(',') if len(sys.argv)>1 else list(pub)
env={**os.environ,'PATH':'/home/azureuser/.nvm/versions/node/v24.11.1/bin:'+os.environ['PATH']}
def check(n):
 root=pathlib.Path(pub[n]['path']);pkg=json.loads((root/'package.json').read_text());script='check:pages' if n=='vuelil' else 'check:site'
 if script not in pkg.get('scripts',{}):script='build:pages' if n=='vuelil' else 'build:site'
 r=subprocess.run(['npm','run',script],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=900)
 (run/'logs'/(n+'-page-check.log')).write_text(r.stdout)
 print(n,'OK' if r.returncode==0 else 'FAIL '+r.stdout[-1800:],flush=True)
 return {'name':n,'exitCode':r.returncode,'command':['npm','run',script]}
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:results=list(pool.map(check,names))
(run/'page-checks.json').write_text(json.dumps(results,indent=2)+'\n')
