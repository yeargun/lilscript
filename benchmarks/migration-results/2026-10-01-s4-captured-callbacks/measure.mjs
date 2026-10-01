import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-captured-callbacks-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-choices-2/lilscript';
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
 id:'callback-control',matched:true,
 source:'struct Point{int x;}export Point apply(func(Point)->Point f,Point p){auto copy=f;return copy(p);}',
 exercise:'console.log(JSON.stringify(m.apply(p=>({x:p.x+2}),{x:4})));',expected:{x:6}
},{
 id:'captured-callbacks',matched:false,
 source:'struct Point{int x;}export func(int)->int make(func(Point)->Point f,int offset){auto alias=f;return (int n)=>{auto nested=(int x)=>{auto local=alias;Point got=local(Point{x});return got.x;};return nested(n+offset);};}',
 exercise:"let reads=0;const f=m.make(p=>({get x(){reads++;return p.x+10}}),2),a=f(3),b=f(4);let thrown=false;try{m.make(()=>{throw 'sentinel'},0)(1)}catch(e){thrown=e==='sentinel'}console.log(JSON.stringify([a,b,reads,thrown]));",expected:[15,16,2,true]
},{
 id:'captured-development',matched:false,config:'javascript.checks="development"',
 source:'struct Point{int x;}export func()->int make(func(Point)->Point f){return ()=>{Point got=f(Point{4});return got.x;};}',
 exercise:"let reads=0;const f=m.make(p=>({get x(){reads++;return p.x+3}})),value=f(),bad=[];try{m.make(3)}catch(e){bad.push(e instanceof TypeError)}try{m.make(()=>({x:'wrong'}))()}catch(e){bad.push(e instanceof TypeError)}console.log(JSON.stringify([value,reads,bad]));",expected:[7,1,[true,true]]
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
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'existing product/callback controls plus captured invocation-only callback capabilities; independent behavior oracles, no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
