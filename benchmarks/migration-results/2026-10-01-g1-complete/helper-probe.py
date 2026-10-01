from pathlib import Path
import subprocess,json
folder=Path('benchmarks/migration-results/2026-10-01-g1-complete');p=folder/'helper-holes';p.mkdir(exist_ok=True)
source='extern int input();'+''.join(f'int helper{i}(int x){{return x+{i};}}' for i in range(80))+'int seed=input();'+''.join(f'print(helper{i}(seed));print(helper{i}(seed+1));' for i in range(80))
entry=p/'main.lil';entry.write_text(source)
expected=''.join(f'{7+i}\n{8+i}\n' for i in range(80));rows=[]
for lane,metric in [('raw','raw'),('gzip','gzip9'),('brotli','brotli11')]:
 row={'codec':lane}
 for version in ['q1','g1']:
  binary=f'/home/azureuser/lilscript-work/bin/{version}-complete-{2 if version=="q1" else 1}/lilscript';out=p/f'{version}-{lane}.js'
  r=subprocess.run([binary,str(entry),'--config',str(folder/f'configs/{lane}.toml'),'--target','js','--format','iife','--explain','json','-o',str(out)],capture_output=True,text=True,timeout=180)
  if r.returncode:raise Exception(r.stderr[:1000])
  code=out.read_text();actual=subprocess.run(['node','-e','globalThis.input=()=>7;'+code],capture_output=True,text=True);assert actual.returncode==0 and actual.stdout==expected,(version,lane,actual.stderr)
  measured=json.loads(subprocess.check_output(['/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec','--json',str(out)],text=True))['artifacts'][0]
  report=json.loads(r.stderr[r.stderr.index('{\n'):]);t=report['search']['terminal']['objectives'][0]
  row[version]={'size':measured[metric],'raw':measured['raw'],'allocation':[x for x in t['joint_trials'] if x['name']=='naming:compact-order'],'judged':t['judged']}
  print(lane,version,row[version],flush=True)
 rows.append(row)
(folder/'helper-holes.json').write_text(json.dumps(rows,indent=2)+'\n')
