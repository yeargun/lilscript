#!/usr/bin/env python3
"""Q4 production artifacts, typed configuration and independent observations."""
import hashlib,json,os,resource,subprocess,sys,time
from pathlib import Path
here=Path(__file__).resolve().parent;repo=here.parents[2]
binary=Path(sys.argv[1]).resolve();before_binary=Path('/home/azureuser/lilscript-work/bin/q3-search-1/lilscript')
codec=Path('/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec')
work=Path('/home/azureuser/lilscript-work/q4-data-qualification/builds');work.mkdir(parents=True,exist_ok=True)
fixtures=here/'fixtures';fixtures.mkdir(exist_ok=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
env={k:v for k,v in os.environ.items() if not k.startswith('LILSCRIPT_') and k!='NODE_OPTIONS'}
def table(prefix,count=96): return 'record{'+','.join(f'key{i:03}:"{prefix}-group{i%12}-repeated-payload-repeated-payload"' for i in range(count))+'}'
(fixtures/'public.lil').write_text('export const Record<string> data='+table('public')+';export string probe(string key){return data[key]??"missing";}\n')
(fixtures/'modules.lil').write_text('import {left} from "./left";import {right} from "./right";export string probe(string key){return left(key)+right(key);}\n')
for name in ['left','right']:
 (fixtures/f'{name}.lil').write_text('const Record<string> data='+table(name)+';export string '+name+'(string key){return data[key]??"missing";}\n')
(fixtures/'schema.lil').write_text('const int[] schema=[3,7];int validate(int[] shape,int n){if(shape[0]==3){return n+shape[1];}return n-1;}export int probe(int n){return validate(schema,n)+validate(schema,n+1);}\n')
(fixtures/'native.lil').write_text('define int SIZE=5;const int square(int n){int result=0;for(int i=0;i<n;i++){result+=n;}return result;}const int answer=square(SIZE);int[] make(){int[] data=[];for(int i=0;i<5;i++){data.push(i*i);}return data;}const int[] data=make();print(answer);print(data[3]);\n')
report={'algorithm':65,'compiler':{'path':str(binary),'sha256':sha(binary)},'previous':{'path':str(before_binary),'sha256':sha(before_binary)},'codec_sha256':sha(codec),'fixtures':{p.name:sha(p) for p in fixtures.glob('*.lil')},'rows':[],'complete':False}
def save(): (here/'artifacts.json').write_text(json.dumps(report,indent=2)+'\n')
def run(args):
 start=time.monotonic();before=resource.getrusage(resource.RUSAGE_CHILDREN)
 p=subprocess.run([str(a) for a in args],cwd=repo,env=env,text=True,capture_output=True,timeout=300)
 after=resource.getrusage(resource.RUSAGE_CHILDREN)
 return p,{'wall_seconds':time.monotonic()-start,'cpu_seconds':after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime}
def artifact(name,objective,variant,entry,config_text,compiler=binary):
 folder=work/name/objective/variant;folder.mkdir(parents=True,exist_ok=True)
 (folder/'package.json').write_text('{"type":"module"}\n')
 config=folder/'config.toml';config.write_text(f"objective.codecs='{objective}'\neffort.level=13\n"+config_text)
 out=folder/'main.mjs'
 p,cost=run([compiler,entry,'--config',config,'--target','js-module','--format','esm','-o',out,'--explain','json'])
 (folder/'explain.json').write_text(p.stderr)
 row={'name':name,'objective':objective,'variant':variant,'exit_code':p.returncode,'config_sha256':sha(config),**cost};report['rows'].append(row);save()
 assert p.returncode==0,p.stderr[-3500:]
 build=json.JSONDecoder().raw_decode(p.stderr[p.stderr.index('{'):])[0]
 if name=='existing-data':
  script=(repo/'tests/cases/data_tables.host.js').read_text()+f'\nawait import({json.dumps(out.as_uri())});'
  expected=(repo/'tests/cases/data_tables.out').read_text()
 else:
  prelude=f'const m=await import({json.dumps(out.as_uri())});'
  if name=='schema':
   script=prelude+"const inputs=[-7,0,1,1024,2147483647];for(const n of inputs)if(m.probe(n)!==((n*2+15)|0))throw Error(n);console.log('ok');"
  else:
   prefixes=['public'] if name=='public' else ['left','right']
   script=prelude+f"for(let i=0;i<96;i++){{const expected={json.dumps(prefixes)}.map(p=>p+'-group'+i%12+'-repeated-payload-repeated-payload').join('');if(m.probe('key'+String(i).padStart(3,'0'))!==expected)throw Error(i)}}"
   if name=='public': script+="if(Object.keys(m.data).length!==96||Object.getPrototypeOf(m.data)!==null||!Object.isFrozen(m.data))throw Error('boundary');let refused=false;try{m.data.key000='changed'}catch(e){refused=true}if(!refused)throw Error('mutable');"
   script+="console.log('ok');"
  expected='ok\n'
 oracle,_=run(['node','--input-type=module','-e',script]);assert oracle.returncode==0 and oracle.stdout==expected,oracle.stderr+oracle.stdout
 paths=sorted({*folder.glob('*.mjs'),*folder.glob('*.js')});metric={'raw':'raw','gzip':'gzip9','brotli':'brotli11'}[objective]
 files=[]
 for file in paths:
  sizes=json.loads(subprocess.check_output([str(codec),'--json',str(file)],env=env,text=True))['artifacts'][0]
  files.append({'name':file.name,'sha256':sha(file),'raw':sizes['raw'],'gzip9':sizes['gzip9'],'brotli11':sizes['brotli11']})
 stage=build['search']['terminal']['objectives'][0]
 assert stage['after']==sum(file[metric] for file in files),(stage['after'],files)
 row.update(files=files,bytes=stage['after'],choices=stage['choices'],judged=stage['judged'],oracle=True,resources=build['resources'],explanation={'path':str(folder/'explain.json'),'sha256':sha(folder/'explain.json')});save();print(name,objective,variant,row['bytes'],round(row['cpu_seconds'],3),flush=True)
for objective in ['raw','gzip','brotli']:
 for name in ['public','modules']:
  for variant in ['literal','eager','lazy']:
   startup='off' if variant=='literal' else 'on';lazy='on' if variant=='lazy' else 'off'
   config=f"[policy]\nversion=3\n[javascript]\nassume_pristine_builtins=true\n[policy.tactics]\nstartup-reconstruction='{startup}'\nrecurring-reconstruction='{lazy}'\n"
   if name=='modules': config+="[delivery]\nmode='preserve-modules'\n"
   artifact(name,objective,variant,fixtures/f'{name}.lil',config)
 for variant,fold in [('runtime','off'),('specialized','on')]:
  artifact('schema',objective,variant,fixtures/'schema.lil',f"[policy]\nversion=3\n[policy.tactics]\nconstant-folding='{fold}'\ninlining='off'\n")
 for variant,compiler in [('before',before_binary),('after',binary)]:
  artifact('existing-data',objective,variant,repo/'tests/cases/data_tables.lil',(repo/'tests/cases/data_tables.toml').read_text(),compiler)
# Required const/define lowering is shared with native, even under a folding veto.
native=work/'native';native.mkdir(exist_ok=True)
config=native/'config.toml';config.write_text("effort.level=0\n[defines]\nSIZE=8\n[policy.tactics]\nconstant-folding='off'\n")
c=native/'data.c';p,_=run([binary,fixtures/'native.lil','--config',config,'--target','c','-o',c]);assert p.returncode==0,p.stderr
report['native']={'c_sha256':sha(c),'config_sha256':sha(config),'compilers':[]}
for compiler in ['gcc','/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18']:
 exe=native/('const-'+Path(compiler).name);p,_=run([compiler,'-std=c11','-O2',c,'-lm','-o',exe]);assert p.returncode==0,p.stderr
 output,_=run([exe]);assert output.returncode==0 and output.stdout=='64\n9\n',output.stderr+output.stdout
 report['native']['compilers'].append({'compiler':compiler,'sha256':sha(exe),'oracle':True})
report['complete']=True;save()
