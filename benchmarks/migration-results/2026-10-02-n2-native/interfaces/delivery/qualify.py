from pathlib import Path
import os,json,subprocess,time,hashlib
root=Path('/home/azureuser/lilscript');os.chdir(root)
work=Path('/tmp/lilscript-n2-interfaces-extra-3');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-interfaces-3/lilscript'
clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18'
node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node'
rows=[]
def run(name,args,expected=None,code=0):
 start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'})
 (work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==code and (expected is None or p.stdout==expected);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round((time.monotonic()-start)*1000),passed=ok));print(name,ok,flush=True)
 if not ok:print(p.stderr.decode(errors='replace')[-6000:],p.stdout.decode(errors='replace')[-2000:],flush=True)
 assert ok
 return p
try:
 config=work/'failed.toml';config.write_text('effort.level=13\n[target.native]\nartifact="object"\nsymbol_prefix="failed"\ncycle_collection_threshold=1\n[host.native]\nobserve="host_observe"\n')
 source=work/'initialization.c';run('failure-source',[compiler,'tests/native/interfaces-init.lil','--config',str(config),'--target','c','-o',str(source)])
 exe=work/'failed';run('failure-sanitizer-build',[clang,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I',str(work),str(source),'tests/native/interfaces-init.client.c','-lm','-o',str(exe)]);run('failure-sanitizer-run',[str(exe)],b'initialization failure cached\n')
 config=work/'peer.toml';config.write_text('effort.level=13\n[target.native]\nartifact="shared-library"\nsymbol_prefix="peer"\n')
 peer=work/'peer.c';run('peer-source',[compiler,'tests/native/interfaces-peer.lil','--config',str(config),'--target','c','-o',str(peer)])
 lib=work/'libpeer.so';run('peer-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-fPIC','-shared','-DLS_NATIVE_QUALIFICATION',str(peer),'-lm','-o',str(lib)])
 old=Path('/tmp/lilscript-n2-interfaces-3/interfaces-13');exe=work/'coexist';run('coexist-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION','-I',str(old),'-I',str(work),'tests/native/interfaces-coexist.client.c',str(old/'libinterfaces-gcc.so'),str(lib),'-lm','-o',str(exe)]);run('coexist-run',[str(exe)],b'two libraries coexist\n')
 for artifact in ['shared-library','object']:
  d=work/artifact;d.mkdir(exist_ok=True);config=d/'source.toml';text=Path('tests/native/interfaces.toml').read_text().replace('level=0','level=13').replace('artifact="shared-library"',f'artifact="{artifact}"')+'\n'
  if artifact=='shared-library':text+='[host]\nnative_sources=['+json.dumps(str(root/'tests/native/interfaces.host.c'))+']\n'
  config.write_text(text);output=d/('interfaces.so' if artifact=='shared-library' else 'interfaces.o')
  run(f'cli-{artifact}',[compiler,'tests/native/interfaces.lil','--config',str(config),'--target','native','-o',str(output)])
  client=d/'main.c';client.write_text('#include "interfaces.h"\n#include <assert.h>\nint main(void){assert(demo_initialize(0,0));assert(demo_e_interfaces__callProvider(4)==12);demo_shutdown();}\n')
  exe=d/'client';args=['cc','-std=c11','-Wall','-Wextra','-Werror','-I',str(d),str(client)]
  if artifact=='object':args+=['tests/native/interfaces.host.c']
  run(f'cli-{artifact}-client-build',args+[str(output),'-lm','-o',str(exe)]);run(f'cli-{artifact}-client-run',[str(exe)],b'')
 # Two source entry names differ by escapes, and both publish the same name.
 config=work/'multi.toml';config.write_text('effort.level=13\n[target.native]\nartifact="object"\nsymbol_prefix="duo"\n[delivery.entries]\n"a-b"='+json.dumps(str(root/'tests/native/interfaces-peer.lil'))+'\n"a_2db"='+json.dumps(str(root/'tests/native/interfaces-barrel.lil'))+'\n')
 source=work/'multi.c';run('multi-source',[compiler,'--config',str(config),'--target','c','-o',str(source)])
 client=work/'multi-client.c';client.write_text('#include "multi.h"\n#include <assert.h>\nint main(void){assert(duo_initialize(0,0));assert(duo_e_a_2db__add(5)==1005);assert(duo_e_a_5f2db__add(6)==1006);duo_shutdown();}\n')
 exe=work/'multi';run('multi-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror',str(source),str(client),'-lm','-o',str(exe)]);run('multi-run',[str(exe)],b'')
 # Independent JavaScript oracle for the public-ancestor retention fix.
 oracle=work/'oracle.mjs';oracle.write_text('class Counter{constructor(start=4){this.value=start}bump(by=1){return this.value+=by}}class DoubleCounter extends Counter{bump(by=1){return this.value+=by*2}}console.log(new DoubleCounter(10).bump(3));\n')
 expected=run('override-independent-oracle',[node,str(oracle)]).stdout
 for level in [0,13]:
  config=work/f'js-{level}.toml';config.write_text(f'effort.level={level}\nobjective.codecs="raw"\n[language]\nabsence="unified"\n')
  output=work/f'interface-{level}.mjs';run(f'js-{level}-source',[compiler,'tests/native/interfaces.lil','--config',str(config),'--target','js-module','-o',str(output)])
  runner=work/f'run-{level}.mjs';runner.write_text('const m=await import('+json.dumps(str(output))+');console.log(m.doubled(10).bump(3));\n');run(f'js-{level}-override',[node,str(runner)],expected)
finally:(work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':dict(path=compiler,sha256=hashlib.sha256(Path(compiler).read_bytes()).hexdigest()),'rows':rows},indent=2)+'\n')
