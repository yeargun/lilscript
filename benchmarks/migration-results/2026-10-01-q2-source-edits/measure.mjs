import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-source-edits-1/lilscript';
const previous='/home/azureuser/lilscript-work/bin/q2-delivery-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,env={})=>{const r=spawnSync(command,args,{cwd:root,env:{...process.env,...env},encoding:'utf8',timeout:300000,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[
 {id:'closed-signatures',source:'int f(int x,int spare){return x+1;}print(f(2,4));print(f(5,4));',expected:'3\n6'},
 {id:'captured-banks',source:'func()->int make(int n){int[] state=[n,0];return ()=>{state[0]+=1;return state[0];};}auto a=make(1);auto b=make(10);print(a());print(b());print(a());',expected:'2\n11\n3'},
 {id:'aggregate-aliases',source:'int mark(int n){print(n);return n;}int[] a=[mark(1),mark(2)];int[] b=a;b[0]=mark(3);print(a[0]);print(b[1]);',expected:'1\n2\n3\n3\n2'},
 {id:'conditional-returns',source:'extern bool input();int choose(bool yes){if(yes){return 3;}return 7;}print(choose(input()));print(choose(input()));',setup:'let next=false;globalThis.input=()=>next=!next;',expected:'3\n7'},
 {id:'declaration-defaults',source:'int add(int n,int m=3){return n+m;}print(add(2));print(add(4,5));',expected:'5\n9'},
 {id:'interned-strings',source:'string join(string left){return left+"b";}print(join("a"));print(join("c"));',expected:'ab\ncb'},
].map(c=>({...c,mode:'single',entries:['main'],files:{'main.lil':c.source},exercise:'await load("main");'}));
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 for(const [name,text] of Object.entries(c.files))writeFileSync(resolve(folder,name),text);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({setup:c.setup||'',exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,sources:Object.entries(c.files).map(([path,text])=>({path,sha256:sha(text)})),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const label of ['before','after']){
   const compiler=label==='before'?previous:binary,config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[delivery]\nmode='${c.mode}'\n[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
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
  if(JSON.stringify(outputs.before.files)!==JSON.stringify(outputs.after.files))throw Error(`output changed ${c.id}/${lane}`);
  row.lanes[lane]=outputs;
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,metric])=>[lane,Object.fromEntries(['before','after'].map(label=>[label,rows.reduce((n,row)=>n+row.lanes[lane][label].sizes[metric],0)]))]));
const result={compiler:identity(binary),previous:identity(previous),encoder:identity(codec),checks:36,rows,totals};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({checks:36,totals,changes:rows.map(row=>({id:row.id,objectives:Object.fromEntries(Object.entries(row.lanes).map(([lane,x])=>[lane,{judged:[x.before.search.judged,x.after.search.judged],cpu:[x.before.cpu_seconds,x.after.cpu_seconds],resources:x.after.resources}]))}))},null,2));
