from pathlib import Path
import subprocess,json,os,time,hashlib
root=Path('/home/azureuser/lilscript');os.chdir(root);work=Path('/tmp/lilscript-n2-wasi-3');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-physical-3/lilscript';sdk=Path('/home/azureuser/lilscript-work/toolchains/wasi-sdk-34.0-x86_64-linux');runtime='/home/azureuser/lilscript-work/toolchains/wasmtime-v49.0.1-x86_64-linux/wasmtime';rows=[]
def run(name,args,expected=None):
 start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=120)
 (work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==0 and (expected is None or p.stdout==expected);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round((time.monotonic()-start)*1000),passed=ok));print(name,ok,flush=True)
 if not ok:print(p.stderr.decode(errors='replace')[-3500:],p.stdout.decode(errors='replace')[-1000:],flush=True)
 return ok
try:
 # Cross-target runtime matrix over unchanged previously qualified semantic inputs.
 for name in ['binary64','ownership','text','json','generators','tasks','physical-storage','generic-storage']:
  d=work/name;d.mkdir(exist_ok=True);host=None
  if name=='binary64':src=Path('src/program/fixtures/native/binary64.lil');expected=src.with_suffix('.expected.out').read_bytes();config='effort.level=13\n[language]\nabsence="unified"\n[target.native]\n'
  elif name in ['physical-storage','generic-storage']:
   src=Path(f'tests/native/{name}.lil');expected=(Path('/tmp/lilscript-n2-physical-2c')/(name+'-oracle.stdout')).read_bytes();config='effort.level=13\n[language]\nabsence="unified"\n[policy.tactics]\ninlining="off"\n[target.native]\n'
  else:
   src=Path(f'tests/native/{name}.lil');expected=src.with_suffix('.out').read_bytes();config=src.with_suffix('.toml').read_text();host=d/'host.c';host.write_text(src.with_suffix('.host.c').read_text());config=config.split('[target.native]')[0]+'[target.native]\n'
  config+=f'compiler="{sdk}/bin/clang"\ntriple="wasm32-wasip1"\nsysroot="{sdk}/share/wasi-sysroot"\nobjective="balanced"\nwarnings_as_errors=true\nwasm_stack_size=1048576\n'
  cfg=d/'source.toml';cfg.write_text(config);c=d/('tasks.c' if name=='tasks' else 'ownership.c');wasm=d/'run.wasm'
  if not run(name+'-source',[compiler,str(src),'--config',str(cfg),'--target','c','-o',str(c)]):continue
  args=[str(sdk/'bin/clang'),'--target=wasm32-wasip1','--sysroot='+str(sdk/'share/wasi-sysroot'),'-std=c11','-O2','-fno-fast-math','-ffp-contract=off','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION','-Wl,-z,stack-size=1048576',str(c)]
  if host:args.append(str(host))
  args+=['-lm','-o',str(wasm)]
  if run(name+'-build',args):run(name+'-run',[runtime,str(wasm)],expected)
finally:
 (work/'qualification.json').write_text(json.dumps({'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'toolchains':json.loads(Path('/home/azureuser/lilscript-work/toolchains/n2-wasi-identity.json').read_text()),'rows':rows},indent=2)+'\n')
