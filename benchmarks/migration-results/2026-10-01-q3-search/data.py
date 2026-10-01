#!/usr/bin/env python3
"""Affected public data build, with independent literal-observation oracle."""
import hashlib,json,os,resource,subprocess,sys,time
from pathlib import Path
here=Path(__file__).resolve().parent;repo=here.parents[2]
binaries={'before':'/home/azureuser/lilscript-work/bin/q2-print-1/lilscript','after':str(Path(sys.argv[1]).resolve())}
codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec'
work=Path('/home/azureuser/lilscript-work/q3-data-qualification');work.mkdir(parents=True,exist_ok=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
env={k:v for k,v in os.environ.items() if not k.startswith('LILSCRIPT_') and k!='NODE_OPTIONS'}
rows=[]
report={'compilers':{name:{'path':path,'sha256':sha(path)} for name,path in binaries.items()},'source_sha256':sha(repo/'tests/cases/data_tables.lil'),
 'oracle_sha256':sha(repo/'tests/cases/data_tables.out'),'scope':'One affected 22KB generic data fixture, three objective lanes. Before is algorithm62, so delta also includes algorithm63 accounting; single CPU observations only.', 'rows':rows,'complete':False}
for objective,metric in [('raw','raw'),('gzip','gzip9'),('brotli','brotli11')]:
 row={'objective':objective};rows.append(row)
 for name,binary in binaries.items():
  config=work/f'{name}-{objective}.toml';out=work/f'{name}-{objective}.mjs';explanation=work/f'{name}-{objective}.json'
  config.write_text(f"objective.codecs='{objective}'\neffort.level=13\n"+(repo/'tests/cases/data_tables.toml').read_text())
  before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic()
  result=subprocess.run([binary,str(repo/'tests/cases/data_tables.lil'),'--config',str(config),'--target','js-module','--format','esm','-o',str(out),'--explain','json'],text=True,capture_output=True,env=env,timeout=300)
  after=resource.getrusage(resource.RUSAGE_CHILDREN);explanation.write_text(result.stderr)
  own={'exit_code':result.returncode,'wall_seconds':time.monotonic()-start,'cpu_seconds':after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime};row[name]=own
  assert result.returncode==0,result.stderr[-1500:]
  build=json.JSONDecoder().raw_decode(result.stderr[result.stderr.index('{'):])[0]
  script=(repo/'tests/cases/data_tables.host.js').read_text()+f'\nawait import({json.dumps(out.as_uri())});'
  oracle=subprocess.run(['node','--input-type=module','-e',script],text=True,capture_output=True,env=env,timeout=20)
  assert oracle.returncode==0 and oracle.stdout==(repo/'tests/cases/data_tables.out').read_text(),oracle.stderr+oracle.stdout
  sizes=json.loads(subprocess.check_output([codec,'--json',str(out)],text=True))['artifacts'][0]
  stage=build['search']['terminal']['objectives'][0];assert stage['after']==sizes[metric]
  own.update(bytes=sizes[metric],sha256=sha(out),artifact=str(out),explanation={'path':str(explanation),'sha256':sha(explanation)},resources=build['resources'],choices=stage['choices'],judged=stage['judged'],checkpoints=build['search']['terminal'].get('effort_checkpoints',[]),oracle=True)
  (here/'data.json').write_text(json.dumps(report,indent=2)+'\n');print(objective,name,own['bytes'],round(own['cpu_seconds'],3),flush=True)
report['complete']=True
(here/'data.json').write_text(json.dumps(report,indent=2)+'\n')
