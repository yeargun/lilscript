// Affected development cases, not held-out or fleet qualification.
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)), root=resolve(directory,'../../..');
const binaries={g2:'/home/azureuser/lilscript-work/bin/g2-complete-1/lilscript',g3:'/home/azureuser/lilscript-work/bin/g3-complete-2/lilscript'};
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const run=(command,args,input)=>{const result=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:600000,maxBuffer:64*1024*1024});if(result.status!==0)throw Error(`${command}: ${result.stderr||result.error}`);return result;};
const config=(lane,level=13)=>`[optimization]\npreset='maximum'\ninlining=false\nconstant_folding=false\nscalar_replacement=false\n[javascript]\npriority='size-first'\necmascript='es2021'\nstrip_debug=false\nstrip_console_calls=false\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=${level}\n[policy.tactics]\nstring-pooling='off'\nhelper-sharing='off'\ncall-specialization='off'\n`;
const compile=(binary,entry,configPath,output,format='esm',target='js-module')=>{
 const result=run(binary,[entry,'--config',configPath,'--format',format,'--target',target,'--explain','json','-o',output]);
 const code=readFileSync(output,'utf8'),sizes=JSON.parse(run(codec,['--json',output]).stdout).artifacts[0];
 const search=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
 return {code,metrics:{sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}},judgedBytes:t.after};
};
const importCode=code=>`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));`;
const check=(code,exercise,expected)=>{
 const actual=run('node',['--input-type=module'],importCode(code)+exercise).stdout.trim();
 if(actual!==JSON.stringify(expected))throw Error(`oracle mismatch: ${actual.slice(0,200)}`);
};
const inputs=[-9,0,7,31];
const cases=[
 {id:'spellings',source:`extern void observe(int n);${Array.from({length:6},(_,i)=>`export int select${i}(bool flag,int n){if(flag){return n+${i+1};}else{return n-${i+1};}}`).join('')}export int loops(int n){int total=0;for(int i=0;i<n;i+=1){total+=i;}if(n>0){observe(total);}if(n==0){observe(99);}return total;}export string quotes(){return ${JSON.stringify('"quoted" "again"')};}`,
  exercise:`let events=[];globalThis.observe=n=>events.push(n);console.log(JSON.stringify([${Array.from({length:6},(_,i)=>`m.select${i}(true,7),m.select${i}(false,7)`).join(',')},m.loops(7),m.loops(0),events,m.quotes()]));`,
  expected:[...Array.from({length:6},(_,i)=>[8+i,6-i]).flat(),21,0,[21,99],'"quoted" "again"']},
 {id:'receiver',source:`class Box{int amount;init(int n){this.amount=n;}int total(){return ${Array(12).fill('this.amount').join('+')};}}export constructor Box;`,
  exercise:`const box=new m.Box(7);const a=box.total();let reads=0;Object.defineProperty(box,'amount',{get(){return ++reads;}});console.log(JSON.stringify([a,box.total(),reads,box.total(),reads]));`,expected:[84,78,12,222,24]},
 {id:'modern',source:`extern bool next();export JsValue read(JsValue obj){if(JS.strictEqual(obj,null)||JS.isUndefined(obj)){return JS.undefined();}return obj["item"];}export JsValue call(JsValue obj){JsValue callback;if(JS.strictEqual(obj,null)||JS.isUndefined(obj)){callback=JS.undefined();}else{callback=obj["method"];}return callback();}export bool ensure(bool flag){if(!flag){flag=next();}return flag;}`,
  exercise:`let next=0,reads=0;globalThis.next=()=>{next++;return true;};const obj={get item(){return ++reads;},method(){return this===undefined;}};console.log(JSON.stringify([m.read(null)===undefined,m.read(undefined)===undefined,m.read(obj),reads,m.call(obj),m.ensure(true),m.ensure(false),next]));`,expected:[true,true,1,1,true,true,true,1]},
];
// Independent deterministic data: 32 clusters repeat beyond the 32 KiB window.
const strings=Array.from({length:32},(_,i)=>{let x=i+79;return Array.from({length:1200},()=>{x=(Math.imul(x,1664525)+1013904223)>>>0;return 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'[x%62];}).join('');});
const localitySource='export string run(int n){'+Array.from({length:64},(_,i)=>`auto f${i}=(int x)=>{return "${strings[i%32]}"+JS.string(x+${i});};`).join('')+'return '+Array.from({length:64},(_,i)=>`f${i}(n)`).join('+')+';}';
const localityExpected=sha(Array.from({length:64},(_,i)=>strings[i%32]+(i+7)).join(''));
cases.push({id:'gzip-window-locality',source:localitySource,level:14,exercise:`const {createHash}=await import('node:crypto');console.log(JSON.stringify(createHash('sha256').update(m.run(7)).digest('hex')));`,expected:localityExpected});
const rows=[];
for(const item of cases){
 const folder=resolve(directory,'artifacts',item.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,item.source+'\n');
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:item.exercise,expected:item.expected})+'\n');
 const row={id:item.id,level:item.level||13,source_sha256:sha(item.source+'\n'),lanes:{}};
 for(const lane of ['raw','gzip','brotli']){
  const settings=config(lane,item.level),configPath=resolve(folder,lane+'.toml');writeFileSync(configPath,settings);row.lanes[lane]={config_sha256:sha(settings)};
  for(const [version,binary]of Object.entries(binaries)){
   const output=resolve(folder,`${version}-${lane}.mjs`),result=compile(binary,entry,configPath,output);
   check(result.code,item.exercise,item.expected);
   const metric={raw:'raw',gzip:'gzip9',brotli:'brotli11'}[lane];
   if(result.judgedBytes!==result.metrics[metric])throw Error('scored/delivered mismatch');
   row.lanes[lane][version]=result.metrics;
   console.log(`checked ${item.id}/${lane}/${version}`);
  }
 }
 rows.push(row);
 writeFileSync(resolve(directory,'partial.json'),JSON.stringify(rows,null,2)+'\n');
}
// Retain G2's six property-boundary checks without rebuilding its known artifacts.
const previous=resolve(directory,'../2026-10-01-g2-complete'),prior=JSON.parse(readFileSync(resolve(previous,'comparison.json')));
for(const before of prior.rows){
 const folder=resolve(directory,'artifacts','g2-'+before.id+'-'+before.boundary);mkdirSync(folder,{recursive:true});
 const inputFolder=resolve(previous,'artifacts',before.id+'-'+before.boundary),entry=resolve(inputFolder,'main.lil');
 const row={id:'g2-'+before.id+'-'+before.boundary,source_sha256:before.source_sha256,level:13,lanes:{}};
 const oracle=JSON.parse(readFileSync(resolve(inputFolder,'oracle.json')));
 for(const lane of ['raw','gzip','brotli']){
  const output=resolve(folder,'g3-'+lane+'.mjs'),result=compile(binaries.g3,entry,resolve(inputFolder,lane+'.toml'),output,before.boundary==='open'?'esm':'iife',before.boundary==='open'?'js-module':'js');
  if(result.judgedBytes!==result.metrics[{raw:'raw',gzip:'gzip9',brotli:'brotli11'}[lane]])throw Error('G2 scored/delivered mismatch');
  if(before.boundary==='closed'){
   const actual=run('node',['--input-type=module'],`let i=0;globalThis.input=()=>${JSON.stringify(oracle.inputs)}[i++];`+result.code).stdout;
   if(actual!==oracle.expected.join('\n')+'\n')throw Error('G2 numeric oracle');
  }else{
   check(result.code,`console.log(JSON.stringify([${JSON.stringify(oracle.inputs)}.map(n=>m.compute(n)),m.expose()]));`,[oracle.expected,oracle.shape]);
  }
  row.lanes[lane]={config_sha256:before.lanes[lane].config_sha256,g2:before.lanes[lane].g2,g3:result.metrics};
 }
 rows.push(row);console.log(`checked ${row.id}`);
}
const total=cohort=>Object.fromEntries([['raw','raw'],['gzip','gzip9'],['brotli','brotli11']].map(([lane,key])=>{
 const before=cohort.reduce((s,r)=>s+r.lanes[lane].g2[key],0),after=cohort.reduce((s,r)=>s+r.lanes[lane].g3[key],0);return[lane,{before,after,delta:after-before}];
}));
const identity=path=>({path,sha256:sha(readFileSync(path))});
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'four G3 affected development workloads and six retained G2 boundaries; all three objectives; exact bytes and independent oracles; not fleet/timing evidence',compilers:Object.fromEntries(Object.entries(binaries).map(([v,p])=>[v,identity(p)])),codec:identity(codec),totals:{g3:total(rows.slice(0,4)),g2:total(rows.slice(4)),all:total(rows)},rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binaries.g3),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(total(rows)));
