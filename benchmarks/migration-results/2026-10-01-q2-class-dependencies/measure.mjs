import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-dependencies-1/lilscript';
const previous='/home/azureuser/lilscript-work/bin/q2-analysis-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,env={})=>{const r=spawnSync(command,args,{cwd:root,env:{...process.env,...env},encoding:'utf8',timeout:300000,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const shared={
 'core.lil':'int count=0;print("core loaded");export void bump(){count=count+1;}export int read(){return count;}',
 'a.lil':'import {bump,read} from "./core";export int twice(){bump();bump();return read();}export {bump,read};',
 'b.lil':'import {read} from "./core";print("b loaded");export int onlyB(){return 7;}export {read};'
};
const lazy={'feature.lil':'export int answer(int value){return value+2;}export int unused(int value){return value*99;}',
 'main.lil':'import("./feature").then((auto feature)=>print(feature.answer(40))).catch((auto error)=>print(error.message));'};
const cases=[{
 id:'delivered-setter',mode:'preserve-modules',entries:['main'],
 files:{'main.lil':'import {counter,bump,twice} from "./lib";bump();counter=counter+twice(3);print(counter);',
 'lib.lil':'extern int read();export int counter=read();export void bump(){counter=counter+read();}export int twice(int x){for(int i=0;i<2;i++){x=x+read();}return x;}'},
 setup:'globalThis.read=()=>1;',exercise:'await load("main");',expected:'7'
},{id:'shared-split',mode:'split',entries:['a','b'],files:shared,
 exercise:'const b=await load("b"),a=await load("a");console.log(b.read(),a.twice(),b.read(),a.read===b.read,b.onlyB());',expected:'core loaded\nb loaded\n0 2 2 true 7'
},{id:'shared-single',mode:'single',entries:['a','b'],files:shared,
 exercise:'const a=await load("a"),b=await load("b");console.log(a.twice(),b.read());',expected:'core loaded\ncore loaded\nb loaded\n2 0'
},...['split','preserve-modules'].map(mode=>({id:`lazy-${mode}`,mode,entries:['main'],files:lazy,
 exercise:'await load("main");await new Promise(r=>setTimeout(r,20));',expected:'42'})),{
 id:'in-place-lazy-multi',mode:'single',entries:['a','b'],files:{'feature.lil':lazy['feature.lil'],'a.lil':lazy['main.lil'],'b.lil':lazy['main.lil']},
 exercise:'await load("a");await new Promise(r=>setTimeout(r,20));await load("b");await new Promise(r=>setTimeout(r,20));',expected:'42\n42'
}];
for(const c of cases){
 c.settings="entry_names='entry/[name]-[hash:12].[ext]'\nchunk_names='chunks/[name]-[index]-[hash:12].[ext]'\nmodule_names='modules/[path]-[hash:12].[ext]'\npreload='"+(c.mode==='preserve-modules'?'entry':'all')+"'\n";
}
cases.push({id:'numeric-render',mode:'single',entries:['main'],
 files:{'main.lil':'extern void emit(float value);export void run(){emit(0.00015);emit(1.00375);emit(1.7976931348623157e308);emit(-0.7);}'},
 setup:'globalThis.emit=console.log;',exercise:'const main=await load("main");main.run();',expected:'0.00015\n1.00375\n1.7976931348623157e+308\n-0.7'});
for(const mode of ['single','preserve-modules']) cases.push({id:'embedded-host-'+mode,mode,entries:['main'],
 settings:"host_modules='embed'\nentry_names='entry/[name]-[hash:12].[ext]'\nmodule_names='modules/[path]-[hash:12].[ext]'\n",
 files:{'main.lil':'import extern {add} from "./host.js";extern int add(int left,int right);export int answer(){return add(20,22);}',
 'host.js':'import {sum} from "./math.js";export function add(left,right){return sum(left,right)}',
 'math.js':'export function sum(left,right){return left+right}'},
 exercise:'const main=await load("main");console.log(main.answer());',expected:'42'});
// Limit this owner check to affected semantic and delivery boundaries.
for (let i=cases.length-1;i>=0;i--) if(!['delivered-setter','shared-split','lazy-preserve-modules','embedded-host-single'].includes(cases[i].id)) cases.splice(i,1);
cases.push({id:'pure-ranges',mode:'single',entries:['main'],
 files:{'main.lil':'extern int input();pure int bound(int n){int sum=0;for(int i=0;i<8;i++){if(i<n){sum+=i;}}return sum;}int seed=input();export int answer(int n){return bound(n)+seed;}print(answer(4));'},
 setup:'globalThis.input=()=>2;',exercise:'const main=await load("main");console.log(main.answer(7));',expected:'8\n23'});
cases.push({id:'early-read',mode:'single',entries:['main'],
 files:{'main.lil':'int early=read();int value=4;export int read(){return value;}print(early);'},
 exercise:'try{await load("main");console.log("missed")}catch(error){console.log(error.name)}',expected:'ReferenceError'});
const rows=[];
for(const c of cases){
 const folder=resolve('/tmp/lilscript-q2-dependencies-artifacts',c.id);mkdirSync(folder,{recursive:true});
 for(const [name,text] of Object.entries(c.files))writeFileSync(resolve(folder,name),text);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({setup:c.setup||'',exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,sources:Object.entries(c.files).map(([path,text])=>({path,sha256:sha(text)})),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const label of ['before','after','cold']){
   const compiler=label==='before'?previous:binary,config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[delivery]\nmode='${c.mode}'\n${c.settings||''}[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   if(label==='cold')writeFileSync(config,readFileSync(config,'utf8')+'[cache]\nnormalization_reuse=false\n');
   const output=resolve(folder,`${label}-${lane}`);rmSync(output,{recursive:true,force:true});
   const args=c.entries.flatMap(name=>['--entry',`${name}=${resolve(folder,name+'.lil')}`]);
   const timing=resolve(folder,`${label}-${lane}.time`);
   const result=run('/usr/bin/time',['-f','%U %S %e','-o',timing,compiler,...args,'--config',config,'--format','esm','--target','js-module','--explain','json','--out-dir',output],{LILSCRIPT_TIMING:'1'});
   const [user,system,elapsed]=readFileSync(timing,'utf8').trim().split(/\s+/).map(Number);rmSync(timing);
   const split=result.stderr.lastIndexOf('lilscript-timing ');
   const phase=JSON.parse(result.stderr.slice(split+'lilscript-timing '.length));
   const receipt=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'),split).trim());
   const manifest=JSON.parse(readFileSync(resolve(output,'lilscript.manifest.json'),'utf8')).outputs[0];
   const files=manifest.files.map(file=>{const path=resolve(output,file.file),sizes=JSON.parse(run(codec,['--json',path]).stdout).artifacts[0];
    if(sizes[metric]!==file.codec_bytes||sha(readFileSync(path))!==file.sha256)throw Error('manifest differs from independent encoder');
    return {name:file.file,sha256:file.sha256,raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11};});
   const sizes=Object.fromEntries(Object.values(metrics).map(key=>[key,manifest.entries.reduce((total,entry)=>total+entry.closure.reduce((n,name)=>n+files.find(file=>file.name===name)[key],0),0)]));
   const terminal=receipt.search.terminal.objectives[0];
   if(terminal.after!==sizes[metric]||manifest.rows_total!==sizes[metric])throw Error(`score mismatch ${c.id}/${lane}/${label}`);
   writeFileSync(resolve(output,'package.json'),'{"type":"module"}\n');
   const urls=Object.fromEntries(c.entries.map(name=>[name,pathToFileURL(resolve(output,manifest.entries.find(entry=>entry.name===name&&!entry.dynamic).file)).href]));
   const observed=run('node',['--input-type=module','-e',`${c.setup||''}const urls=${JSON.stringify(urls)};const load=name=>import(urls[name]);${c.exercise}`]).stdout.trim();
   if(observed!==c.expected)throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={files,sizes,resources:receipt.resources,search:{structures:receipt.search.structures,judged:terminal.judged,tried:terminal.tried,scored:terminal.scored,heads:terminal.heads},cpu_seconds:user+system,wall_seconds:elapsed,timing:phase};
  }
  writeFileSync(resolve(directory,`${c.id}-${lane}-samples.json`),JSON.stringify(outputs,null,2)+'\n');
  for(const key of ['files','sizes','resources','search'])if(JSON.stringify(outputs.cold[key])!==JSON.stringify(outputs.after[key]))throw Error(`reuse changed ${c.id}/${lane}/${key}`);
  outputs.output_changed=JSON.stringify(outputs.before.files)!==JSON.stringify(outputs.after.files);
  if((outputs.cold.timing?.source_effect_reuse_calls||0)!==0)throw Error('effect reuse veto ignored');
  row.lanes[lane]=outputs;
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,metric])=>[lane,Object.fromEntries(['before','after'].map(label=>[label,rows.reduce((n,row)=>n+row.lanes[lane][label].sizes[metric],0)]))]));
const result={compiler:identity(binary),previous:identity(previous),encoder:identity(codec),checks:cases.length*9,rows,totals};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({checks:cases.length*9,totals,changes:rows.map(row=>({id:row.id,objectives:Object.fromEntries(Object.entries(row.lanes).map(([lane,x])=>[lane,{judged:[x.before.search?.judged,x.after.search?.judged],cpu:[x.before.cpu_seconds,x.after.cpu_seconds],work:[x.before.resources?.work,x.after.resources?.work]}]))}))},null,2));
