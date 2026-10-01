import {readFileSync,writeFileSync,mkdirSync,rmSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),port='/home/azureuser/markedlil';
const binary='/home/azureuser/lilscript-work/bin/q2-formation-5/lilscript',previous='/home/azureuser/lilscript-work/bin/q2-render-1/lilscript';
const sha=x=>createHash('sha256').update(x).digest('hex');
const run=(command,args,env={})=>{const r=spawnSync(command,args,{cwd:port,env:{...process.env,...env},encoding:'utf8',timeout:300000,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const base=readFileSync(resolve(port,'lilscript.toml'),'utf8');
const folder=resolve(directory,'marked');mkdirSync(folder,{recursive:true});
const rows=[],probe=process.argv.includes('--probe'),resume=process.argv.includes('--resume');
for(const [codec,level] of [['raw',13]]){
 const row=resume?JSON.parse(readFileSync(resolve(directory,'marked-samples.json'),'utf8')):{codec,level,runs:[]};
 const reference=sha(readFileSync(resolve(directory,'marked-initial/raw-13-before-0.mjs')));
 for(const [sample,labels] of (probe?[[1,['after']]]:[[1,['after','cold']],[2,['cold','after']],[3,['after','cold']]])){
  for(const label of labels){
   if(row.runs.some(x=>x.label===label&&x.sample===sample))continue;
   const config=resolve(folder,`${codec}-${level}-${label}.toml`);
   writeFileSync(config,base.replace(/optimization_level\s*=\s*\d+/,`optimization_level = ${level}`).replace(/cost_model\s*=\s*"[^"]+"/,`cost_model = "${codec}"`)+(label==='cold'?'\n[cache]\nformation_reuse=false\n':''));
   const output=resolve(folder,`${codec}-${level}-${label}-${sample}.mjs`),timing=output+'.time';
   const r=run('/usr/bin/time',['-f','%U %S %e','-o',timing,label==='before'?previous:binary,resolve(port,'src/entry.lil'),'--target','js-module','--config',config,'--explain','json','-o',output],{LILSCRIPT_TIMING:'1'});
   const [user,system,wall]=readFileSync(timing,'utf8').trim().split(/\s+/).map(Number);rmSync(timing);
   const split=r.stderr.lastIndexOf('lilscript-timing '),phase=JSON.parse(r.stderr.slice(split+'lilscript-timing '.length));
   const receipt=JSON.parse(r.stderr.slice(r.stderr.indexOf('{\n'),split).trim());
   const samples=['# Heading\n\n**bold** and [link](https://example.com)\n','*x* and `y`'];
   const source=readFileSync(output),hash=sha(source),observed=JSON.parse(run('node',['--input-type=module','-e',`const m=await import(${JSON.stringify(pathToFileURL(output).href)}),samples=${JSON.stringify(samples)};console.log(JSON.stringify([m.parse(samples[0],undefined),m.parseInline(samples[1],undefined)]));`]).stdout);
   if(hash!==reference)throw Error('marked differs from the preceding compiler artifact');
   const expected=['<h1>Heading</h1>\n<p><strong>bold</strong> and <a href="https://example.com">link</a></p>\n','<em>x</em> and <code>y</code>'];
   if(JSON.stringify(observed)!==JSON.stringify(expected))throw Error('marked observation mismatch');
   const terminal=receipt.search.terminal.objectives[0];
   row.runs.push({label,sample,sha256:hash,raw:source.length,objective_bytes:terminal.after,judged:terminal.judged,cpu_seconds:user+system,wall_seconds:wall,resources:receipt.resources,timing:phase});
   writeFileSync(resolve(directory,'marked-samples.json'),JSON.stringify({codec,level,runs:row.runs},null,2)+'\n');
   console.log(`${codec}/${level} ${label}/${sample} ${user+system}s ${phase.js_formation_calls} forms ${phase.js_formation_reuse_calls||0} reused ${phase.formation_admission_refused_calls||0} uncacheable-admissions`);
  }
 }
 if(probe){rows.push(row);continue;}
 const after=row.runs.find(x=>x.label==='after'),cold=row.runs.find(x=>x.label==='cold');
 for(const x of row.runs){
  if(x.sha256!==after.sha256||x.objective_bytes!==after.objective_bytes)throw Error(`marked output changed ${codec}/${level}/${x.label}`);
  if(x.label!=='before'&&JSON.stringify(x.resources)!==JSON.stringify(after.resources))throw Error('marked resource transparency');
  if(x.label==='cold'&&(x.timing.js_formation_reuse_calls||0)!==0)throw Error('marked formation veto');
 }
 row.median=Object.fromEntries(['after','cold'].map(label=>[label,row.runs.filter(x=>x.label===label).map(x=>x.cpu_seconds).sort((a,b)=>a-b)[1]]));
 row.physical_forms={cold:cold.timing.js_formation_calls,reused:after.timing.js_formation_calls,hits:after.timing.js_formation_reuse_calls||0};
 rows.push(row);
 writeFileSync(resolve(directory,'marked-comparison.json'),JSON.stringify({binary:{path:binary,sha256:sha(readFileSync(binary))},previous:{path:previous,sha256:sha(readFileSync(previous))},sources:readdirSync(resolve(port,'src'),{recursive:true}).filter(x=>x.endsWith('.lil')).sort().map(path=>({path:'src/'+path,sha256:sha(readFileSync(resolve(port,'src',path)))})),rows},null,2)+'\n');
}
console.log(JSON.stringify(rows.map(({codec,level,median,physical_forms})=>({codec,level,median,physical_forms})),null,2));
