import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-field-contracts-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-host-boundaries-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'class-fields',matched:true,
 source:`export class Box{int value;int[] entries;init(int n){this.value=n;this.entries=[n,n+1];}int next(){this.value+=1;return this.value;}}export constructor Box;export int read(int n){Box box=new Box(n);return box.next()+box.entries[1];}`,
 exercise:`const b=new m.Box(4);console.log(JSON.stringify([b.next(),b.entries,b.constructor===m.Box,b instanceof m.Box,m.read(5),Object.keys(b)]));`,
 expected:[5,[4,5],true,true,12,['value','entries']]
},{
 id:'checked-indexing',matched:false,
 source:`void local(int i){int[] a=[1];a[i];}void binary(int i){Uint8Array a=new Uint8Array(1);a[i];}void unit(int i){"x".codeUnitAt(i);}void text(int i){"x"[i];}export void run(int kind,int index){if(kind==0){local(index);}else if(kind==1){binary(index);}else if(kind==2){unit(index);}else{text(index);}}export number get(int i){Float64Array a=new Float64Array(1);a[0]=2.5;return a.get(i)??-3.0;}`,
 exercise:`const r=[];for(let kind=0;kind<4;kind++)for(const i of [0,-1,1]){try{m.run(kind,i);r.push('ok');}catch(e){r.push(e instanceof RangeError);}}console.log(JSON.stringify([r,[-1,0,1].map(i=>m.get(i))]));`,
 expected:[['ok',true,true,'ok',true,true,'ok',true,true,'ok',true,true],[-3,2.5,-3]]
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:c.matched,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of c.matched?[['before',baseline,null],['legacy',binary,'legacy'],['explicit',binary,'explicit']]:[['explicit',binary,'explicit']]){
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
 const matched=rows.filter(r=>r.matched),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].explicit[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'one matched class boundary under legacy/explicit contracts and a new checked-indexing oracle; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
