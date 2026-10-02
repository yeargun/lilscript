from pathlib import Path
import os,json,hashlib,subprocess,time
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-call-transport-final');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-call-transport-3/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node'
env={**os.environ,'LILSCRIPT_NATIVE_CLANG':'/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'}
rows=[]
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def run(name,args):
 start=time.monotonic();r=subprocess.run(args,capture_output=True,env=env,timeout=180);(work/(name+'.stdout')).write_bytes(r.stdout);(work/(name+'.log')).write_bytes(r.stderr)
 rows.append({'name':name,'args':args,'exit':r.returncode,'elapsed_ms':round((time.monotonic()-start)*1000),'passed':r.returncode==0});print(name,rows[-1]['passed'],flush=True)
 if r.returncode:raise SystemExit(r.returncode)
 return r.stdout
try:
 for fixture in ['products','exceptions']:
  run(fixture,['node','tests/native/qualify-ownership.mjs',compiler,str(work/fixture),'--generated-only','--fixture=tests/native/'+fixture])
 for profile in ['formation','production']:
  config=Path('tests/native/call-transport.toml')
  if profile=='production':
   config=work/'production.toml';config.write_text(Path('tests/native/call-transport.toml').read_text().replace('level=0','level=13').replace('="off"','="auto"'))
  target=work/(profile+'.mjs')
  run(profile+'-js-build',[compiler,'tests/native/call-transport.lil','--config',str(config),'--target','js-module','-o',str(target)])
  expression='globalThis.assertNativeCount=()=>{};globalThis.assertMaxNativeCount=()=>{};globalThis.collectNative=()=>{};await import('+json.dumps(str(target))+');'
  actual=run(profile+'-js-run',[node,'--input-type=module','-e',expression]);assert actual==Path('tests/native/call-transport.out').read_bytes()
  if profile=='production':
   target=work/'production.c'
   run('production-c-build',[compiler,'tests/native/call-transport.lil','--config',str(config),'--target','c','-o',str(target)])
   host=work/'production-host.c';host.write_text(Path('tests/native/call-transport.host.c').read_text().replace('ownership.h','production.h'))
   run('production-gcc-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',str(target),str(host),'-lm','-o',str(work/'production')])
   assert run('production-gcc-run',[str(work/'production')])==Path('tests/native/call-transport.out').read_bytes()
finally:
 (work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':digest(compiler)},'rows':rows,'source':digest('tests/native/call-transport.lil'),'oracle':digest('tests/native/call-transport.oracle.mjs'),'expected':digest('tests/native/call-transport.out')},indent=2)+'\n')
