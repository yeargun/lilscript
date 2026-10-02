from pathlib import Path
import subprocess,os,json,time,hashlib
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-generators-1');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-generators-1/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18';rows=[]
def run(name,args,expected=None):
 t=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=240,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'});(work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=not p.returncode and (expected is None or p.stdout==expected);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round(1000*(time.monotonic()-t)),passed=ok));print(name,ok,flush=True)
 if not ok:print(p.stderr.decode(errors='replace')[-5000:],p.stdout.decode(errors='replace')[-1000:],flush=True)
 assert ok
 return p.stdout
try:
 expected=run('independent-oracle',[node,'tests/native/generators.oracle.mjs']);Path('tests/native/generators.out').write_bytes(expected)
 for level in [0,13]:
  directory=work/f'level-{level}';directory.mkdir(exist_ok=True);config=directory/'source.toml';text=Path('tests/native/generators.toml').read_text().replace('level=0',f'level={level}')
  if level:text=text.replace('="off"','="auto"')
  config.write_text(text);source=directory/'generators.c';run(f'level-{level}-source',[compiler,'tests/native/generators.lil','--config',str(config),'--target','c','-o',str(source)])
  host=directory/'host.c';host.write_text(Path('tests/native/generators.host.c').read_text().replace('ownership.h','generators.h'))
  for label,cc,extra in [('gcc','cc',[]),('clang',clang,[]),('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   executable=directory/label;run(f'level-{level}-{label}-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',*extra,str(source),str(host),'-lm','-o',str(executable)]);run(f'level-{level}-{label}-run',[str(executable)],expected)
finally:(work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':dict(path=compiler,sha256=hashlib.sha256(Path(compiler).read_bytes()).hexdigest()),'rows':rows},indent=2)+'\n')
