from pathlib import Path
import subprocess,json,os,time,hashlib,sys
root=Path('/home/azureuser/lilscript');os.chdir(root)
work=Path(sys.argv[1]);work.mkdir(exist_ok=True)
compiler=sys.argv[2];node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'
rows=[]
def run(name,args,expected=None,env=None):
 start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=120,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1',**(env or {})})
 (work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==0 and (expected is None or p.stdout==expected)
 rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round((time.monotonic()-start)*1000),passed=ok));print(name,ok,flush=True)
 if not ok: print(p.stderr.decode(errors='replace')[-3500:],p.stdout.decode(errors='replace')[-1200:],flush=True)
 return p if ok else None
try:
 for name in ['physical-storage','generic-storage']:
  oracle=run(name+'-oracle',[node,f'tests/native/{name}.oracle.mjs']);assert oracle
  for level in [0,13]:
   for enabled in [False,True]:
    label=f'{name}-{level}-{int(enabled)}';d=work/label;d.mkdir(exist_ok=True);config=d/'source.toml'
    config.write_text(f'effort.level={level}\nobjective.codecs="raw"\n[language]\nabsence="unified"\n[policy.tactics]\ninlining="off"\n[target.native]\nstack_storage={str(enabled).lower()}\ngeneric_specialization={str(enabled).lower()}\ncycle_collection_threshold=1\n')
    c=d/'program.c'
    if not run(label+'-source',[compiler,f'tests/native/{name}.lil','--config',str(config),'--target','c','-o',str(c)]):continue
    wrapper=d/'wrapper.c';wrapper.write_text('#define LS_NATIVE_QUALIFICATION 1\n#define main lilscript_program_main\n#include "program.c"\n#undef main\nint main(int argc,char **argv){int status=lilscript_program_main(argc,argv);fprintf(stderr,"owners=%zu allocations=%zu\\n",ls_native_owned_objects(),ls_native_total_allocations());return status || ls_native_owned_objects()!=0;}\n')
    lanes=[('gcc','cc',[])]
    if enabled:lanes.append(('sanitize',clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']))
    for lane,cc,flags in lanes:
     exe=d/lane
     if run(label+'-'+lane+'-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror',*flags,str(wrapper),'-lm','-o',str(exe)]):run(label+'-'+lane+'-run',[str(exe)],oracle.stdout)
 # Existing escape/exception/suspension paths most exposed to physical lifetime changes.
 for name in ([] if '--new-only' in sys.argv else ['tasks'] if '--finish' in sys.argv else ['ownership','exceptions','generators','tasks']):
  d=work/name;d.mkdir(exist_ok=True);config=d/'source.toml';config.write_text(Path(f'tests/native/{name}.toml').read_text());c=d/('tasks.c' if name=='tasks' else 'ownership.c');host=d/'host.c';host.write_text(Path(f'tests/native/{name}.host.c').read_text())
  if not run(name+'-source',[compiler,f'tests/native/{name}.lil','--config',str(config),'--target','c','-o',str(c)]):continue
  exe=d/'sanitize'
  if run(name+'-sanitize-build',[clang,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(c),str(host),'-lm','-o',str(exe)]):run(name+'-sanitize-run',[str(exe)],Path(f'tests/native/{name}.out').read_bytes())
finally:
 (work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'rows':rows},indent=2)+'\n')
sys.exit(any(not row['passed'] for row in rows))
