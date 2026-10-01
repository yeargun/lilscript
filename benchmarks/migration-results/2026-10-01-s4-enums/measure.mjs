import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-enums-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-char-code-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'legacy-enum',matched:true,
 source:`enum Status{Draft,Active,Sold}export int code(Status s){return match(s){Status.Draft=>7,Status.Active=>3,Status.Sold=>9};}export Status value(){return Status.Active;}`,
 exercise:`console.log(JSON.stringify([m.code(0),m.code(1),m.code(2),m.value()]));`,expected:[7,3,9,1]
},{
 id:'abi-protocol',matched:false,
 source:`enum Kind:string{Text="text",Break="break"}flags enum Access:int{Read=1,Write=4}export Kind? parse(string s){return Kind.from(s);}export int ordinal(Kind k){return k.ordinal;}export string abi(Kind k){return k.abi;}export int bits(Access a,Access b){return (a|b).abi;}export bool has(Access a,Access b){return a.has(b);}export void append(Kind[] values){values.push(Kind.Break);}export bool invalid(int bits){return Access.from(bits)==null;}`,
 exercise:`const v=['text'];m.append(v);console.log(JSON.stringify([m.parse('text'),m.parse('bad'),m.ordinal('break'),m.abi('text'),m.bits(1,4),m.has(5,4),m.invalid(2),m.invalid(5),v]));`,expected:['text',null,1,'text',5,true,true,false,['text','break']]
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:c.matched,migration:!!c.migration,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of c.matched?[['before',baseline,null],['after',binary,c.migration||!c.matched?'explicit':null]]:[['after',binary,c.migration||!c.matched?'explicit':null]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n${mode?`[language]\nenum_abi='${mode}'\n`:''}`);
   const out=resolve(folder,`${label}-${lane}.mjs`);
   const result=run(compiler,[entry,'--config',config,'--format','esm','--target','js-module','--explain','json','-o',out]);
   const code=readFileSync(out,'utf8'),sizes=JSON.parse(run(codec,['--json',out]).stdout).artifacts[0];
   const search=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))).search,t=search.terminal.objectives[0];
   if(t.after!==sizes[metric])throw Error('delivered/scored mismatch');
   const observed=run('node',['--input-type=module','-e',`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));${c.exercise}`]).stdout.trim();
   if(observed!==JSON.stringify(c.expected))throw Error(`oracle mismatch ${c.id}/${lane}/${label}: ${observed}`);
   outputs[label]={config_sha256:sha(readFileSync(config)),sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
  row.lanes[lane]=outputs;console.log(`checked ${c.id}/${lane}`);
 }
 rows.push(row);
}
const groupTotals=migration=>Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const matched=rows.filter(r=>r.matched&&r.migration===migration),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
const totals={unchanged_contract:groupTotals(false)};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'one unchanged legacy enum control and one declared ABI/flag capability with independent behavior oracles; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
