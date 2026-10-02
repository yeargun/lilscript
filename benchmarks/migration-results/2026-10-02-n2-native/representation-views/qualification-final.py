from pathlib import Path
import os,subprocess,time,json,hashlib
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-views-final');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-views-4/lilscript';clang='/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18';rows=[]
def run(name,args,expected=None):
 start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=1:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'});(work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout);ok=p.returncode==0 and (expected is None or p.stdout==expected);rows.append(dict(name=name,args=args,exit=p.returncode,elapsed_ms=round(1000*(time.monotonic()-start)),passed=ok));print(name,ok,flush=True);assert ok
try:
 for fixture in ['representation-views','call-transport']:
  config=work/(fixture+'.toml');config.write_text(Path('tests/native/'+fixture+'.toml').read_text().replace('level=0','level=13').replace('="off"','="auto"'))
  target=work/(fixture+'.c');run(fixture+'-c-build',[compiler,'tests/native/'+fixture+'.lil','--config',str(config),'--target','c','-o',str(target)])
  host=work/(fixture+'-host.c');host.write_text(Path('tests/native/'+fixture+'.host.c').read_text().replace('ownership.h',fixture+'.h'))
  for cc,extra in [('cc',[])]+([(clang,['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])] if fixture=='representation-views' else []):
   label='sanitize' if extra else 'gcc';out=work/(fixture+'-'+label)
   run(fixture+'-'+label+'-build',[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',*extra,str(target),str(host),'-lm','-o',str(out)])
   run(fixture+'-'+label+'-run',[str(out)],Path('tests/native/'+fixture+'.out').read_bytes())
 source=work/'plain-binary.lil';source.write_text('Uint8Array bytes=new Uint8Array(4);bytes[2]=255;print(bytes[2]);print(bytes.subarray(1,3).length);\n');c=work/'plain-binary.c'
 run('plain-binary-source',[compiler,str(source),'--target','c','-o',str(c)])
 assert 'typedef struct ls_value' not in c.read_text(),'ordinary binary code must not pull in tagged recipes'
 run('plain-binary-gcc-build',['cc','-std=c11','-O2','-Wall','-Wextra','-Werror',str(c),'-lm','-o',str(work/'plain-binary')])
 run('plain-binary-gcc-run',[str(work/'plain-binary')],b'255\n2\n')
finally:
 (work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'rows':rows},indent=2)+'\n')
