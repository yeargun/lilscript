from pathlib import Path
import subprocess,json,os,time,resource,statistics,hashlib
root=Path('/home/azureuser/lilscript');os.chdir(root);work=Path('/tmp/lilscript-n2-physical-measure-2');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-physical-2/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';rows=[];commands=[]
def run(args,env=None):
 before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=180,env={**os.environ,**(env or {})});after=resource.getrusage(resource.RUSAGE_CHILDREN)
 sample=dict(wall_ms=(time.monotonic()-start)*1000,cpu_ms=(after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime)*1000);commands.append(dict(args=args,exit=p.returncode,**sample));assert p.returncode==0,(args,p.stderr.decode());return p.stdout,sample
try:
 for name in ['closures','generics']:
  expected,_=run([node,'tests/native/profiles.oracle.mjs',name],{'LILSCRIPT_N2_ITERATIONS':'1000001'})
  for lane,flags,stack,generic in [('general',['-O3'],False,False),('stack-only',['-O3'],True,False),('generic-only',['-O3'],False,True),('speed',['-O3'],True,True),('size',['-Os'],True,True),('balanced',['-O2'],True,True)]:
   d=work/(name+'-'+lane);d.mkdir(exist_ok=True);config=d/'source.toml';c=d/'profile.c'
   config.write_text(f'effort.level=13\nobjective.codecs="raw"\n[policy.tactics]\ninlining="off"\n[host.native]\nprofileIterations="host_profileIterations"\n[target.native]\nstack_storage={str(stack).lower()}\ngeneric_specialization={str(generic).lower()}\n')
   _,formation=run([compiler,f'tests/native/profile-{name}.lil','--config',str(config),'--target','c','-o',str(c)])
   _,build=run(['cc','-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-Wall','-Wextra','-Werror','-I',str(d),str(c),'tests/native/profile.host.c','-lm','-o',str(d/'run')])
   samples=[]
   for repeat in range(5):
    out,sample=run([str(d/'run')],{'LILSCRIPT_N2_ITERATIONS':'1000001'});assert out==expected,(name,lane,out,expected);samples.append(sample)
   row=dict(name=name,lane=lane,stack_storage=stack,generic_specialization=generic,c_bytes=c.stat().st_size,executable_bytes=(d/'run').stat().st_size,formation=formation,c_build=build,samples=samples,median_cpu_ms=statistics.median(s['cpu_ms'] for s in samples),median_wall_ms=statistics.median(s['wall_ms'] for s in samples),output=expected.decode());rows.append(row);print(name,lane,round(row['median_cpu_ms'],2),flush=True)
   wrapper=d/'count.c';wrapper.write_text('#define LS_NATIVE_QUALIFICATION 1\n#define main lilscript_program_main\n#include "profile.c"\n#undef main\nint main(int argc,char**argv){int code=lilscript_program_main(argc,argv);fprintf(stderr,"%zu %zu\\n",ls_native_total_allocations(),ls_native_owned_objects());return code;}\n')
   run(['cc','-std=c11','-O2','-I',str(d),str(wrapper),'tests/native/profile.host.c','-lm','-o',str(d/'count')])
   p=subprocess.run([str(d/'count')],capture_output=True,env={**os.environ,'LILSCRIPT_N2_ITERATIONS':'1000001'},timeout=120);assert p.returncode==0 and p.stdout==expected;allocated,live=map(int,p.stderr.decode().split());assert live==0;row['allocations']=allocated;row['live_after_main']=live
  d=work/(name+'-javascript');d.mkdir(exist_ok=True);js=d/'profile.mjs';config=work/(name+'-speed')/'source.toml'
  _,formation=run([compiler,f'tests/native/profile-{name}.lil','--config',str(config),'--target','js','-o',str(js)])
  launcher=d/'run.mjs';launcher.write_text('globalThis.profileIterations=()=>Number(process.env.LILSCRIPT_N2_ITERATIONS)|0;await import('+json.dumps(str(js))+');\n')
  samples=[]
  for repeat in range(5):
   out,sample=run([node,str(launcher)],{'LILSCRIPT_N2_ITERATIONS':'1000001'});assert out==expected;samples.append(sample)
  row=dict(name=name,lane='javascript',js_bytes=js.stat().st_size,formation=formation,samples=samples,median_cpu_ms=statistics.median(s['cpu_ms'] for s in samples),median_wall_ms=statistics.median(s['wall_ms'] for s in samples),output=expected.decode());rows.append(row);print(name,'javascript',round(row['median_cpu_ms'],2),flush=True)
finally:
 (work/'measurement.json').write_text(json.dumps({'scope':'end-to-end process CPU and wall, five serial samples; includes Node startup; no steady-state or fleet claim; inlining disabled to isolate physical costs','compiler':{'path':compiler,'sha256':hashlib.sha256(Path(compiler).read_bytes()).hexdigest()},'rows':rows,'commands':commands},indent=2)+'\n')
