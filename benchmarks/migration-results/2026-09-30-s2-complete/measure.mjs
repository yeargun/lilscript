// Focused S1/S2 comparison, independent objectives at default effort 13.
// Usage: node benchmarks/migration-results/2026-09-30-s2-complete/measure.mjs
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {loadCorpus} from '../../../scripts/ratchet.mjs';
const directory=dirname(fileURLToPath(import.meta.url));
const root=resolve(directory,'../../..');
const binaries={s1:'/home/azureuser/lilscript-work/bin/s1-complete-4/lilscript',s2:'/home/azureuser/lilscript-work/bin/s2-complete-2/lilscript'};
const codec='/home/azureuser/lilscript-work/bin/s2-complete-2/lilscript-codec';
const sha=text=>createHash('sha256').update(text).digest('hex');
const run=(command,args,input)=>{const result=spawnSync(command,args,{cwd:root,input,encoding:'utf8',maxBuffer:64*1024*1024,timeout:60000});if(result.status!==0)throw Error(`${command} ${args.join(' ')}: ${result.stderr||result.error}`);return result;};
const wanted=[
'cases/canonical/aggregates/nested-struct','cases/canonical/aggregates/struct-point',
'cases/canonical/collections/record-json','cases/canonical/functions/closure-capture',
'cases/canonical/identity/snapshot-captured-rebind','cases/catalog/aggregate/struct-pairs-3-1-4-1-5-9',
'cases/catalog/aggregate/nested-struct-3-1-4-1-5-9','cases/catalog/aggregate/record-transform-2',
'cases/catalog/string/string-concat-scalar-replace','cases/catalog/string/string-concat-struct-layout',
'cases/catalog/aggregate/frontier-default-constructor','cases/catalog/closure/closure-capture-3'];
const {items}=await loadCorpus(['cases']);
const selected=wanted.map(id=>{const item=items.find(x=>x.id===id);if(!item)throw Error(id);return item;});
selected.push(
{id:'s2/array-alias',lil:'extern int input();int[] state=[input(),2];int[] alias=state;auto step=()=>{alias[0]+=alias[1];return alias[0];};print(step());print(step());print(state[1]);',js:'let state=[input(),2],alias=state;const step=()=>{alias[0]+=alias[1];return alias[0]};console.log(step());console.log(step());console.log(state[1]);',setup:'globalThis.input=()=>7;'},
{id:'s2/owned-product',lil:'extern int input();struct P{int x;int y;}struct Box{P point;int z;}Box state=Box{P{input(),2},3};for(int i=0;i<4;i+=1){if(i==2){state.point.x+=state.z;}else{state.point.y+=state.point.x;}}print(state.point.x);print(state.point.y);',js:'let state={point:{x:input(),y:2},z:3};for(let i=0;i<4;i++){if(i===2)state.point.x+=state.z;else state.point.y+=state.point.x;}console.log(state.point.x);console.log(state.point.y);',setup:'globalThis.input=()=>7;'},
{id:'s2/namespace',lil:'extern int input();JsValue add=(JsValue n)=>JS.add(n,1);JsValue ns=JS.object("add",add,"scale",3);auto use=(int n)=>JS.invoke(ns,"add",n);print(use(input()));print(ns["scale"]);',js:'const ns={add:n=>n+1,scale:3};const use=n=>ns.add(n);console.log(use(input()));console.log(ns.scale);',setup:'globalThis.input=()=>7;'}
);
const rows=[];
for(const item of selected){
 const folder=resolve(directory,'artifacts',item.id);mkdirSync(folder,{recursive:true});
 const source=item.lil??readFileSync(item.entry,'utf8');
 const entry=resolve(folder,'main.lil');writeFileSync(entry,source);
 writeFileSync(resolve(folder,'oracle.js'),(item.setup??'')+item.js);
 const expected=run('node',['--input-type=commonjs'],(item.setup??'')+item.js).stdout;
 const row={id:item.id,source_sha256:sha(source),oracle_sha256:sha((item.setup??'')+item.js),expected,lanes:{}};
 for(const lane of ['raw','gzip','brotli']){
  row.lanes[lane]={};
  for(const [version,binary] of Object.entries(binaries)){
   const output=resolve(folder,`${version}-${lane}.js`);
   const compiled=run(binary,[entry,'--config',resolve(directory,'configs',lane+'.toml'),'--format','iife','--target','js','--explain','json','-o',output]);
   const code=readFileSync(output,'utf8');
   const actual=run('node',['--input-type=commonjs'],(item.setup??'')+code).stdout;
   if(actual!==expected)throw Error(`Behavior mismatch: ${item.id}/${version}/${lane}`);
   const sizes=JSON.parse(run(codec,['--json',output]).stdout).artifacts[0];
   const explanation=JSON.parse(compiled.stderr.slice(compiled.stderr.indexOf("{\n")));
   row.lanes[lane][version]={sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,rules:explanation.phases_ns?.rules??explanation.rules??null};
  }
 }
 rows.push(row);console.log(`checked ${item.id}`);
}
const totals=Object.fromEntries([['raw','raw'],['gzip','gzip9'],['brotli','brotli11']].map(([lane,metric])=>{const before=rows.reduce((s,r)=>s+r.lanes[lane].s1[metric],0),after=rows.reduce((s,r)=>s+r.lanes[lane].s2[metric],0);return[lane,{before,after,delta:after-before}]}));
const report={scope:'15 focused development programs, not fleet or full-ratchet qualification',format:'iife',effort:13,compilers:Object.fromEntries(Object.entries(binaries).map(([key,path])=>[key,{path,sha256:sha(readFileSync(path))}])),codec:{path:codec,sha256:sha(readFileSync(codec))},configurations:Object.fromEntries(['raw','gzip','brotli'].map(lane=>[lane,sha(readFileSync(resolve(directory,'configs',lane+'.toml')))])),totals,rows};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(totals,null,2));
