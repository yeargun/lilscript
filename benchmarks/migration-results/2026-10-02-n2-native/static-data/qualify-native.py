from pathlib import Path
import os,subprocess,time,json,hashlib,shutil,re
os.chdir('/home/azureuser/lilscript')
compiler='/home/azureuser/lilscript-work/bin/n2-static-2/lilscript'
clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'
node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node'
work=Path('/tmp/lilscript-n2-static-2');work.mkdir(exist_ok=True)
rows=json.loads((work/'qualification.json').read_text())['rows'] if (work/'qualification.json').exists() else []
def run(name,args,expected=None):
 old=next((row for row in rows if row['name']==name and row['args']==args and row['passed']),None)
 if old:
  output=(work/(name+'.stdout')).read_bytes();assert expected is None or output==expected;print(name,'retained pass',flush=True);return output
 start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'})
 (work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==0 and (expected is None or p.stdout==expected)
 rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round(1000*(time.monotonic()-start)),passed=ok))
 print(name,ok,flush=True)
 if not ok: print(p.stderr.decode(errors='replace')[-4000:],p.stdout.decode(errors='replace')[-1000:],flush=True)
 assert ok
 return p.stdout
try:
 expected=run('independent-oracle',[node,'tests/native/static-data.oracle.mjs']);Path('tests/native/static-data.out').write_bytes(expected)
 for level in [0,13]:
  for static in [True,False]:
   name=f'level-{level}-static-{int(static)}';directory=work/name;directory.mkdir(exist_ok=True)
   config=directory/'source.toml';text=Path('tests/native/static-data.toml').read_text().replace('static_data=true','static_data='+str(static).lower()).replace('level=0',f'level={level}')
   if level: text=text.replace('="off"','="auto"')
   config.write_text(text);c=directory/'static-data.c'
   run(name+'-source',[compiler,'tests/native/static-data.lil','--config',str(config),'--target','c','-o',str(c)])
   host=directory/'host.c';shutil.copy('tests/native/static-data.host.c',host)
   assert int(re.search(r'^#define LS_NATIVE_STATIC_DATA ([01])$',c.read_text(),re.M).group(1)) == int(static)
   for label,cc,extra in [('gcc','cc',[]),('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]+([('clang',clang,[])] if level==0 else []):
    executable=directory/label
    run(name+'-'+label+'-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',f'-DLS_EXPECT_STATIC={int(static)}',*extra,str(c),str(host),'-lm','-o',str(executable)])
    run(name+'-'+label+'-run',[str(executable)],expected)
 for label,cc,extra in [('gcc','cc',[]),('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
  executable=work/('ownership-'+label)
  run('ownership-'+label+'-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror',*extra,'tests/native/runtime-ownership.c','-lm','-o',str(executable)])
  run('ownership-'+label+'-run',[str(executable)],b'runtime ownership: passed\n')
 for level in [0,13]:
  config=work/f'js-{level}.toml';config.write_text(Path('tests/native/static-data.toml').read_text().replace('level=0',f'level={level}').replace('="off"','="auto"') if level else Path('tests/native/static-data.toml').read_text())
  config.write_text('objective.codecs="raw"\n'+config.read_text())
  js=work/f'js-{level}.js';run(f'js-{level}-source',[compiler,'tests/native/static-data.lil','--config',str(config),'--target','js','-o',str(js)])
  wrapper=work/f'js-{level}-run.mjs';wrapper.write_text('globalThis.observeNativeStartup=()=>{};globalThis.collectNative=()=>{};globalThis.assertMaxNativeCount=()=>{};await import('+json.dumps(str(js))+');\n')
  run(f'js-{level}-run',[node,str(wrapper)],expected)
finally:
 (work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'rows':rows},indent=2)+'\n')
