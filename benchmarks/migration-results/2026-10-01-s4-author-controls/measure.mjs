import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-inline-for-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-shared-storage-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:120000,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[
 {id:'control',source:'export int probe(int n){int total=0;for(int i=0;i<4;i+=1){total+=n+i;}return total;}',
  exercise:'console.log(JSON.stringify([m.probe(0),m.probe(7)]));',expected:[6,34]},
 {id:'expanded',source:'export int probe(int n){int total=0;inline for(int i of [0,1,2,3]){total+=n+i;}return total;}',
  exercise:'console.log(JSON.stringify([m.probe(0),m.probe(7)]));',expected:[6,34]},
 {id:'captures',source:'export (func()->int)[] make(int offset){(func()->int)[] reads=[];inline for(int n of [1,2,3]){int local=n*10;reads.push(()=>{n+=1;local+=2;return offset+n*100+local;});}return reads;}',
  exercise:'const a=m.make(7),b=m.make(9);console.log(JSON.stringify([a[0](),a[0](),a[1](),a[2](),b[0]()]));',expected:[219,321,329,439,221]},
 {id:'modules',source:'import {make} from "./factory.lil";export int probe(){auto f=make(5);return f[0]()+f[1]()+f[0]();}',
  files:{'factory.lil':'export (func()->int)[] make(int start){(func()->int)[] out=[];inline for(int n of [1,2]){int local=n;out.push(()=>{local+=start;return local;});}return out;}'},
  exercise:'console.log(JSON.stringify([m.probe(),m.probe()]));',expected:[24,24]},
];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 for(const [name,source] of Object.entries(c.files||{}))writeFileSync(resolve(folder,name),source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler] of [['before',baseline],['after',binary]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'\n`);
   const out=resolve(folder,`${label}-${lane}.mjs`);
   const result=run(compiler,[entry,'--config',config,'--format','esm','--target','js-module','--explain','json','-o',out]);
   const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
   const search=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
   if(t.after!==sizes[metric])throw Error('delivered/scored mismatch');
   const observed=run('node',['--input-type=module','-e',`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));${c.exercise}`]).stdout.trim();
   if(observed!==JSON.stringify(c.expected))throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={config_sha256:sha(readFileSync(config)),sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
  row.lanes[lane]=outputs;console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const before=rows.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=rows.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'authored scalar expansion and mutable closure banks, including module conversion and an unchanged ordinary-loop control; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
