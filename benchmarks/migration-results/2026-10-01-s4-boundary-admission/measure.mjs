import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-boundary-admission-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-captured-callbacks-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'product-control',matched:true,
 source:'struct Point{int x;int y;}export Point increment(Point p){p.x+=1;return p;}',
 exercise:'const p={x:3,y:4};console.log(JSON.stringify([m.increment(p),p]));',expected:[{x:4,y:4},{x:3,y:4}]
},{
 id:'closed-array',matched:true,
 source:'struct Point{int x;}export int total(Point[] values){int n=0;for(Point p of values){n+=p.x;}return n;}export int first(Point[] values){Point p=JS.assume(values[0]);return p.x;}',
 exercise:'let reads=0;console.log(JSON.stringify([m.total([{get x(){reads++;return 3}},{get x(){reads++;return 4}}]),reads,m.first([{x:9}])]));',expected:[7,2,9]
},{
 id:'captured-callback',matched:true,
 source:'struct Point{int x;}export func(int)->int make(func(Point)->Point f,int offset){auto alias=f;return (int n)=>{Point got=alias(Point{n+offset});return got.x;};}',
 exercise:'const f=m.make(p=>({x:p.x+10}),2);console.log(JSON.stringify([f(3),f(4)]));',expected:[15,16]
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected,setup:c.setup||''},null,2)+'\n');
 const row={id:c.id,matched:c.matched,migration:!!c.migration,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of c.matched?[['before',baseline,null],['after',binary,null]]:[['after',binary,null]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`${c.config||''}\n[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const out=resolve(folder,`${label}-${lane}.mjs`);
   const result=run(compiler,[entry,'--config',config,'--format','esm','--target','js-module','--explain','json','-o',out]);
   const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
   const search=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
   if(t.after!==sizes[metric])throw Error('delivered/scored mismatch');
   const observed=run('node',['--input-type=module','-e',`${c.setup||''}const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));${c.exercise}`]).stdout.trim();
   if(observed!==JSON.stringify(c.expected))throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={config_sha256:sha(readFileSync(config)),sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
  row.lanes[lane]=outputs;console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
const groupTotals=migration=>Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const matched=rows.filter(r=>r.matched&&r.migration===migration),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
const totals={unchanged_contract:groupTotals(false)};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'matched concrete product, isolated snapshot-array and captured-callback controls; separate reentrant-array refusal receipt; independent behavior oracles, no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
// Retain the old wrong output and the new source refusal independently of
// the size controls. Never count a refused capability as a compression win.
const counter=resolve(directory,'artifacts/reentrant-array');mkdirSync(counter,{recursive:true});
const source='struct Point{int x;}export int read(Point[] values,func()->void poke){int first=values[0].x;poke();return first*10+values[0].x;}';
const entry=resolve(counter,'main.lil'),config=resolve(counter,'config.toml');
writeFileSync(entry,source);writeFileSync(config,"[objective]\ncodecs='raw'\n[effort]\nlevel=0\n");
const previous=resolve(counter,'before.mjs');run(baseline,[entry,'--config',config,'--format','esm','--target','js-module','-o',previous]);
const code=readFileSync(previous,'utf8');
const observed=run('node',['--input-type=module','-e',`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));const a=[{x:1}];console.log(m.read(a,()=>{a[0]={x:2}}));`]).stdout.trim();
if(observed!=='11')throw Error('counterexample did not reproduce');
const refusal=spawnSync(binary,[entry,'--config',config,'--format','esm','--target','js-module','-o',resolve(counter,'after.mjs')],{cwd:root,encoding:'utf8',timeout:120000});
if(refusal.status===0 || !refusal.stderr.includes('public value-struct ABI adaptation') || !refusal.stderr.includes('main.lil'))throw Error('missing source refusal');
writeFileSync(resolve(directory,'reentrant-array.json'),JSON.stringify({source_sha256:sha(source),expected:12,baseline_observed:Number(observed),baseline_artifact_sha256:sha(code),candidate_exit:refusal.status,candidate_diagnostic:refusal.stderr},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
