import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync,statSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-build-cache-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/q2-effect-components-2/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const cacheRoot='/home/azureuser/lilscript-work/cache/q2-build-cache-evidence';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,env={})=>{const r=spawnSync(command,args,{cwd:root,env:{...process.env,...env},encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
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
  const outputs={};const cache=resolve(cacheRoot,c.id,lane);
  for(const label of ['before','disabled','cold-0','warm-0','cold-1','warm-1','cold-2','warm-2']){
   const compiler=label==='before'?baseline:binary,cached=/^(cold|warm)/.test(label);
   if(label.startsWith('cold'))rmSync(cache,{recursive:true,force:true});
   const config=resolve(folder,`${cached?'cached':label}-${lane}.toml`);
   const cacheConfig=label==='before'?'':`[cache]\nbuild_reuse=${cached}\n${cached?`directory=${JSON.stringify(cache)}\n`:''}`;
   writeFileSync(config,`${c.config||''}\n${cacheConfig}[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const output=resolve(folder,`${label}-${lane}${c.delivery?'':'.mjs'}`);
   if(c.delivery)rmSync(output,{recursive:true,force:true});
   const args=[entry,'--config',config,'--format','esm','--target','js-module'];
   const policy=JSON.parse(run(compiler,[...args,'--print-policy']).stdout);
   const timing=resolve(folder,`${label}-${lane}.time`);
   const result=run('/usr/bin/time',['-f','%U %S %e','-o',timing,compiler,...args,'--explain','json',c.delivery?'--out-dir':'-o',output],{LILSCRIPT_TIMING:'1'});
   const [user,system,elapsed]=readFileSync(timing,'utf8').trim().split(/\s+/).map(Number);rmSync(timing);
   const split=result.stderr.lastIndexOf('lilscript-timing ');
   const phase=JSON.parse(result.stderr.slice(split+'lilscript-timing '.length));
   const receipt=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'),split).trim());
   const paths=c.delivery?readdirSync(output,{recursive:true}).filter(n=>/\.(m?js)$/.test(n)).sort().map(n=>resolve(output,n)):[output];
   const files=paths.map(path=>{const sizes=JSON.parse(run(codec,['--json',path]).stdout).artifacts[0];return {name:c.delivery?path.slice(output.length+1):'main.mjs',sha256:sha(readFileSync(path)),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11};});
   const sizes=Object.fromEntries(Object.values(metrics).map(key=>[key,files.reduce((n,file)=>n+file[key],0)]));
   const search=receipt.search,t=search.terminal.objectives[0];
   if(t.after!==sizes[metric])throw Error(`delivered/scored mismatch ${c.id}/${lane}/${label}`);
   const resources=receipt.resources;
   const logical={baseline:resources.baseline_work,optional:resources.optional_work,codec:resources.codec_work};
   let load;
   if(c.delivery){writeFileSync(resolve(output,'package.json'),'{"type":"module"}\n');const main=paths.find(path=>/\/main\.(m?js)$/.test(path));if(!main)throw Error(`no main file: ${paths}`);load=`await import(${JSON.stringify(pathToFileURL(main).href)})`;}
   else load=`await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(readFileSync(output,'utf8'))}))`;
   const observed=run('node',['--input-type=module','-e',`${c.setup||''}const m=${load};${c.exercise||''}`]).stdout.trim();
   if(observed!==c.expected)throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   const hit=receipt.build_cache?.hit??false;
   if(hit!==label.startsWith('warm'))throw Error(`cache hit mismatch ${c.id}/${lane}/${label}: ${JSON.stringify(receipt.build_cache)}`);
   if(label.startsWith('cold')&&receipt.build_cache.state!=='stored')throw Error(`cache not stored ${c.id}/${lane}`);
   if(cached && receipt.total_ns < (receipt.build_cache.lookup_ns??receipt.build_cache.elapsed_ns))throw Error('cache time excluded from total');
   outputs[label]={config_sha256:sha(readFileSync(config)),policy_fingerprint:policy.fingerprint,files,...sizes,logical,
    physical:{target_passes:phase.js_rule_calls??0,effect_units:phase.source_effect_unit_calls??0,encodes:receipt.codec_cache.encodes??0},
    cache:receipt.build_cache?{hit,state:receipt.build_cache.state,key:receipt.build_cache.key,serialized_bytes:receipt.build_cache.serialized_bytes,lookup_ns:receipt.build_cache.lookup_ns??receipt.build_cache.elapsed_ns}:null,
    compiler_total_ns:receipt.total_ns,peak_retained_bytes:resources.peak_retained_bytes,cpu_seconds:user+system,wall_seconds:elapsed,
    work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
   for(const key of ['files','logical','work','policy_fingerprint']){
    if(label!=='before'&&label!=='disabled'&&JSON.stringify(outputs.disabled[key])!==JSON.stringify(outputs[label][key]))throw Error(`cache parity mismatch ${c.id}/${lane}/${label}/${key}`);
   }
  }
  outputs.same_prior_files=JSON.stringify(outputs.before.files)===JSON.stringify(outputs.disabled.files);
  outputs.cache_file_extent=statSync(resolve(cache,'build-v1.bin')).size;
  if(outputs.cache_file_extent>268440576)throw Error('cache extent exceeds bound');
  row.lanes[lane]=outputs;console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{const before=rows.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=rows.reduce((n,r)=>n+r.lanes[lane].disabled[key],0);return[lane,{before,after,delta:after-before}];}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'three generic controls; separate raw/gzip/Brotli; previous pin, disabled cache and three fresh cold/warm pairs; fresh runtime and independent score oracle per artifact. Coarse /usr/bin/time samples, not a fleet or universal speed claim.',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify({totals,timing:rows.map(r=>({id:r.id,lanes:Object.fromEntries(Object.entries(r.lanes).map(([lane,v])=>[lane,{cold:[0,1,2].map(i=>v[`cold-${i}`].cpu_seconds),warm:[0,1,2].map(i=>v[`warm-${i}`].cpu_seconds),cold_wall:[0,1,2].map(i=>v[`cold-${i}`].wall_seconds),warm_wall:[0,1,2].map(i=>v[`warm-${i}`].wall_seconds)}]))}))},null,2));
