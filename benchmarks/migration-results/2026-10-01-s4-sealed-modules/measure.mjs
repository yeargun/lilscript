import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-sealed-modules-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-field-contracts-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const box='export class Box{int longPrivateField;init(int n){this.longPrivateField=n;}int read(int add=2){return this.longPrivateField+add;}}export constructor Box;';
const cases=[
 {id:'internal-visibility',matched:true,source:'import {Crate} from "./barrel";export int run(int n){Crate b=new Crate(n);return b.read();}',exercise:'console.log(JSON.stringify([-2,0,5,1000].map(n=>m.run(n))));',expected:[0,2,7,1002]},
 {id:'public-barrel',matched:true,source:'import {Crate} from "./barrel";export constructor Crate;export int run(int n){Crate b=new Crate(n);return b.read();}',exercise:'const C=m.Crate,b=new C(6);console.log(JSON.stringify([m.run(5),b.longPrivateField,b.read(),b.read(4),b.constructor===C,b instanceof C,Object.keys(b),C.length,C.prototype.read.length]));',expected:[7,6,8,10,true,true,['longPrivateField'],1,0]},
 {id:'constructor-value',matched:true,source:'import {Crate} from "./barrel";export JsValue ctor(){return Crate;}export int run(int n){Crate b=new Crate(n);return b.read();}',exercise:'const C=m.ctor(),b=new C(6);console.log(JSON.stringify([m.run(5),b.longPrivateField,b.read(),b.read(4),b.constructor===C,b instanceof C,Object.keys(b),C.length,C.prototype.read.length]));',expected:[7,6,8,10,true,true,['longPrivateField'],1,0]},
 {id:'dynamic-constructor',matched:true,source:'import {Crate} from "./barrel";export async JsValue ctor(){auto ns=await import("./box");return ns.Box;}export int run(int n){Crate b=new Crate(n);return b.read();}',exercise:'const C=await m.ctor(),b=new C(6);console.log(JSON.stringify([m.run(5),b.longPrivateField,b.read(),b.read(4),b.constructor===C,b instanceof C,Object.keys(b),C.length,C.prototype.read.length]));',expected:[7,6,8,10,true,true,['longPrivateField'],1,0]},
];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'box.lil'),box);
 writeFileSync(resolve(folder,'barrel.lil'),'import {Box as Crate} from "./box";export {Crate};export constructor Crate;');
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:c.matched,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of [['before',baseline,null],['after',binary,null]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n${mode?`[language]\nfield_initialization='${mode}'\n`:''}${c.matched?'':"[javascript]\nchecks='development'\n"}`);
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
 const matched=rows.filter(r=>r.matched),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'four matched sealed-module boundary cases; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
