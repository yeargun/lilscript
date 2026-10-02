from pathlib import Path
import os,json,subprocess,time,hashlib
root=Path('/home/azureuser/lilscript');os.chdir(root)
work=Path('/tmp/lilscript-n2-interfaces-3');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-interfaces-3/lilscript'
clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'
rows=[]
def run(name,args,expected=None,code=0,env=None):
 start=time.monotonic();p=subprocess.run(args,capture_output=True,input="stdin é 😀".encode(),timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1',**(env or {})})
 (work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==code and (expected is None or p.stdout==expected)
 rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round((time.monotonic()-start)*1000),passed=ok));print(name,ok,flush=True)
 if not ok: print(p.stderr.decode(errors='replace')[-5000:],p.stdout.decode(errors='replace')[-2000:],flush=True)
 assert ok
 return p
try:
 for level in [0,13]:
  d=work/f'interfaces-{level}';d.mkdir(exist_ok=True)
  config=d/'source.toml';config.write_text(Path('tests/native/interfaces.toml').read_text().replace('level=0',f'level={level}'))
  source=d/'interfaces.c';run(f'library-{level}-source',[compiler,'tests/native/interfaces.lil','--config',str(config),'--target','c','-o',str(source)])
  for label,cc,flags in [('gcc','cc',[]),('clang',clang,[]),('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   lib=d/f'libinterfaces-{label}.so';run(f'library-{level}-{label}-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',*flags,'-fPIC','-shared','-I',str(d),str(source),'tests/native/interfaces.host.c','-lm','-o',str(lib)])
   client=d/f'client-{label}';run(f'client-{level}-{label}-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',*flags,'-I',str(d),'tests/native/interfaces.client.c',str(lib),'-lm','-o',str(client)])
   run(f'client-{level}-{label}-run',[str(client)],b'native interfaces done\n')
  exports=run(f'library-{level}-namespace',['nm','-D','--defined-only',str(d/'libinterfaces-gcc.so')]).stdout.decode().splitlines()
  assert all(line.split()[-1].startswith('demo_') or line.split()[-1]=='host_provider' for line in exports)
 for level in [0,13]:
  d=work/f'providers-{level}';d.mkdir(exist_ok=True);config=d/'source.toml';config.write_text(Path('tests/native/providers.toml').read_text().replace('level=0',f'level={level}'));source=d/'providers.c'
  run(f'providers-{level}-source',[compiler,'tests/native/providers.lil','--config',str(config),'--target','c','-o',str(source)])
  expected='4\ntrue\ntrue\nrun\né😀\nvalue é 😀\ntrue\nout é 😀\nstdin é 😀\nfirst é\nsecond 😀\x00end\ntrue\ntrue\nmissing caught\nnul caught\nsurrogate caught\nfirst é\nsecond 😀\x00end\n'.encode()
  for label,cc,flags in [('gcc','cc',[]),('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   exe=d/label;run(f'providers-{level}-{label}-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror',*flags,str(source),'-lm','-o',str(exe)])
   p=run(f'providers-{level}-{label}-run',[str(exe),'run','é😀',str(d/'text.txt')],expected,env={'LILSCRIPT_N2_TEXT':'value é 😀'})
   assert p.stderr=='err é 😀\n'.encode()
   run(f'providers-{level}-{label}-exit',[str(exe),'exit'],b'exiting\n',code=7)
finally:
 (work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'rows':rows},indent=2)+'\n')
