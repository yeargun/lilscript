import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-normalization-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/q2-codec-cache-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,env={})=>{const r=spawnSync(command,args,{cwd:root,env:{...process.env,...env},encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'spellings',source:readFileSync(resolve(root,'tests/cases/objective_judged_spellings.lil'),'utf8'),
 expected:readFileSync(resolve(root,'tests/cases/objective_judged_spellings.out'),'utf8').trim()
},{
 id:'local-returns',source:'export int first(int n){if(n>0){return n+1;}else{return n-1;}}export int second(int n){return n+2;}',
 exercise:'console.log(JSON.stringify([m.first(2),m.first(-2),m.second(3)]));',expected:'[3,-3,5]'
},{
 id:'captured-activations',source:'func()->int make(int n){return ()=>{n+=1;return n;};}func()->int first=()=>0;func()->int last=()=>0;for(int i=0;i<2;i+=1){auto next=make(i*10);if(i==0){first=next;}else{last=next;}}print(first());print(last());print(first());',expected:'1\n11\n2'
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
  const outputs={};
  for(const [label,compiler] of [['before',baseline],['reused',binary],['disabled',binary]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   const cache=label==='before'?'':`[cache]\nnormalization_reuse=${label==='reused'}\n`;
   writeFileSync(config,`${c.config||''}\n${cache}[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const output=resolve(folder,`${label}-${lane}${c.delivery?'':'.mjs'}`);
   if(c.delivery)rmSync(output,{recursive:true,force:true});
   const args=[entry,'--config',config,'--format','esm','--target','js-module'];
   const policy=JSON.parse(run(compiler,[...args,'--print-policy']).stdout);
   if(label!=='before' && policy.resolution.cache.normalization_reuse!==(label==='reused'))throw Error('normalization TOML not applied');
   const timing=resolve(folder,`${label}-${lane}.time`);
   const result=run('/usr/bin/time',['-f','%U %S %e','-o',timing,compiler,...args,'--explain','json',c.delivery?'--out-dir':'-o',output],{LILSCRIPT_TIMING:'1'});
   const [user,system,elapsed]=readFileSync(timing,'utf8').trim().split(/\s+/).map(Number);rmSync(timing);
   const split=result.stderr.lastIndexOf('lilscript-timing ');
   const phase=JSON.parse(result.stderr.slice(split+'lilscript-timing '.length));
   const receipt=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'),split).trim());
   const paths=c.delivery?readdirSync(output,{recursive:true}).filter(n=>/\.(m?js)$/.test(n)).sort().map(n=>resolve(output,n)):[output];
   const files=paths.map(path=>{
    const sizes=JSON.parse(run(codec,['--json',path]).stdout).artifacts[0];
    return {name:c.delivery?path.slice(output.length+1):'main.mjs',sha256:sha(readFileSync(path)),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11};
   });
   const sizes=Object.fromEntries(Object.values(metrics).map(key=>[key,files.reduce((n,file)=>n+file[key],0)]));
   const search=receipt.search,t=search.terminal.objectives[0];
   if(t.after!==sizes[metric])throw Error(`delivered/scored mismatch ${c.id}/${lane}/${label}`);
   const resources=receipt.resources;
   const logical={baseline:resources.baseline_work,optional:resources.optional_work,codec:resources.codec_work};
   let load;
   if(c.delivery){
    writeFileSync(resolve(output,'package.json'),'{"type":"module"}\n');
    const main=paths.find(path=>/\/main\.(m?js)$/.test(path));
    if(!main)throw Error(`no main file: ${paths}`);
    load=`await import(${JSON.stringify(pathToFileURL(main).href)})`;
   }else load=`await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(readFileSync(output,'utf8'))}))`;
   const observed=run('node',['--input-type=module','-e',`${c.setup||''}const m=${load};${c.exercise||''}`]).stdout.trim();
   if(observed!==c.expected)throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={config_sha256:sha(readFileSync(config)),policy_fingerprint:policy.fingerprint,files,...sizes,logical,
    physical:{source_units:receipt.phases_ns.rules.local_units_visited??null,source_reused:receipt.phases_ns.rules.local_units_reused??null,
      target_passes:phase.js_rule_calls,target_reused:phase.js_rule_reuse_calls??null},
    peak_retained_bytes:resources.peak_retained_bytes,cpu_seconds:user+system,wall_seconds:elapsed,
    work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
  for(const key of ['files','logical','work','policy_fingerprint']){
   if(JSON.stringify(outputs.reused[key])!==JSON.stringify(outputs.disabled[key]))throw Error(`normalization transparency ${c.id}/${lane}/${key}`);
  }
  if(outputs.disabled.physical.source_reused!==0||outputs.disabled.physical.target_reused!==0)throw Error('disabled normalization reused a proof');
  row.lanes[lane]=outputs;console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const before=rows.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=rows.reduce((n,r)=>n+r.lanes[lane].reused[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'four affected controls, separate raw/gzip/Brotli objectives, prior pin and current reuse enabled/disabled; identical current logical decisions and independent runtime/scored-byte oracles; single coarse timing triplets, no fleet or general speed claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify({totals,physical:rows.map(row=>({id:row.id,lanes:Object.fromEntries(Object.entries(row.lanes).map(([name,lane])=>[name,Object.fromEntries(['before','reused','disabled'].map(label=>[label,lane[label].physical]))]))}))},null,2));
