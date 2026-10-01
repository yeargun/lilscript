#!/usr/bin/env python3
"""Independent D1 format consumers and canonical measurement of actual files."""
import hashlib,json,os,resource,subprocess,sys,time
from pathlib import Path
here=Path(__file__).resolve().parent;repo=here.parents[2]
binary=Path(sys.argv[1]).resolve();codec=Path('/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec')
node='/home/azureuser/.nvm/versions/node/v20.19.0/bin/node'
work=Path('/home/azureuser/lilscript-work/d1-formats-qualification');work.mkdir(exist_ok=True)
fixtures=here/'fixtures';fixtures.mkdir(exist_ok=True)
(fixtures/'library.lil').write_text('export int count=3;export const Record<string> words=record{negative:"minus",zero:"zero",positive:"plus"};export int step(int n){count+=n;return count;}export {step as inc};export string classify(int n){if(n<0){return words["negative"]??"missing";}if(n==0){return words["zero"]??"missing";}return words["positive"]??"missing";}\n')
(fixtures/'application.lil').write_text('int n=3;for(int i=0;i<4;i++){n+=i;}print(n);string text="😀";print(text.length);\n')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
env={k:v for k,v in os.environ.items() if not k.startswith('LILSCRIPT_') and k!='NODE_OPTIONS'}
report={'algorithm':66,'compiler':{'path':str(binary),'sha256':sha(binary)},'node':{'path':node,'sha256':sha(node)},'codec':{'path':str(codec),'sha256':sha(codec)},'fixtures':{p.name:sha(p) for p in fixtures.glob('*.lil')},'rows':[],'complete':False}
if (here/'artifacts.json').exists():
 previous=json.loads((here/'artifacts.json').read_text())
 if previous['compiler']==report['compiler'] and previous['fixtures']==report['fixtures']:
  report['rows']=[row for row in previous['rows'] if row.get('oracle')]
def save():(here/'artifacts.json').write_text(json.dumps(report,indent=2)+'\n')
def run(args):
 before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();p=subprocess.run([str(a) for a in args],cwd=repo,env=env,text=True,capture_output=True,timeout=300);after=resource.getrusage(resource.RUSAGE_CHILDREN)
 return p,{'wall_seconds':time.monotonic()-start,'cpu_seconds':after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime}
def qualify(format,objective,application=False):
 name=('application-' if application else 'library-')+format
 if any(row['fixture']==name and row['objective']==objective for row in report['rows']):return
 folder=work/name/objective;folder.mkdir(parents=True,exist_ok=True)
 config=folder/'config.toml';config.write_text(f"objective.codecs='{objective}'\neffort.level=13\n[policy]\nversion=3\n[target.javascript]\nformat='{format}'\n[delivery]\nentry_names='main.[ext]'\nglobal='Library'\n")
 source=fixtures/('application.lil' if application else 'library.lil')
 p,cost=run([binary,source,'--config',config,'--target','js' if application else 'js-module','--out-dir',folder,'--explain','json'])
 (folder/'explain.json').write_text(p.stderr)
 row={'fixture':name,'objective':objective,'config_sha256':sha(config),'exit_code':p.returncode,**cost};report['rows'].append(row);save();assert p.returncode==0,p.stderr[-3500:]
 build=json.JSONDecoder().raw_decode(p.stderr[p.stderr.index('{'):])[0]
 manifest=json.loads((folder/'lilscript.manifest.json').read_text());output=manifest['outputs'][0];entry=folder/output['entries'][0]['file'];code=entry.read_text()
 if application:
  script=code;expected='9\n2\n'
 else:
  oracle="if(m.count!==3||m.inc!==m.step||m.step(4)!==7||m.count!==7||m.inc(-10)!==-3||m.count!==-3)throw Error('live surface');if([-9,0,11].map(m.classify).join(',')!=='minus,zero,plus')throw Error('behavior');if(Object.getPrototypeOf(m.words)!==null||!Object.isFrozen(m.words))throw Error('const boundary');console.log('ok');"
  if format=='esm':script=f"const m=await import({json.dumps(entry.as_uri())});"+oracle
  elif format=='cjs':script=f"import{{createRequire}}from'node:module';const m=createRequire(import.meta.url)({json.dumps(str(entry))});"+oracle
  else:
   script=f"import vm from'node:vm';const context={{}};vm.runInNewContext({json.dumps(code)},context);const m=context.Library;"+oracle
  expected='ok\n'
 p,_=run([node,entry] if application else [node,'--input-type=module','-e',script]);assert p.returncode==0 and p.stdout==expected,(p.stderr,p.stdout,code)
 if format=='umd':
  for branch in ['cjs','amd']:
   if branch=='cjs':setup="const context={exports:{},require:()=>{throw Error('unexpected dependency')}};context.module={exports:context.exports};";surface='context.exports'
   else:setup="let result;const define=(deps,f)=>{result=f({})};define.amd={};const context={define};";surface='result'
   p,_=run([node,'--input-type=module','-e',f"import vm from'node:vm';{setup}vm.runInNewContext({json.dumps(code)},context);const m={surface};"+oracle]);assert p.returncode==0 and p.stdout=='ok\n',p.stderr
 metric={'raw':'raw','gzip':'gzip9','brotli':'brotli11'}[objective];files=[]
 for listed in output['files']:
  file=folder/listed['file'];data=file.read_bytes();assert len(data)==listed['bytes'] and sha(file)==listed['sha256']
  measured=json.loads(subprocess.check_output([str(codec),'--json',str(file)],env=env,text=True))['artifacts'][0];assert measured[metric]==listed['codec_bytes']
  files.append({'name':listed['file'],'sha256':sha(file),'raw':measured['raw'],'gzip9':measured['gzip9'],'brotli11':measured['brotli11']})
 stage=build['search']['terminal']['objectives'][0];assert stage['after']==output['rows_total'];assert output['codec_total']==sum(file[metric] for file in files)
 row.update(files=files,bytes=stage['after'],judged=stage['judged'],oracle=True,manifest_sha256=sha(folder/'lilscript.manifest.json'),explanation_sha256=sha(folder/'explain.json'));save();print(name,objective,row['bytes'],round(row['cpu_seconds'],3),flush=True)
for objective in ['raw','gzip','brotli']:
 for format in ['esm','cjs','iife','umd']:qualify(format,objective)
 for format in ['iife','bare','cjs']:qualify(format,objective,True)
report['complete']=True;save()
