from pathlib import Path
import subprocess,json,hashlib,time,os
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-callable-followup');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-callable-1/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';rows=[]
def run(name,args):
 start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=180);(work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round(1000*(time.monotonic()-start)),passed=p.returncode==0));print(name,p.returncode,flush=True);return p
expected=Path('tests/native/callable-containers.out').read_bytes()
try:
 for profile in ['formation','production']:
  config=Path('tests/native/callable-containers.toml')
  if profile=='production':
   config=work/'production.toml';config.write_text(Path('tests/native/callable-containers.toml').read_text().replace('level=0','level=13').replace('="off"','="auto"'))
  target=work/(profile+'.mjs');r=run(profile+'-js-build',[compiler,'tests/native/callable-containers.lil','--config',str(config),'--target','js-module','-o',str(target)])
  if r.returncode==0:
   expression='globalThis.assertNativeCount=()=>{};globalThis.assertMaxNativeCount=()=>{};globalThis.collectNative=()=>{};await import('+json.dumps(str(target))+');'
   r=run(profile+'-js-run',[node,'--input-type=module','-e',expression]);rows[-1]['passed']=r.returncode==0 and r.stdout==expected
 config=work/'production.toml';target=work/'production.c'
 assert run('production-c-build',[compiler,'tests/native/callable-containers.lil','--config',str(config),'--target','c','-o',str(target)]).returncode==0
 host=work/'production-host.c';host.write_text(Path('tests/native/callable-containers.host.c').read_text().replace('ownership.h','production.h'))
 assert run('production-gcc-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',str(target),str(host),'-lm','-o',str(work/'production')]).returncode==0
 r=run('production-gcc-run',[str(work/'production')]);rows[-1]['passed']=r.returncode==0 and r.stdout==expected
finally:
 (work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'source_sha256':hashlib.sha256(Path('tests/native/callable-containers.lil').read_bytes()).hexdigest(),'rows':rows},indent=2)+'\n')
