#!/usr/bin/env python3
"""Frozen Q3 policy comparison, complete artifact oracles and isolated costs.
The prior is already frozen from C3 training. Existing evaluation inputs are
reused as exposed evaluation, never represented as new blind data.
"""
import hashlib, json, os, resource, subprocess, sys, time
from pathlib import Path
here=Path(__file__).resolve().parent
repo=here.parents[2]
binary=Path(sys.argv[1]).resolve()
codec=Path('/home/azureuser/lilscript-work/bin/q2-print-1/lilscript-codec')
if not codec.exists(): codec=Path('/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec')
manifest_path=repo/'benchmarks/calibration/corpus/manifest.json'
manifest=json.loads(manifest_path.read_text())
sha=lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
env={k:v for k,v in os.environ.items() if not k.startswith('LILSCRIPT_') and k not in ('NODE_OPTIONS','RAYON_NUM_THREADS')}
env.update(TZ='UTC',LC_ALL='C')
variants={'protected12': (12,''), 'ordinary13': (13,"protect_effort=false\nobjective_prior='off'\n"), 'protected13': (13,''), 'protected14': (14,'')}
work=Path('/home/azureuser/lilscript-work/q3-search-qualification')
rows=[]
result={'compiler':{'path':str(binary),'sha256':sha(binary)},'codec':{'path':str(codec),'sha256':sha(codec)},
        'prior_sha256':sha(here/'objective-prior.json'),'manifest_sha256':sha(manifest_path),
        'scope':'Four frozen generic programs; evaluation families are already exposed and excluded from fitting. Single CPU observations, not speedup estimates.',
        'rows':rows,'complete':False}
(here/'qualification-inputs.json').write_text(json.dumps({**result,'variants':variants,'ids':['arithmetic-128','checksum-library','distance-library','interval-library']},indent=2)+'\n')
def run(args):
 before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic()
 p=subprocess.run(args,cwd=repo,env=env,text=True,capture_output=True,timeout=300)
 after=resource.getrusage(resource.RUSAGE_CHILDREN)
 return p, {'cpu_seconds':after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,'wall_seconds':time.monotonic()-start}
def save(): (here/'artifacts.json').write_text(json.dumps(result,indent=2)+'\n')
for id in ['arithmetic-128','checksum-library','distance-library','interval-library']:
 item=next(row for row in manifest['workloads'] if row['id']==id)
 entry=manifest_path.parent/item['entry'];assert sha(entry)==item['source_sha256']
 for objective,metric in [('raw','raw'),('gzip','gzip9'),('brotli','brotli11')]:
  row={'id':id,'split':item['split'],'objective':objective,'source_sha256':sha(entry),'variants':{}}
  rows.append(row)
  for variant,(level,search) in variants.items():
   folder=work/id/objective/variant;folder.mkdir(parents=True,exist_ok=True)
   config=folder/'config.toml';out=folder/'out.mjs'
   config.write_text(f"objective.codecs='{objective}'\n[policy]\nversion=3\n[effort]\nlevel={level}\n[policy.search]\n{search}")
   p,cost=run([str(binary),str(entry),'--target','js-module','--format','esm','--config',str(config),'-o',str(out),'--explain','json'])
   (folder/'explain.json').write_text(p.stderr)
   own={'config_sha256':sha(config),'exit_code':p.returncode,**cost};row['variants'][variant]=own;save()
   if p.returncode: raise RuntimeError(p.stderr[-2000:])
   report=json.JSONDecoder().raw_decode(p.stderr[p.stderr.index('{'):])[0]
   script="const m=await import(process.argv[1]);console.log(JSON.stringify("+json.dumps(item['inputs'])+".map(x=>m.probe(x))));"
   oracle,_=run(['node','--input-type=module','-e',script,out.as_uri()])
   assert oracle.returncode==0 and json.loads(oracle.stdout)==item['expected'],oracle.stderr+oracle.stdout
   sizes,_=run([str(codec),'--json',str(out)]);assert sizes.returncode==0
   sizes=json.loads(sizes.stdout)['artifacts'][0]
   terminal=report['search']['terminal'];stage=terminal['objectives'][0]
   assert stage['after']==sizes[metric]
   own.update({'artifact':str(out),'explanation':{'path':str(folder/'explain.json'),'sha256':sha(folder/'explain.json')},'sha256':sha(out),'bytes':sizes[metric], 'raw':sizes['raw'],'gzip9':sizes['gzip9'],'brotli11':sizes['brotli11'],
               'resources':report['resources'],'search':{k:report['search'][k] for k in ['proposals','renders','codec_probes','stop']},
               'judged':stage['judged'],'recalled':sum(t['outcome']=='recalled' for key in ['trials','choice_trials','joint_trials','starts'] for t in stage[key]),
               'stops':stage['stops'],'checkpoints':terminal.get('effort_checkpoints',[]),'effort_refusal':terminal.get('effort_refusal'),
               'oracle':True})
   save();print(id,objective,variant,own['bytes'],f"{cost['cpu_seconds']:.3f}s",flush=True)
  assert row['variants']['protected14']['bytes']<=row['variants']['protected13']['bytes']<=row['variants']['protected12']['bytes']
  for variant in ['protected13','protected14']:
   for checkpoint in row['variants'][variant]['checkpoints']:
    previous=row['variants'][f"protected{checkpoint['level']}"]
    _,size,digest=checkpoint['objectives'][0]
    assert (size,digest)==(previous['bytes'],previous['sha256'])
   stop=next(stop for stop in row['variants'][variant]['stops'] if stop['level']==12)
   previous=row['variants']['protected12']
   assert (stop['size'],stop['sha256'])==(previous['bytes'],previous['sha256'])
result['totals']={objective:{variant:sum(row['variants'][variant]['bytes'] for row in rows if row['objective']==objective) for variant in variants} for objective in ['raw','gzip','brotli']}
result['complete']=True;save();print(json.dumps(result['totals']),flush=True)
