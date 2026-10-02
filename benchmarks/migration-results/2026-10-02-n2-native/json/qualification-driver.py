from pathlib import Path
import hashlib,json,os,subprocess,time
root=Path('/home/azureuser/lilscript');os.chdir(root)
work=Path('/tmp/lilscript-n2-json-4');compiler='/home/azureuser/lilscript-work/bin/n2-json-4/lilscript'
node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node'
clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'
inputs=Path('/tmp/lilscript-n2-json-2/midpoints.txt');expected=Path('/tmp/lilscript-n2-json-2/numbers-js.txt')
env={**os.environ,'LILSCRIPT_NATIVE_CLANG':clang,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'}
rows=[]
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def run(name,args,stdin=None,output=None,extra=None):
 start=time.monotonic();result=subprocess.run(args,input=None if stdin is None else stdin.read_bytes(),capture_output=True,env={**env,**(extra or {})},timeout=180)
 (work/(name+'.log')).write_bytes(result.stderr+(b'' if output else result.stdout))
 if output:output.write_bytes(result.stdout)
 rows.append({'name':name,'args':args,'exit':result.returncode,'elapsed_ms':round((time.monotonic()-start)*1000),'passed':result.returncode==0})
 print(name,rows[-1]['passed'],flush=True)
 if result.returncode:raise SystemExit(result.returncode)
 return result.stdout
try:
 run('numbers-gcc-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION','-I'+str(work),'tests/native/json-numbers.c',str(work/'host.c'),'-lm','-o',str(work/'numbers-gcc')])
 run('numbers-gcc-run',[str(work/'numbers-gcc')],inputs,work/'numbers-gcc.txt')
 assert digest(work/'numbers-gcc.txt')==digest(expected)
 locales=work/'locales';locales.mkdir(exist_ok=True)
 run('locale-create',['localedef','--no-archive','-i','de_DE','-f','UTF-8',str(locales/'de_DE.UTF-8')])
 run('numbers-comma-locale',[str(work/'numbers-sanitize'),'de_DE.UTF-8'],inputs,work/'numbers-locale.txt',{'LOCPATH':str(locales)})
 assert digest(work/'numbers-locale.txt')==digest(expected)
 for fixture in ['json','data','arrays','text','ownership']:
  args=['node','tests/native/qualify-ownership.mjs',compiler,str(work/fixture),'--fixture=tests/native/'+fixture]
  if fixture!='ownership':args+=['--generated-only']
  run('fixture-'+fixture,args)
 run('source-js',[compiler,'tests/native/json.lil','--config','tests/native/json.toml','--target','js','-o',str(work/'json.js')])
 expression='globalThis.assertNativeCount=()=>{};globalThis.assertMaxNativeCount=()=>{};globalThis.collectNative=()=>{};await import('+json.dumps(str(work/'json.js'))+');'
 js=run('run-js',[node,'--input-type=module','-e',expression],output=work/'json-js.txt')
 oracle=run('oracle-js',[node,'tests/native/json.oracle.mjs'],output=work/'json-oracle.txt')
 assert js==oracle==Path('tests/native/json.out').read_bytes()
 rows.append({'name':'numeric-parity','passed':all(digest(work/name)==digest(expected) for name in ['numbers-native.txt','numbers-gcc.txt','numbers-locale.txt']),'samples':len(expected.read_bytes().splitlines()),'sha256':digest(expected),'bytes':expected.stat().st_size,'midpoint_samples':len(inputs.read_bytes().splitlines()),'input_sha256':digest(inputs)})
finally:
 (work/'final-qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':digest(compiler)},'rows':rows,'fixtures':{str(p):digest(p) for p in Path('tests/native').glob('json*')},'node':node},indent=2)+'\n')
