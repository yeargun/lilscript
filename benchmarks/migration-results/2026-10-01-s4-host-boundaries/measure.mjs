import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-host-boundaries-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-boundaries-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const fixture=(path,name)=>readFileSync(resolve(root,path),'utf8').split(`fn ${name}()`)[1].split('r#"')[1].split('"#')[0];
const cases=[{
 id:'existing-callback',matched:true,formats:['module'],
 source:fixture('src/program/javascript_struct_boundary_tests.rs','function_values_reaching_host_code_are_wrapped_by_a_callable_adapter'),
 prelude:`globalThis.call=(f,a)=>{const r=typeof a==='string'?f.call(a):f(a);return typeof r==='object'&&r!==null?JSON.stringify(r):r};`,
 exercise:`m.run();`,expected:'{"name":"self","value":"s"}\nnull\n{"name":"b","value":"c"}\nk=v'
},{
 id:'callable-contracts',matched:false,formats:['script','module'],
 source:fixture('src/build_tests.rs','s4_host_callbacks_keep_struct_defaults_rest_and_primitive_receivers'),
 prelude:`globalThis.observe=(f,g,h)=>{const q={x:3};console.log(JSON.stringify([f.length,f(),f(undefined,q),g.length,g.call(5),g.call(5,q,q),h(null,q),q]));};`,
 exercise:'',expected:'[0,{"x":4},{"x":7},0,{"x":7},{"x":11},{"x":3},{"x":3}]'
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({prelude:c.prelude,exercise:c.exercise,expected:c.expected},null,2)+'\n');
 for(const format of c.formats){
  const row={id:c.id,format,matched:c.matched,source_sha256:sha(c.source),lanes:{}};
  for(const [lane,metric] of Object.entries(metrics)){
   const config=resolve(folder,lane+'.toml');writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
   const outputs={};
   for(const [label,compiler] of c.matched?[['before',baseline],['after',binary]]:[['after',binary]]){
    const out=resolve(folder,`${label}-${format}-${lane}.${format==='module'?'mjs':'js'}`);
    const r=run(compiler,[entry,'--config',config,'--format',format==='module'?'esm':'iife','--target',format==='module'?'js-module':'js','--explain','json','-o',out]);
    const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
    const search=JSON.parse(r.stderr.slice(r.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
    if(t.after!==sizes[metric])throw Error('delivered/scored mismatch');
    const exercise=format==='module'?`${c.prelude}const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));${c.exercise}`:`${c.prelude}${code}`;
    const observed=run('node',format==='module'?['--input-type=module','-e',exercise]:['-e',exercise]).stdout.trim();
    if(observed!==c.expected)throw Error(`oracle mismatch ${c.id}/${format}/${lane}/${label}: ${observed}`);
    outputs[label]={sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
   }
   row.lanes[lane]={config_sha256:sha(readFileSync(config)),...outputs};
   console.log(`checked ${c.id}/${format}/${lane}`);
  }
  rows.push(row);
 }
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const matched=rows.filter(r=>r.matched),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'one matched callback and new script/module ABI oracles; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
