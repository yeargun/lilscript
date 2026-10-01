import {readFileSync,writeFileSync,mkdirSync,readdirSync,rmSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/q2-use-index-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-boundary-admission-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args)=>{const r=spawnSync(command,args,{cwd:root,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'product-control', source:'struct Point{int x;int y;}export Point increment(Point p){p.x+=1;return p;}',
 exercise:'const p={x:3,y:4};console.log(JSON.stringify([m.increment(p),p]));',expected:[{x:4,y:4},{x:3,y:4}]
},{
 id:'recursive-group', source:'int left(int x){return right(x);}int right(int x){return left(x);}export int alive(int x){return x+1;}',
 exercise:'console.log(JSON.stringify([m.alive(2),m.alive(-4)]));',expected:[3,-3]
},{
 id:'delivered-control',source:'import {increment} from "./lib";print(increment(3));',
 files:{'lib.lil':'extern int read();export int offset=read();export int increment(int x){return x+offset;}'},
 config:"[delivery]\nmode='preserve-modules'\n",setup:'globalThis.read=()=>1;',expected:4,exercise:'',delivery:true
},{
 id:'delivered-setter', baseline_refusal:'index out of bounds', source:'import {counter,bump,twice} from "./lib";bump();counter=counter+twice(3);print(counter);',
 files:{'lib.lil':'extern int read();export int counter=read();export void bump(){counter=counter+read();}export int twice(int x){for(int i=0;i<2;i++){x=x+read();}return x;}'},
 config:"[delivery]\nmode='preserve-modules'\n",setup:'globalThis.read=()=>1;',expected:7,exercise:'',delivery:true
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 for(const [name,text] of Object.entries(c.files||{}))writeFileSync(resolve(folder,name),text);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({setup:c.setup||'',exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:!c.baseline_refusal,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler] of [['before',baseline],['after',binary]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`${c.config||''}\n[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const output=resolve(folder,`${label}-${lane}${c.delivery?'':'.mjs'}`);
   if(c.delivery)rmSync(output,{recursive:true,force:true});
   const args=[entry,'--config',config,'--format','esm','--target','js-module','--explain','json',c.delivery?'--out-dir':'-o',output];
   if(label==='before' && c.baseline_refusal){
    const failed=spawnSync(compiler,args,{cwd:root,encoding:'utf8',timeout:120000});
    if(failed.status===0 || !failed.stderr.includes(c.baseline_refusal))throw Error('baseline failure did not reproduce');
    outputs[label]={exit:failed.status,diagnostic:failed.stderr};
    continue;
   }
   const result=run(compiler,args);
   const paths=c.delivery?readdirSync(output,{recursive:true}).filter(n=>/\.(m?js)$/.test(n)).sort().map(n=>resolve(output,n)):[output];
   const files=paths.map(path=>{
    const sizes=JSON.parse(run(codec,['--json',path]).stdout).artifacts[0];
    return {name:c.delivery?path.slice(output.length+1):'main.mjs',sha256:sha(readFileSync(path)),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11};
   });
   const sizes=Object.fromEntries(Object.values(metrics).map(key=>[key,files.reduce((n,file)=>n+file[key],0)]));
   const receipt=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))),search=receipt.search,t=search.terminal.objectives[0];
   if(t.after!==sizes[metric])throw Error(`delivered/scored mismatch ${c.id}/${lane}/${label}: ${t.after} vs ${sizes[metric]}`);
   let load;
   if(c.delivery){
    writeFileSync(resolve(output,'package.json'),'{"type":"module"}\n');
    const main=paths.find(path=>/\/main\.(m?js)$/.test(path));
    if(!main)throw Error(`no main file: ${paths}`);
    load=`await import(${JSON.stringify(pathToFileURL(main).href)})`;
   }else load=`await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(readFileSync(output,'utf8'))}))`;
   const observed=run('node',['--input-type=module','-e',`${c.setup||''}const m=${load};${c.exercise}`]).stdout.trim();
   if(observed!==JSON.stringify(c.expected))throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={config_sha256:sha(readFileSync(config)),files,...sizes,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
  row.lanes[lane]=outputs;console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const matched=rows.filter(r=>r.matched),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'matched product, recursive-group and multi-file controls; separate retained old setter panic and repaired output; independent runtime and scored-byte oracles, no fleet or CPU claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>/\.(rs|lil|h)$/.test(p)).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
