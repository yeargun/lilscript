from pathlib import Path
import os,json,subprocess,time,hashlib
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-interfaces-contracts-3');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-interfaces-3/lilscript';clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18';rows=[]
def run(name,args,expected=None,code=0,input=None):
 t=time.monotonic();p=subprocess.run(args,capture_output=True,input=input,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'});(work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 ok=p.returncode==code and (expected is None or expected==p.stdout);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round((time.monotonic()-t)*1000),passed=ok));print(name,ok,flush=True)
 if not ok:print(p.stderr.decode(errors='replace')[-5000:],p.stdout.decode(errors='replace')[-1500:],flush=True)
 assert ok
 return p
try:
 for level in [0,13]:
  d=work/f'generic-{level}';d.mkdir(exist_ok=True);config=d/'source.toml';config.write_text(f'effort.level={level}\n[language]\nabsence="unified"\n[target.native]\nartifact="object"\nsymbol_prefix="generic"\ncycle_collection_threshold=1\n')
  source=d/'generic.c';run(f'generic-{level}-source',[compiler,'tests/native/interfaces-generic.lil','--config',str(config),'--target','c','-o',str(source)])
  exe=d/'run';run(f'generic-{level}-sanitizer-build',[clang,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I',str(d),str(source),'tests/native/interfaces-generic.client.c','-lm','-o',str(exe)])
  run(f'generic-{level}-sanitizer-run',[str(exe)],b'generic override and enum interfaces done\n')
 source=work/'provider.lil';source.write_text('extern int read();print(read());\n')
 for label,body,diagnostic in [('provider-type','[host.native]\nread="host_lil_stdin"','native bundled provider'),('provider-name','[host.native]\nread="host_lil_unknown"','native bundled provider'),('prefix','[target.native]\nsymbol_prefix="ls_private"','symbol_prefix'),('object-lto','[target.native]\nartifact="object"\nlink_time_optimization=true','link_time_optimization')]:
  config=work/(label+'.toml');config.write_text('effort.level=0\n'+body+'\n');p=run(label,[compiler,str(source),'--config',str(config),'--target','c','-o',str(work/'invalid.c')],code=1);assert diagnostic in p.stderr.decode()
 # Native-only controls leave a JavaScript-only policy unchanged.
 policies=[]
 for label,settings in [('executable','artifact="executable"\nsymbol_prefix="first"'),('object','artifact="object"\nsymbol_prefix="second"')]:
  config=work/(label+'.toml');config.write_text('effort.level=0\n[target.native]\n'+settings+'\n')
  pair=[]
  for target in ['js','c']:
   p=run(f'policy-{label}-{target}',[compiler,'tests/native/interfaces-peer.lil','--config',str(config),'--target',target,'--print-policy']);pair.append(json.loads(p.stdout)['fingerprint'])
  policies.append(pair)
 assert policies[0][0]==policies[1][0] and policies[0][1]!=policies[1][1]
 # An empty library still has a functioning explicit lifecycle and header.
 source=work/'empty.lil';source.write_text('');config=work/'empty.toml';config.write_text('effort.level=13\n[target.native]\nartifact="object"\nsymbol_prefix="empty"\n');out=work/'empty.c'
 run('empty-source',[compiler,str(source),'--config',str(config),'--target','c','-o',str(out)])
 client=work/'empty-client.c';client.write_text('#include "empty.h"\n#include <assert.h>\nint main(void){assert(empty_initialize(0,0));assert(empty_drain());empty_shutdown();}\n');exe=work/'empty';run('empty-build',['cc','-std=c11','-Wall','-Wextra','-Werror',str(out),str(client),'-lm','-o',str(exe)]);run('empty-run',[str(exe)],b'')
 # Malformed stdin is a real failure, never lossy replacement or a sanitizer finding.
 exe='/tmp/lilscript-n2-interfaces-3/providers-13/sanitize';p=run('stdin-invalid',[exe,'run','utf8',str(work/'unused.txt')],code=1,input=b'\xc0\xaf');assert b'EncodingError' in p.stderr and b'AddressSanitizer' not in p.stderr
finally:(work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':dict(path=compiler,sha256=hashlib.sha256(Path(compiler).read_bytes()).hexdigest()),'rows':rows},indent=2)+'\n')
