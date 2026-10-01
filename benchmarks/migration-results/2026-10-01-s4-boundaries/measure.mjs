// Focused matched evidence, using each objective's final delivered artifact.
import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-boundaries-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-contracts-2/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const compile=(compiler,entry,config,out)=>{
 const r=run(compiler,[entry,'--config',config,'--format','esm','--target','js-module','--explain','json','-o',out]);
 const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
 const search=JSON.parse(r.stderr.slice(r.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
 return {code,judged:t.after,metrics:{sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}}};
};
const check=(code,exercise,expected)=>{
 const result=run('node',['--input-type=module'],`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));${exercise}`).stdout.trim();
 if(result!==JSON.stringify(expected))throw Error(`oracle mismatch: ${result}`);
};
const rows=[];
for(const [id,path] of [['receiver','../2026-10-01-g3-complete/artifacts/receiver'],['generic-open','../2026-10-01-g2-complete/artifacts/generic-open']]){
 const inputFolder=resolve(directory,path),entry=resolve(inputFolder,'main.lil'),oracle=JSON.parse(readFileSync(resolve(inputFolder,'oracle.json')));
 const folder=resolve(directory,'artifacts',id);mkdirSync(folder,{recursive:true});
 const row={id,source_sha256:sha(readFileSync(entry)),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const config=resolve(inputFolder,lane+'.toml'),outputs={};
  for(const [label,compiler] of [['before',baseline],['after',binary]]){
   const result=compile(compiler,entry,config,resolve(folder,`${label}-${lane}.mjs`));
   if(result.judged!==result.metrics[metric])throw Error('delivered/scored mismatch');
   if(id==='generic-open')check(result.code,`console.log(JSON.stringify([${JSON.stringify(oracle.inputs)}.map(n=>m.compute(n)),m.expose()]));`,[oracle.expected,oracle.shape]);
   else check(result.code,oracle.exercise,oracle.expected);
   outputs[label]=result.metrics;
  }
  row.lanes[lane]={config_sha256:sha(readFileSync(config)),...outputs};
  console.log(`checked matched ${id}/${lane}`);
 }
 rows.push(row);
}
const folder=resolve(directory,'artifacts/new-boundaries');mkdirSync(folder,{recursive:true});
const tests=readFileSync(resolve(root,'src/build_tests.rs'),'utf8');
const source=tests.split('fn s4_struct_constructor_and_prototype_boundaries_keep_class_identity()')[1].split('r#"')[1].split('"#')[0]+`
struct Wrapper<T>{T value;}
export Wrapper<Point> wrap(Wrapper<Point> original){Wrapper<Point> copy=original;copy.value.x+=1;return copy;}
`;
const entry=resolve(folder,'main.lil');writeFileSync(entry,source);
const exercise=`let reads=0;const p={get x(){reads++;return 8;}},q={x:2},box=new m.Box(p,q),base=new m.Box(),derived=new m.Derived(),original={value:{x:6}},copy=m.wrap(original);console.log(JSON.stringify([m.Box.name,m.Box.length,box.constructor===m.Box,box instanceof m.Box,box.total,base.total,box.add.length,box.add(),box.add(q,q),q,reads,derived.total,derived instanceof m.Box,derived.constructor===m.Derived,m.make().total,m.derived().total,original,copy,original!==copy,original.value!==copy.value]));`;
const expected=['Box',0,true,true,10,3,0,{x:14},{x:14},{x:2},1,2,true,true,11,7,{value:{x:6}},{value:{x:7}},true,true];
writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise,expected},null,2)+'\n');
const added={source_sha256:sha(source),scope:'independent behavior oracle; no prior supported artifact or byte-win claim',lanes:{}};
for(const [lane,metric] of Object.entries(metrics)){
 const config=resolve(folder,lane+'.toml');writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n`);
 const result=compile(binary,entry,config,resolve(folder,lane+'.mjs'));
 if(result.judged!==result.metrics[metric])throw Error('delivered/scored mismatch');
 check(result.code,exercise,expected);added.lanes[lane]=result.metrics;
 console.log(`checked new boundaries/${lane}`);
}
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const before=rows.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=rows.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'two matched development cases and one new capability oracle; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows,added},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
