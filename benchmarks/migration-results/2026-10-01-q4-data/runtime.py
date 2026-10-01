#!/usr/bin/env python3
"""Forced legal representation micro-study; no policy tuning or parity claim."""
import hashlib,importlib.util,json,os,statistics,subprocess,sys,shutil
from pathlib import Path
here=Path(__file__).resolve().parent;repo=here.parents[2]
forms=Path(sys.argv[1]).resolve()
codec=Path('/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec')
node=Path(shutil.which('node')).resolve()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
spec=importlib.util.spec_from_file_location('runtime_cost',repo/'scripts/runtime-cost.py');analysis=importlib.util.module_from_spec(spec);spec.loader.exec_module(analysis)
artifacts=sorted(forms.glob('*.mjs'));baseline=forms/'0-literal.mjs'
env={'PATH':os.defpath,'TZ':'UTC','LANG':'C','LC_ALL':'C'}
report={'scope':'512-entry generic immutable dictionary, seven forced legal forms. Fresh Node processes; 5 alternating pairs per representation, 3 baseline/baseline controls. Source-generated target forms use the production decoder and printer. Not a default-policy winner or fleet claim.', 'engine':{'path':str(node),'sha256':sha(node)},'runner_sha256':sha(__file__),'worker_sha256':sha(here/'runtime-worker.mjs'),'codec_sha256':sha(codec),'artifacts':{},'controls':[],'pairs':{},'complete':False}
def save(): (here/'runtime.json').write_text(json.dumps(report,indent=2)+'\n')
def run(path):
 return json.loads(subprocess.check_output([str(node),'--expose-gc',str(here/'runtime-worker.mjs'),str(path)],env=env,text=True,timeout=30))
for path in artifacts:
 measured=json.loads(subprocess.check_output([str(codec),'--json',str(path)],env=env,text=True))['artifacts'][0]
 report['artifacts'][path.name]={'sha256':sha(path),'path':str(path),'sizes':measured}
for pair in range(3): report['controls'].append([run(baseline),run(baseline)]);save()
for path in artifacts[1:]:
 pairs=[];report['pairs'][path.name]=pairs
 for pair in range(5):
  order=[('reference',baseline),('candidate',path)]
  if pair%2:order.reverse()
  own={label:run(file) for label,file in order};pairs.append(own);save()
 print(path.name,'qualified',flush=True)
report['summary']={}
for name,pairs in report['pairs'].items():
 own={};report['summary'][name]=own
 for metric in ['startup_wall_ns','startup_cpu_us','first_access_wall_ns','steady_wall_ns','steady_cpu_us','rss_bytes']:
  ratios=[p['candidate']['metrics'][metric]/p['reference']['metrics'][metric] for p in pairs]
  controls=[[p[1]['metrics'][metric]/p[0]['metrics'][metric] for p in report['controls']]]
  own[metric]=analysis.judge_ratios(ratios,controls,.10)
 for metric in ['unused_heap_bytes','first_access_heap_bytes']:
  own[metric]={label:analysis.describe([p[label]['metrics'][metric] for p in pairs]) for label in ['reference','candidate']}
report['complete']=True;save()
print(json.dumps({name:{metric:row[metric]['verdict'] for metric in ['startup_wall_ns','steady_wall_ns']} for name,row in report['summary'].items()}),flush=True)
