import concurrent.futures,fcntl,hashlib,json,os,pathlib,subprocess,sys
run=pathlib.Path('/tmp/lilscript-source-performance-20260910');old=pathlib.Path('/tmp/lilscript-page-refresh-20260910');pub=json.loads((old/'publications.json').read_text());names=sys.argv[1].split(',') if len(sys.argv)>1 else [n for n in pub if n!='motionlil']
def command(args,root):
 p=subprocess.run(args,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=600)
 if p.returncode:raise RuntimeError(' '.join(args)+'\n'+p.stdout[-3500:])
 return p.stdout.strip()
def save(n,v):
 with (old/'publications.lock').open('w') as f:
  fcntl.flock(f,fcntl.LOCK_EX);p=old/'publications.json';j=json.loads(p.read_text());j[n]=v;p.write_text(json.dumps(j,indent=2)+'\n')
def runone(n):
 r=pathlib.Path(pub[n]['path']);data=json.loads((r/('web' if n=='vuelil' else 'site')/'comparison.json').read_text());prep=json.loads((run/'prepared'/f'{n}.json').read_text())
 for path,digest in prep['css'].items():
  assert hashlib.sha256((r/path).read_bytes()).hexdigest()==digest, 'Styles changed: '+n
 # Include the replay configuration, raw build logs and exact dependency locks.
 records=r/'comparison/source-build';jobs=json.loads((run/'jobs.json').read_text());job=jobs[n];job.pop('publication',None);job['portSource'].pop('snapshot',None)
 (records/'job.json').write_text(json.dumps(job,indent=2)+'\n')
 text=f'''# Source-build measurements\n\nMeasured {data['measuredAt']} using LilScript `{data['compiler']['commit']}` and the upstream Git revision recorded in `job.json`.\n\n`result.json` records the commands, wall time, CPU time, machine and exit codes. `esm.json` records the production ESM assembly and exact input graph. The lockfiles record dependency resolution. The public page uses `source-build.json` for the final consolidated record.\n\nRun the installation and setup commands from `job.json` in the corresponding pinned upstream checkout; they are excluded from build time. Run the recorded build command with Node {data['machine']['node']}. Clear the listed generated output directories between repetitions. Install the port dependencies and set `LILSCRIPT_COMPILER`, `MOTIONLIL_LILSCRIPT_BIN`, `SOLIDLIL_LILSCRIPT_BIN`, `LILSCRIPT_ROOT` and `LILSCRIPT_CODEC` to the recorded compiler and codec as applicable. Some ports import sibling LilScript source trees.\n\nThe original repository build and comparison ESM assembly are measured separately. Build output scope can differ between repositories; no build speedup is inferred. Both lanes used the same shared Azure worker.\n'''
 (records/'README.md').write_text(text)
 command(['git','add','-A'],r)
 if command(['git','status','--porcelain'],r):command(['git','commit','-m','Record paired source builds and refresh current ESM comparisons'],r)
 commit=command(['git','rev-parse','HEAD'],r)
 command(['git','fetch','origin','main'],r)
 # Refuse to overwrite concurrent remote changes.
 command(['git','merge-base','--is-ancestor','origin/main','HEAD'],r)
 command(['git','push','origin','HEAD:main'],r)
 v=pub[n];v.update(commit=commit,status='pushed',deployment='pending',pageChecks='passed',presentation='informational',sourceBuilds=True);save(n,v);print('PUSHED',n,commit[:10],flush=True)
def safe(n):
 try:runone(n)
 except Exception as e:print('ERROR',n,str(e),flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as p:list(p.map(safe,names))
