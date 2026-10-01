// One pre-existing C3 training family; records-128 separately fails in both compilers. the protected evaluation set is untouched.
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)), root=resolve(directory,'../../..');
const hash=x=>createHash('sha256').update(x).digest('hex');
const run=(cmd,args,input)=>{const result=spawnSync(cmd,args,{cwd:root,input,encoding:'utf8',timeout:180000,maxBuffer:128*1024*1024});if(result.status!==0)throw Error(`${cmd}: ${result.stderr||result.error}`);return result;};
const binaries={q1:'/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript',g1:'/home/azureuser/lilscript-work/bin/g1-complete-2/lilscript'};
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const manifest=JSON.parse(readFileSync(resolve(root,'benchmarks/calibration/corpus/manifest.json'),'utf8'));
const rows=[];
for(const id of ['arithmetic-128']){
 const item=manifest.workloads.find(x=>x.id===id), entry=resolve(root,'benchmarks/calibration/corpus',item.entry);
 if(hash(readFileSync(entry))!==item.source_sha256||item.split!=='training')throw Error('training input drift');
 const folder=resolve(directory,'medium',id);mkdirSync(folder,{recursive:true});
 const row={...item,lanes:{}};
 for(const lane of ['raw','gzip','brotli']){
  const config=resolve(directory,'configs',lane+'.toml');row.lanes[lane]={config_sha256:hash(readFileSync(config))};
  for(const [version,binary] of Object.entries(binaries)){
   const output=resolve(folder,`${version}-${lane}.js`);
   const compiled=run(binary,[entry,'--config',config,'--format','esm','--target','js-module','--explain','json','-o',output]);
   const code=readFileSync(output,'utf8');
   const actual=run('node',['--input-type=module','-e',`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));console.log(JSON.stringify(${JSON.stringify(item.inputs)}.map(x=>m.probe(x))));`]).stdout;
   if(actual!==JSON.stringify(item.expected)+'\n')throw Error(`${id}/${lane}/${version}: ${actual}`);
   const sizes=JSON.parse(run(codec,['--json',output]).stdout).artifacts[0];
   const explain=JSON.parse(compiled.stderr.slice(compiled.stderr.indexOf('{\n'))), search=explain.search, t=search.terminal.objectives[0];
   const metric={raw:'raw',gzip:'gzip9',brotli:'brotli11'}[lane];
   if(t.after!==sizes[metric])throw Error('scored/delivered mismatch');
   row.lanes[lane][version]={sha256:hash(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,
    work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged},
    starts:t.starts,allocation:t.joint_trials.filter(x=>x.name==='naming:compact-order')};
   console.log(`checked ${id}/${lane}/${version}`);
  }
 }
 rows.push(row);
}
const totals=Object.fromEntries([['raw','raw'],['gzip','gzip9'],['brotli','brotli11']].map(([lane,key])=>{const before=rows.reduce((s,r)=>s+r.lanes[lane].q1[key],0), after=rows.reduce((s,r)=>s+r.lanes[lane].g1[key],0);return[lane,{before,after,delta:after-before}];}));
writeFileSync(resolve(directory,'medium.json'),JSON.stringify({scope:'one frozen medium C3 training workload; records-128 is recorded separately as an existing compiler refusal; independent Python-generated oracles; no held-out tuning or CPU claim',format:'esm',effort:13,manifest_sha256:hash(readFileSync(resolve(root,'benchmarks/calibration/corpus/manifest.json'))),compilers:Object.fromEntries(Object.entries(binaries).map(([v,path])=>[v,{path,sha256:hash(readFileSync(path))}])),totals,rows},null,2)+'\n');
console.log(JSON.stringify(totals,null,2));
