// Focused S4 development evidence; frozen G3 artifacts are the comparison.
import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-contracts-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const previous=resolve(directory,'../2026-10-01-g3-complete');
const sha=x=>createHash('sha256').update(x).digest('hex');
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:600000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const prior=JSON.parse(readFileSync(resolve(previous,'comparison.json')));
const metric={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const compile=(entry,config,out,format='esm',target='js-module')=>{
 const r=run(binary,[entry,'--config',config,'--format',format,'--target',target,'--explain','json','-o',out]);
 const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
 const search=JSON.parse(r.stderr.slice(r.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
 return {code,judged:t.after,metrics:{sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}}};
};
const check=(code,exercise,expected)=>{
 const result=run('node',['--input-type=module'],`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));${exercise}`).stdout.trim();
 if(result!==JSON.stringify(expected))throw Error(`oracle mismatch: ${result}`);
};
const rows=[];
for(const id of ['spellings','receiver','modern','g2-generic-closed','g2-generic-open']){
 const before=prior.rows.find(r=>r.id===id),folder=resolve(directory,'artifacts',id);mkdirSync(folder,{recursive:true});
 const boundary=id.startsWith('g2-')?id.split('-').at(-1):null;
 const inputFolder=boundary?resolve(directory,'../2026-10-01-g2-complete/artifacts',id.slice(3)):resolve(previous,'artifacts',id);
 const entry=resolve(inputFolder,'main.lil'),oracle=JSON.parse(readFileSync(resolve(inputFolder,'oracle.json')));
 if(sha(readFileSync(entry))!==before.source_sha256)throw Error('source identity changed');
 const row={id,source_sha256:before.source_sha256,lanes:{}};
 for(const lane of Object.keys(metric)){
  const config=resolve(inputFolder,lane+'.toml');
  if(sha(readFileSync(config))!==before.lanes[lane].config_sha256)throw Error('config identity changed');
  const oldCode=readFileSync(resolve(previous,'artifacts',id,`g3-${lane}.mjs`));
  if(sha(oldCode)!==before.lanes[lane].g3.sha256)throw Error('baseline artifact changed');
  const result=compile(entry,config,resolve(folder,lane+'.mjs'),boundary==='closed'?'iife':'esm',boundary==='closed'?'js':'js-module');
  if(result.judged!==result.metrics[metric[lane]])throw Error('delivered/scored mismatch');
  if(boundary==='closed'){
   const actual=run('node',['--input-type=module'],`let i=0;globalThis.input=()=>${JSON.stringify(oracle.inputs)}[i++];`+result.code).stdout;
   if(actual!==oracle.expected.join('\n')+'\n')throw Error('numeric oracle');
  }else if(boundary){
   check(result.code,`console.log(JSON.stringify([${JSON.stringify(oracle.inputs)}.map(n=>m.compute(n)),m.expose()]));`,[oracle.expected,oracle.shape]);
  }else check(result.code,oracle.exercise,oracle.expected);
  row.lanes[lane]={config_sha256:before.lanes[lane].config_sha256,g3:before.lanes[lane].g3,s4:result.metrics};
  console.log(`checked ${id}/${lane}`);
 }
 rows.push(row);
}
const manifest=JSON.parse(readFileSync(resolve(root,'benchmarks/calibration/corpus/manifest.json'))).workloads.find(w=>w.id==='records-128');
const entry=resolve(root,'benchmarks/calibration/corpus',manifest.entry);
if(sha(readFileSync(entry))!==manifest.source_sha256)throw Error('frozen source changed');
const records={...manifest,lanes:{}};
for(const lane of Object.keys(metric)){
 const folder=resolve(directory,'artifacts/records-128');mkdirSync(folder,{recursive:true});
 const config=resolve(folder,lane+'.toml');
 writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
 const result=compile(entry,config,resolve(folder,lane+'.mjs'));
 check(result.code,`console.log(JSON.stringify(${JSON.stringify(manifest.inputs)}.map(n=>m.probe(n))));`,manifest.expected);
 if(result.judged!==result.metrics[metric[lane]])throw Error('record delivered/scored mismatch');
 records.lanes[lane]=result.metrics;console.log(`checked records-128/${lane}`);
}
const identity=path=>({path,sha256:sha(readFileSync(path))});
const totals=Object.fromEntries(Object.entries(metric).map(([lane,key])=>{
 const before=rows.reduce((n,r)=>n+r.lanes[lane].g3[key],0),after=rows.reduce((n,r)=>n+r.lanes[lane].s4[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'five affected development cases against frozen G3 artifacts; records-128 independently checked, not a compression comparison; no fleet or timing claim',compiler:identity(binary),codec:identity(codec),totals,rows,records},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
