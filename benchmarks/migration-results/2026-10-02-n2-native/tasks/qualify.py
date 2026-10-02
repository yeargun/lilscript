from pathlib import Path
import os,json,subprocess,time,hashlib
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-tasks-3');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-tasks-3/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18';rows=[]
def run(name,args,expected=None,code=0,required=True):
 t=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=240,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'});(work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==code and (expected is None or p.stdout==expected);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round(1000*(time.monotonic()-t)),passed=ok));print(name,ok,flush=True)
 if not ok:print(p.stderr.decode(errors='replace')[-5000:],p.stdout.decode(errors='replace')[-1000:],flush=True)
 if required:assert ok
 return p
try:
 for name in ['tasks','task-contracts']:
  expected=run(f'{name}-independent-oracle',[node,f'tests/native/{name}.oracle.mjs']).stdout;Path(f'tests/native/{name}.out').write_bytes(expected)
  for level in [0,13]:
   d=work/f'{name}-{level}';d.mkdir(exist_ok=True);config=d/'source.toml';text=Path(f'tests/native/{name}.toml').read_text().replace('level=0',f'level={level}')
   if level:text=text.replace('="off"','="auto"')
   config.write_text(text);source=d/'tasks.c';run(f'{name}-{level}-source',[compiler,f'tests/native/{name}.lil','--config',str(config),'--target','c','-o',str(source)])
   host=Path(f'tests/native/{name}.host.c')
   for label,cc,extra in [('gcc','cc',[]),('clang',clang,[]),('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
    executable=d/label;run(f'{name}-{level}-{label}-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',*extra,'-I',str(d),str(source),str(host),'-lm','-o',str(executable)]);run(f'{name}-{level}-{label}-run',[str(executable)],expected)
 for name in ['task-unhandled','generator-completions']:
  d=work/name;d.mkdir(exist_ok=True);source=d/'tasks.c';config=f'tests/native/{name}.toml';expected=run(f'{name}-oracle',[node,f'tests/native/{name}.oracle.mjs'],code=1 if name=='task-unhandled' else 0).stdout
  run(f'{name}-source',[compiler,f'tests/native/{name}.lil','--config',config,'--target','c','-o',str(source)])
  host=d/'host.c';body=Path('tests/native/tasks.host.c' if name=='task-unhandled' else f'tests/native/{name}.host.c').read_text().replace('ownership.h','tasks.h');host.write_text(body)
  exe=d/'sanitize';run(f'{name}-build',[clang,'-std=c11','-O2','-Wall','-Wextra','-Werror','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-DLS_NATIVE_QUALIFICATION',str(source),str(host),'-lm','-o',str(exe)]);run(f'{name}-run',[str(exe)],expected,code=1 if name=='task-unhandled' else 0)
 for name in ['tasks','task-contracts']:
  expected=Path(f'tests/native/{name}.out').read_bytes()
  for level in [0,13]:
   d=work/f'{name}-{level}';config=d/'js.toml';config.write_text('objective.codecs="raw"\n'+(d/'source.toml').read_text());output=d/'output.mjs'
   p=run(f'{name}-{level}-javascript-source',[compiler,f'tests/native/{name}.lil','--config',str(config),'--target','js','-o',str(output)],required=False)
   if p.returncode==0:
    runner=d/'run.mjs';runner.write_text('globalThis.collectNative=()=>{};globalThis.assertMaxNativeCount=()=>{};globalThis.watchNativeExit=()=>{};await import('+json.dumps(str(output))+');\n');run(f'{name}-{level}-javascript-run',[node,str(runner)],expected,required=False)
finally:(work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':dict(path=compiler,sha256=hashlib.sha256(Path(compiler).read_bytes()).hexdigest()),'rows':rows},indent=2)+'\n')
