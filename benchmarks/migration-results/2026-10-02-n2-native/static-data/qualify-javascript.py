from pathlib import Path
import subprocess,os,json,time
os.chdir('/home/azureuser/lilscript');work=Path('/tmp/lilscript-n2-static-js');work.mkdir(exist_ok=True)
compiler='/home/azureuser/lilscript-work/bin/n2-static-2/lilscript';node='/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node';rows=[]
def run(name,args,expected=None,refusal=None):
 t=time.monotonic();p=subprocess.run(args,capture_output=True,timeout=120);(work/(name+'.log')).write_bytes(p.stderr);(work/(name+'.stdout')).write_bytes(p.stdout)
 status='known-refusal' if refusal and p.returncode and refusal.encode() in p.stderr else 'passed' if not p.returncode and (expected is None or p.stdout==expected) else 'failed'
 rows.append(dict(name=name,args=args,exit=p.returncode,status=status,elapsed_ms=round(1000*(time.monotonic()-t))));print(name,status,flush=True)
 if status=='failed':print(p.stderr.decode()[-3000:],p.stdout.decode()[-500:],flush=True)
 assert status!='failed'
 return p.stdout
try:
 source=Path('tests/native/static-data.lil').read_text();(work/'static-data-module.lil').write_text(Path('tests/native/static-data-module.lil').read_text())
 finite=source.replace('1.0/0.0,0.0/0.0','2.5,3.5');(work/'finite.lil').write_text(finite)
 oracle=Path('tests/native/static-data.oracle.mjs').read_text().replace('Infinity,NaN','2.5,3.5');(work/'finite.oracle.mjs').write_text(oracle);expected=run('finite-oracle',[node,str(work/'finite.oracle.mjs')])
 for level in [0,13]:
  cfg=work/f'level-{level}.toml';text=Path('tests/native/static-data.toml').read_text().replace('level=0',f'level={level}')
  if level:text=text.replace('="off"','="auto"')
  cfg.write_text('objective.codecs="raw"\n'+text)
  if level==13:run('full-13-refusal',[compiler,'tests/native/static-data.lil','--config',str(cfg),'--target','js','-o',str(work/'full-13.js')],refusal='non-finite semantic literal')
  js=work/f'finite-{level}.mjs';run(f'finite-{level}-source',[compiler,str(work/'finite.lil'),'--config',str(cfg),'--target','js','-o',str(js)])
  wrapper=work/f'finite-{level}-run.mjs';wrapper.write_text('globalThis.observeNativeStartup=()=>{};globalThis.collectNative=()=>{};globalThis.assertMaxNativeCount=()=>{};await import('+json.dumps(str(js))+');\n');run(f'finite-{level}-run',[node,str(wrapper)],expected)
 for index,invalid in enumerate([
  'const int[][] data=[[1]];int[][] copy=data.slice();copy[0].push(2);',
  'const Record<int[]> data=record{x:[1]};int[][] copy=Object.values(data);copy[0][0]=2;',
  'const int[] data=[1];int[] copy=data.concat([2]);copy[0]=3;data[0]=4;',
 ]):
  f=work/f'invalid-{index}.lil';f.write_text(invalid);run(f'invalid-{index}',[compiler,str(f),'--config',str(work/'level-0.toml'),'--check'],refusal='const data')
finally:(work/'qualification.json').write_text(json.dumps({'schema':1,'compiler':compiler,'rows':rows},indent=2)+'\n')
