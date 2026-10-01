// Focused development evidence. Each codec chooses and measures its own output.
import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {loadCorpus} from '../../../scripts/ratchet.mjs';
const directory=dirname(fileURLToPath(import.meta.url)), root=resolve(directory,'../../..');
const binaries={s2:'/home/azureuser/lilscript-work/bin/s2-complete-2/lilscript',s3:'/home/azureuser/lilscript-work/bin/s3-complete-2/lilscript'};
const codec='/home/azureuser/lilscript-work/bin/s3-complete-2/lilscript-codec';
const sha=text=>createHash('sha256').update(text).digest('hex');
const run=(command,args,input)=>{const result=spawnSync(command,args,{cwd:root,input,encoding:'utf8',maxBuffer:64*1024*1024,timeout:60000});if(result.status!==0)throw Error(`${command} ${args.join(' ')}: ${result.stderr||result.error}`);return result;};
// Keep work counts, selected assignments and challenger outcomes; omit bulky
// per-trial naming plans from the retained summary.
function compactSearch(search){
 if(!search)return null;
 const terminal={objectives:search.terminal.objectives.map(row=>{
  const {trials,choice_trials,joint_trials,starts,surveys,...counts}=row;
  return {...counts,challengers:trials.map(({challenger,outcome,delta})=>({challenger,outcome,delta}))};
 })};
 return {...search,terminal};
}
const wanted=['cases/canonical/functions/closure-capture','cases/canonical/functions/default-args','cases/canonical/functions/identical-helpers','cases/canonical/functions/nested-local','cases/catalog/function/factorial-8','cases/catalog/function/identical-helpers-5','cases/catalog/closure/closure-capture-3','cases/canonical/identity/snapshot-captured-rebind'];
const {items}=await loadCorpus(['cases']);
const selected=wanted.map(id=>{const item=items.find(x=>x.id===id);if(!item)throw Error(id);return item;});
selected.push(
 {id:'s3/captured-activations',lil:'func()->int make(int n){return ()=>{n+=1;return n;};}func()->int first=()=>0;func()->int last=()=>0;for(int i=0;i<2;i+=1){auto next=make(i*10);if(i==0){first=next;}else{last=next;}}print(first());print(last());print(first());',js:'const make=n=>()=>++n;let first,last;for(let i=0;i<2;i++){const next=make(i*10);if(i===0)first=next;else last=next;}console.log(first());console.log(last());console.log(first());'},
 {id:'s3/guard-return',lil:'extern bool input();int mark(int n){print(n);return n;}int choose(bool c){if(c){return mark(1);}return mark(2);}int a=choose(input());int b=choose(input());print(a);print(b);',js:'const mark=n=>(console.log(n),n);const choose=c=>c?mark(1):mark(2);const a=choose(input()),b=choose(input());console.log(a);console.log(b);',setup:'{let calls=0;globalThis.input=()=>++calls===1;}'},
 {id:'s3/waiting-product',lil:readFileSync(resolve(root,'tests/cases/interprocedural_values.lil'),'utf8'),js:'let total=0;for(let n=-12;n<=12;n++){const box={value:n%10};total+=box.value+5+box.value+1;}console.log(total);'},
 {id:'s3/mutating-argument',lil:'struct P{int x;int y;}P p=P{1,2};int change(){p.x=9;return 3;}int read(P q,int n){return q.x+q.y+n;}print(read(p,change()));print(p.x);',js:'let p={x:1,y:2};const read=(q,n)=>q.x+q.y+n;const change=()=>{p={...p,x:9};return 3;};console.log(read({...p},change()));console.log(p.x);'},
 {id:'s3/cross-module',lil:'import {add} from "./math";int answer=add(5);print(answer);',dependencies:{'math.lil':'int base=7;export int add(int n){return base+n;}'},js:'const base=7;const add=n=>base+n;console.log(add(5));'},
 {id:'s3/helper-sharing-opt-in',group:'explicit-sharing',extra:"[policy.tactics]\ninlining='off'\nhelper-sharing='on'\n",lil:'extern int input();int a(int x){return x*x+x*3+17;}int b(int n){return n*n+n*3+29;}int n=input();print(a(n));print(b(n+1));print(a(n+2));print(b(n+3));',js:'const a=x=>x*x+x*3+17,b=n=>n*n+n*3+29;const n=input();console.log(a(n));console.log(b(n+1));console.log(a(n+2));console.log(b(n+3));',setup:'globalThis.input=()=>3;'},
 {id:'s3/constant-signature',extra:"[policy.tactics]\ninlining='off'\n",lil:'extern int input();int scale(int n,int k){return n*k+k;}int shift(int n,int k){return n+k;}int n=input();print(scale(n,7));print(scale(n+1,7));print(shift(n,3));print(shift(n+1,4));',js:'const scale=(n,k)=>n*k+k,shift=(n,k)=>n+k;const n=input();console.log(scale(n,7));console.log(scale(n+1,7));console.log(shift(n,3));console.log(shift(n+1,4));',setup:'globalThis.input=()=>3;'}
);
const rows=[];
for(const item of selected){
 const folder=resolve(directory,'artifacts',item.id);mkdirSync(folder,{recursive:true});
 const source=item.lil??readFileSync(item.entry,'utf8'),entry=resolve(folder,'main.lil');writeFileSync(entry,source);
 for(const [name,text] of Object.entries(item.dependencies??{}))writeFileSync(resolve(folder,name),text);
 const oracle=(item.setup??'')+item.js;writeFileSync(resolve(folder,'oracle.js'),oracle);
 const expected=run('node',['--input-type=commonjs'],oracle).stdout;
 const row={id:item.id,group:item.group??'ordinary-permissions',source_sha256:sha(source),dependency_sha256:Object.fromEntries(Object.entries(item.dependencies??{}).map(([name,text])=>[name,sha(text)])),oracle_sha256:sha(oracle),expected,lanes:{}};
 for(const lane of ['raw','gzip','brotli']){
  row.lanes[lane]={};
  const baseConfig=readFileSync(resolve(directory,'configs',lane+'.toml'),'utf8');
  const config=(item.extra?.includes("inlining='off'") ? baseConfig.replace('inlining = true','inlining = false') : baseConfig)+'\n'+(item.extra??'');
  const configPath=resolve(folder,lane+'.toml');writeFileSync(configPath,config);
  row.lanes[lane].config_sha256=sha(config);
  for(const [version,binary] of Object.entries(binaries)){
   const output=resolve(folder,`${version}-${lane}.js`);
   const compiled=run(binary,[entry,'--config',configPath,'--format','iife','--target','js','--explain','json','-o',output]);
   const code=readFileSync(output,'utf8'),actual=run('node',['--input-type=commonjs'],(item.setup??'')+code).stdout;
   if(actual!==expected)throw Error(`Behavior mismatch: ${item.id}/${version}/${lane}: ${JSON.stringify({expected,actual})}`);
   const sizes=JSON.parse(run(codec,['--json',output]).stdout).artifacts[0];
   const explanation=JSON.parse(compiled.stderr.slice(compiled.stderr.indexOf('{\n')));
   row.lanes[lane][version]={sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,rules:explanation.phases_ns?.rules??null,search:compactSearch(explanation.search)};
  }
 }
 writeFileSync(resolve(folder,'observations.json'),JSON.stringify(row,null,2)+'\n');
 rows.push(row);console.log(`checked ${item.id}`);
}
const totals=Object.fromEntries(['ordinary-permissions','explicit-sharing'].map(group=>[group,Object.fromEntries([['raw','raw'],['gzip','gzip9'],['brotli','brotli11']].map(([lane,metric])=>{const cohort=rows.filter(r=>r.group===group);const before=cohort.reduce((s,r)=>s+r.lanes[lane].s2[metric],0),after=cohort.reduce((s,r)=>s+r.lanes[lane].s3[metric],0);return[lane,{before,after,delta:after-before}];}))]));
const compilerIdentity=path=>({path,sha256:sha(readFileSync(path))});
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'15 development programs; ordinary permissions and recurring-sharing opt-in reported separately; not fleet or timing qualification',format:'iife',effort:13,compilers:Object.fromEntries(Object.entries(binaries).map(([version,path])=>[version,compilerIdentity(path)])),codec:compilerIdentity(codec),totals,rows},null,2)+'\n');
const sourcePaths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(path=>path.endsWith('.rs')).map(path=>'src/'+path)].sort();
const source_files=sourcePaths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order',compiler:compilerIdentity(binaries.s3),codec:compilerIdentity(codec)},null,2)+'\n');
console.log(JSON.stringify(totals,null,2));
