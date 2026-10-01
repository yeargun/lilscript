import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-decisions-1/lilscript';
const previous='/home/azureuser/lilscript-work/bin/q2-build-cache-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,env={})=>{const r=spawnSync(command,args,{cwd:root,env:{...process.env,...env},encoding:'utf8',timeout:300000,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'independent-bodies-128',source:Array.from({length:128},(_,i)=>`export int f${i}(int n){if(n>0){return n+${i};}else{return n-${i};}}`).join(''),
 exercise:'console.log(JSON.stringify([m.f0(1),m.f127(-1)]));',expected:'[1,-128]'
},{
 id:'spellings',source:readFileSync(resolve(root,'tests/cases/objective_judged_spellings.lil'),'utf8'),
 expected:readFileSync(resolve(root,'tests/cases/objective_judged_spellings.out'),'utf8').trim()
},{
 id:'delivered-setter',source:'import {counter,bump,twice} from "./lib";bump();counter=counter+twice(3);print(counter);',
 files:{'lib.lil':'extern int read();export int counter=read();export void bump(){counter=counter+read();}export int twice(int x){for(int i=0;i<2;i++){x=x+read();}return x;}'},
 config:"[delivery]\nmode='preserve-modules'\n",setup:'globalThis.read=()=>1;',expected:'7',delivery:true
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 for(const [name,text] of Object.entries(c.files||{}))writeFileSync(resolve(folder,name),text);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({setup:c.setup||'',exercise:c.exercise||'',expected:c.expected},null,2)+'\n');
 const row={id:c.id,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={},lock=resolve(folder,`${lane}.choices.lock`);
  for(const label of ['before','off','save','replay']){
   const compiler=label==='before'?previous:binary, config=resolve(folder,`${label}-${lane}.toml`);
   const decisions=label==='save'?`[decisions]\nwrite=${JSON.stringify(lock)}\n`:label==='replay'?`[decisions]\nread=${JSON.stringify(lock)}\n`:'';
   writeFileSync(config,`${c.config||''}\n${decisions}[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const output=resolve(folder,`${label}-${lane}${c.delivery?'':'.mjs'}`);
   if(c.delivery)rmSync(output,{recursive:true,force:true});
   const args=[entry,'--config',config,'--format','esm','--target','js-module'];
   const timing=resolve(folder,`${label}-${lane}.time`);
   const result=run('/usr/bin/time',['-f','%U %S %e','-o',timing,compiler,...args,'--explain','json',c.delivery?'--out-dir':'-o',output],{LILSCRIPT_TIMING:'1'});
   const [user,system,elapsed]=readFileSync(timing,'utf8').trim().split(/\s+/).map(Number);rmSync(timing);
   const split=result.stderr.lastIndexOf('lilscript-timing ');
   const phase=JSON.parse(result.stderr.slice(split+'lilscript-timing '.length));
   const receipt=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'),split).trim());
   const paths=c.delivery?readdirSync(output,{recursive:true}).filter(n=>/\.(m?js)$/.test(n)).sort().map(n=>resolve(output,n)):[output];
   const files=paths.map(path=>{const sizes=JSON.parse(run(codec,['--json',path]).stdout).artifacts[0];return {name:c.delivery?path.slice(output.length+1):'main.mjs',sha256:sha(readFileSync(path)),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11};});
   const sizes=Object.fromEntries(Object.values(metrics).map(key=>[key,files.reduce((n,file)=>n+file[key],0)]));
   const terminal=receipt.search.terminal.objectives[0];
   if(terminal.after!==sizes[metric])throw Error(`scored/delivered mismatch ${c.id}/${lane}/${label}`);
   let load;
   if(c.delivery){writeFileSync(resolve(output,'package.json'),'{"type":"module"}\n');const main=paths.find(path=>/\/main\.(m?js)$/.test(path));load=`await import(${JSON.stringify(pathToFileURL(main).href)})`;}
   else load=`await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(readFileSync(output,'utf8'))}))`;
   const observed=run('node',['--input-type=module','-e',`${c.setup||''}const m=${load};${c.exercise||''}`]).stdout.trim();
   if(observed!==c.expected)throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   const logical={baseline:receipt.resources.baseline_work,optional:receipt.resources.optional_work,codec:receipt.resources.codec_work};
   outputs[label]={files,sizes,logical,search:{structures:receipt.search.structures,judged:terminal.judged,tried:terminal.tried,scored:terminal.scored,heads:terminal.heads,decision:terminal.starts.find(s=>s.name==='decision-lock')||null,decision_error:terminal.decision_error||null},decisions:receipt.decisions||null,cpu_seconds:user+system,wall_seconds:elapsed,timing:phase};
   if(label==='replay'&&(receipt.decisions?.read!=='matched'||terminal.decision_error))throw Error(`lock not replayed ${c.id}/${lane}`);
  }
  for(const label of ['off','save'])if(JSON.stringify(outputs[label].files)!==JSON.stringify(outputs.before.files))throw Error(`unrequested output change ${c.id}/${lane}/${label}`);
  if(JSON.stringify(outputs.off.logical)!==JSON.stringify(outputs.save.logical))throw Error('saving changes logical work');
  if(outputs.replay.sizes[metric]>outputs.save.sizes[metric])throw Error('matching replay lost the saved incumbent');
  row.lanes[lane]=outputs;
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,metric])=>[lane,Object.fromEntries(['before','off','save','replay'].map(label=>[label,rows.reduce((n,r)=>n+r.lanes[lane][label].sizes[metric],0)]))]));
const result={compiler:identity(binary),previous:identity(previous),encoder:identity(codec),checks:36,rows,totals};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({checks:36,totals,changes:rows.map(r=>({id:r.id,objectives:Object.fromEntries(Object.entries(r.lanes).map(([lane,x])=>[lane,{judged:[x.off.search.judged,x.replay.search.judged],cpu:[x.off.cpu_seconds,x.save.cpu_seconds,x.replay.cpu_seconds],decision:x.replay.search.decision?.outcome}]))}))},null,2));
