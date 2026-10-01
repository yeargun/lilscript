import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-choices-2/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-pool-2/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:120000,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[
 {id:'control',source:'string label(int n){if(n>0){return "alpha";}return "beta";}export string probe(int n){return label(n)+label(-n);}',exercise:'console.log(JSON.stringify([m.probe(0),m.probe(7)]));',expected:['betabeta','alphabeta']},
 {id:'branches',source:'@choose(conditional_returns=0) int first(bool c,int n){if(c){return n+1;}return n+2;}@choose(conditional_returns=1) int second(bool c,int n){if(c){return n+3;}return n+4;}export {first,second};',exercise:'console.log(JSON.stringify([m.first(true,5),m.first(false,5),m.second(true,5),m.second(false,5)]));',expected:[6,7,8,9]},
 {id:'modules',source:'import {label} from "./label.lil";export string probe(string x){return label(x)+label(x+x);}',files:{'label.lil':'@choose(quote_delimiter=1) string inner(string x){return "prefix:"+x;}export string label(string x){return inner(x);}'},exercise:'console.log(JSON.stringify(m.probe("x")));',expected:'prefix:xprefix:xx'},
 {id:'modern',source:'@choose(optional_chain=1) @choose(conditional_returns=1) JsValue read(JsValue obj){if(JS.strictEqual(obj,null)||JS.isUndefined(obj)){return JS.undefined();}return obj["item"];}@choose(logical_assignment=1) bool ensure(bool flag){if(!flag){flag=next();}return flag;}extern bool next();export {read,ensure};',setup:'globalThis.next=()=>true;',exercise:'let reads=0;const obj={get item(){return ++reads}};console.log(JSON.stringify([m.read(null)===undefined,m.read(undefined)===undefined,m.read(obj),reads,m.ensure(true),m.ensure(false)]));',expected:[true,true,1,1,true,true]},
];
const unpinned = source => source.replace(/@choose\([^)]*\)\s*/g,'');
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 for(const label of ['before','after']) {
  const sourceFolder=resolve(folder,label);mkdirSync(sourceFolder,{recursive:true});
  const selected=source=>label==='before'?unpinned(source):source;
  writeFileSync(resolve(sourceFolder,'main.lil'),selected(c.source));
  for(const [name,source] of Object.entries(c.files||{}))writeFileSync(resolve(sourceFolder,name),selected(source));
 }
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,source_sha256:sha(c.source),baseline_source_sha256:sha(unpinned(c.source)),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler] of [['before',baseline],['after',binary]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[javascript]\necmascript='es2021'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const out=resolve(folder,`${label}-${lane}.mjs`);
   const result=run(compiler,[resolve(folder,label,'main.lil'),'--config',config,'--format','esm','--target','js-module','--explain','json','-o',out]);
   const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
   if(label==='after' && c.id==='modules' && !code.includes("'prefix:'"))throw Error('imported quote pin lost');
   if(label==='after' && c.id==='modern' && (!code.includes('?.') || !code.includes('||=')))throw Error('modern pin lost');
   if(label==='after' && c.id==='branches' && !code.includes('if('))throw Error('canonical return pin lost');
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
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const before=rows.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=rows.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'regional pins on branches, modern spellings and imported inlined helpers; baseline removes only unsupported @choose annotations from otherwise identical sources; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
