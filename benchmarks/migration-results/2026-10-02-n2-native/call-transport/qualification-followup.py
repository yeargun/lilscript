from pathlib import Path
import os,json,hashlib,subprocess,time
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-call-transport-final');compiler='/home/azureuser/lilscript-work/bin/n2-call-transport-3/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node'
rows=[]
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def run(name,args):
 start=time.monotonic();r=subprocess.run(args,capture_output=True,timeout=180);(work/(name+'.stdout')).write_bytes(r.stdout);(work/(name+'.log')).write_bytes(r.stderr)
 rows.append({'name':name,'args':args,'exit':r.returncode,'elapsed_ms':round((time.monotonic()-start)*1000),'passed':r.returncode==0});print(name,rows[-1]['passed'],flush=True)
 if r.returncode:raise SystemExit(r.returncode)
 return r.stdout
source=Path('tests/native/call-transport.lil').read_text();begin=source.index('void replace(');end=source.index('int wide(');source=source[:begin]+source[end:];source=source.replace('    references();\n','')
subset=work/'defaults.lil';subset.write_text(source)
oracle=Path('tests/native/call-transport.oracle.mjs').read_text();begin=oracle.index('const text={');end=oracle.index('for(let i=0;i<300;');oracle=oracle[:begin]+oracle[end:];(work/'defaults.oracle.mjs').write_text(oracle)
try:
 expected=run('defaults-oracle',[node,str(work/'defaults.oracle.mjs')]);(work/'defaults.out').write_bytes(expected)
 for profile in ['formation','production']:
  config=Path('tests/native/call-transport.toml')
  if profile=='production':
   config=work/'production.toml';config.write_text(Path('tests/native/call-transport.toml').read_text().replace('level=0','level=13').replace('="off"','="auto"'))
  target=work/(profile+'-defaults.mjs')
  run(profile+'-defaults-js-build',[compiler,str(subset),'--config',str(config),'--target','js-module','-o',str(target)])
  expression='globalThis.assertNativeCount=()=>{};globalThis.assertMaxNativeCount=()=>{};globalThis.collectNative=()=>{};await import('+json.dumps(str(target))+');'
  actual=run(profile+'-defaults-js-run',[node,'--input-type=module','-e',expression]);assert actual==expected
 config=work/'production.toml';target=work/'production.c'
 run('production-c-build',[compiler,'tests/native/call-transport.lil','--config',str(config),'--target','c','-o',str(target)])
 host=work/'production-host.c';host.write_text(Path('tests/native/call-transport.host.c').read_text().replace('ownership.h','production.h'))
 run('production-gcc-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',str(target),str(host),'-lm','-o',str(work/'production')])
 assert run('production-gcc-run',[str(work/'production')])==Path('tests/native/call-transport.out').read_bytes()
finally:
 (work/'followup.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':digest(compiler)},'rows':rows,'full_source':digest('tests/native/call-transport.lil'),'subset':digest(subset),'subset_oracle':digest(work/'defaults.oracle.mjs')},indent=2)+'\n')
