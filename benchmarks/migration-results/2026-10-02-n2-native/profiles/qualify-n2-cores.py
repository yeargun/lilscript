from pathlib import Path
import subprocess,json,os,time,resource,statistics,hashlib
root=Path('/home/azureuser/lilscript');os.chdir(root);work=Path('/tmp/lilscript-n2-cores-3');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-physical-3/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18';sdk=Path('/home/azureuser/lilscript-work/toolchains/wasi-sdk-34.0-x86_64-linux');runtime='/home/azureuser/lilscript-work/toolchains/wasmtime-v49.0.1-x86_64-linux/wasmtime';rows=[];measurements=[]
def run(name,args,expected=None,env=None):
 before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=120,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1',**(env or {})});after=resource.getrusage(resource.RUSAGE_CHILDREN)
 sample=dict(wall_ms=(time.monotonic()-start)*1000,cpu_ms=(after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime)*1000)
 (work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==0 and (expected is None or p.stdout==expected);rows.append(dict(name=name,args=args,exit=p.returncode,**sample,passed=ok));print(name,ok,flush=True)
 if not ok:print(p.stderr.decode(errors='replace')[-3500:],p.stdout.decode(errors='replace')[-1000:],flush=True)
 assert ok
 return p.stdout,sample
try:
 for name,count in [('katex',131072),('marked',128)]:
  d=work/name;d.mkdir(exist_ok=True);config=d/'source.toml';source=Path(f'tests/native/profile-{name}.lil');env={'LILSCRIPT_N2_ITERATIONS':str(count)}
  config.write_text('effort.level=13\nobjective.codecs="raw"\n[language]\nabsence="unified"\n[host.native]\nprofileIterations="host_profileIterations"\n')
  expected,_=run(name+'-oracle',[node,'tests/native/cores/oracle.mjs',name],env=env)
  c=d/'profile.c';js=d/'profile.mjs';run(name+'-source-c',[compiler,str(source),'--config',str(config),'--target','c','-o',str(c)])
  run(name+'-source-js',[compiler,str(source),'--config',str(config),'--target','js','-o',str(js)])
  launcher=d/'run.mjs';launcher.write_text('globalThis.profileIterations=()=>Number(process.env.LILSCRIPT_N2_ITERATIONS)|0;await import('+json.dumps(str(js))+');\n')
  lanes=[]
  for label,cc,flags in [('speed','cc',['-O3']),('size','cc',['-Os']),('balanced','cc',['-O2']),('sanitize',clang,['-O2','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   exe=d/label;run(name+'-'+label+'-build',[cc,'-std=c11','-fno-fast-math','-ffp-contract=off','-Wall','-Wextra','-Werror',*flags,'-I',str(d),str(c),'tests/native/profile.host.c','-lm','-o',str(exe)])
   lanes.append((label,[str(exe)]))
  lanes.append(('javascript',[node,str(launcher)]))
  for lane,args in lanes:
   samples=[]
   for i in range(1 if lane=='sanitize' else 5):
    _,sample=run(f'{name}-{lane}-{i}',args,expected,env);samples.append(sample)
   measurements.append(dict(name=name,iterations=count,lane=lane,samples=samples,median_cpu_ms=statistics.median(s['cpu_ms'] for s in samples),median_wall_ms=statistics.median(s['wall_ms'] for s in samples),artifact_bytes=(js if lane=='javascript' else d/lane).stat().st_size))
  # Exercise the CLI-owned cross toolchain and its delivered receipt/header.
  cross=d/'wasi.toml';cross.write_text(config.read_text()+f'[host]\nnative_sources=["/home/azureuser/lilscript/tests/native/profile.host.c"]\n[target.native]\ncompiler="{sdk}/bin/clang"\ntriple="wasm32-wasip1"\nsysroot="{sdk}/share/wasi-sysroot"\nwasm_stack_size=1048576\nobjective="balanced"\nwarnings_as_errors=true\n');wasm=d/'profile.wasm'
  run(name+'-wasi-cli',[compiler,str(source),'--config',str(cross),'--target','native','-o',str(wasm)])
  run(name+'-wasi-run',[runtime,'run','--env',f'LILSCRIPT_N2_ITERATIONS={count}',str(wasm)],expected)
finally:
 (work/'qualification.json').write_text(json.dumps({'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'scope':'end-to-end serial process samples, Node/Wasm startup included; portable cores only, not full library parsers/renderers','rows':rows,'measurements':measurements},indent=2)+'\n')
