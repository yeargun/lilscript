import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync,statSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-codec-cache-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const scratch='/home/azureuser/lilscript-work/measurements/q2-codec-cache';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args)=>{const r=spawnSync(command,args,{cwd:root,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'spellings', source:readFileSync(resolve(root,'tests/cases/objective_judged_spellings.lil'),'utf8'),
 expected:readFileSync(resolve(root,'tests/cases/objective_judged_spellings.out'),'utf8').trim(), setup:''
},{
 id:'delivered-control',source:'import {increment} from "./lib";print(increment(3));',
 files:{'lib.lil':'extern int read();export int offset=read();export int increment(int x){return x+offset;}'},
 config:"[delivery]\nmode='preserve-modules'\n",setup:'globalThis.read=()=>1;',expected:'4',delivery:true
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 for(const [name,text] of Object.entries(c.files||{}))writeFileSync(resolve(folder,name),text);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({setup:c.setup,expected:c.expected},null,2)+'\n');
 const row={id:c.id,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const cache=resolve(scratch,c.id,lane);rmSync(cache,{recursive:true,force:true});
  const outputs={};
  for(const label of ['cold','warm','disabled']){
   const config=resolve(folder,`${label}-${lane}.toml`);
   // Warm deliberately overrides a TOML veto/path through --cache DIR;
   // disabled overrides TOML's enabled directory through --cache off.
   const cacheText=label==='warm'?"codec_reuse=false\ndirectory='unused-cache'":`codec_reuse=true\ndirectory=${JSON.stringify(cache)}`;
   writeFileSync(config,`${c.config||''}\n[cache]\n${cacheText}\n[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const output=resolve(folder,`${label}-${lane}${c.delivery?'':'.mjs'}`);
   if(c.delivery)rmSync(output,{recursive:true,force:true});
   const flags=label==='cold'?[]:['--cache',label==='warm'?cache:'off'];
   const args=[entry,'--config',config,'--format','esm','--target','js-module',...flags];
   const policy=JSON.parse(run(binary,[...args,'--print-policy']).stdout);
   if(policy.resolution.cache.codec_reuse!==(label!=='disabled'))throw Error('cache CLI precedence');
   const timing=resolve(folder,`${label}-${lane}.time`);
   const result=run('/usr/bin/time',['-f','%U %S %e','-o',timing,binary,...args,'--explain','json',c.delivery?'--out-dir':'-o',output]);
   const [user,system,elapsed]=readFileSync(timing,'utf8').trim().split(/\s+/).map(Number);
   rmSync(timing);
   const paths=c.delivery?readdirSync(output,{recursive:true}).filter(n=>/\.(m?js)$/.test(n)).sort().map(n=>resolve(output,n)):[output];
   const files=paths.map(path=>{
    const sizes=JSON.parse(run(codec,['--json',path]).stdout).artifacts[0];
    return {name:c.delivery?path.slice(output.length+1):'main.mjs',sha256:sha(readFileSync(path)),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11};
   });
   const sizes=Object.fromEntries(Object.values(metrics).map(key=>[key,files.reduce((n,file)=>n+file[key],0)]));
   const receipt=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))),search=receipt.search,t=search.terminal.objectives[0];
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
   const observed=run('node',['--input-type=module','-e',`${c.setup}${load};`]).stdout.trim();
   if(observed!==c.expected)throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={config_sha256:sha(readFileSync(config)),policy_fingerprint:policy.fingerprint,flags,files,...sizes,logical,
    physical:receipt.codec_cache,cpu_seconds:user+system,wall_seconds:elapsed,
    work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
  for(const label of ['warm','disabled'])for(const key of ['files','logical','work','policy_fingerprint']){
   if(JSON.stringify(outputs.cold[key])!==JSON.stringify(outputs[label][key]))throw Error(`cache transparency ${c.id}/${lane}/${key}`);
  }
  if(outputs.disabled.physical.disk_hits!==0||outputs.disabled.physical.memory_hits!==0)throw Error('disabled cache reused a measurement');
  if(lane!=='raw' && outputs.warm.physical.disk_hits===0)throw Error('warm cache did not exercise persistent reuse');
  const cacheBytes=lane==='raw'?0:statSync(resolve(cache,'codec-v1.bin')).size;
  if(cacheBytes>393216)throw Error('unbounded cache');
  row.lanes[lane]={cache_bytes:cacheBytes,...outputs};console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'one cold/warm/disabled process triplet per generic case/objective; CLI precedence, exact artifact/logical parity, independent runtime and scored-byte oracles; coarse timing observations, no fleet or general speed claim',compiler:identity(binary),codec:identity(codec),rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(rows.map(row=>({id:row.id,lanes:Object.fromEntries(Object.entries(row.lanes).map(([name,lane])=>[name,Object.fromEntries(['cold','warm','disabled'].map(label=>[label,{...lane[label].physical,cpu_seconds:lane[label].cpu_seconds}]))]))})),null,2));
