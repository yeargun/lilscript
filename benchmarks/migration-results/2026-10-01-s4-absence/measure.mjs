import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-absence-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-shapes-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'class-fields',matched:true,
 source:`export class Box{int value;int[] entries;init(int n){this.value=n;this.entries=[n,n+1];}int next(){this.value+=1;return this.value;}}export constructor Box;export int read(int n){Box box=new Box(n);return box.next()+box.entries[1];}`,
 exercise:`const b=new m.Box(4);console.log(JSON.stringify([b.next(),b.entries,b.constructor===m.Box,b instanceof m.Box,m.read(5),Object.keys(b)]));`,
 expected:[5,[4,5],true,true,12,['value','entries']]
},{
 id:'private-product',matched:true,
 source:`struct Point{int x;int y;}class Holder{Point p=Point{1,2};int next(int n){this.p.x+=n;return this.p.x+this.p.y;}}export int run(int n){Holder h=new Holder();return h.next(n)+h.next(2);}`,
 exercise:`console.log(JSON.stringify([m.run(0),m.run(4),m.run(-8)]));`,
 expected:[8,16,-8]
},{
 id:'internal-absence',matched:true,migration:true,
 source:`export bool same(Map<string,int> values,string a,string b){return values.get(a)==values.get(b);}`,
 exercise:`const values=new Map([['a',0],['b',7],['c',7]]);console.log(JSON.stringify([m.same(values,'a','a'),m.same(values,'b','c'),m.same(values,'a','b'),m.same(values,'x','y')]));`,
 expected:[true,true,false,true]
},{
 id:'absence-boundaries',matched:false,
 source:`int count=0;int next(){count+=1;return count;}int choose(int value=next()){return value;}export int exercise(){int? a=undefined;int? b=null;int same=0;if(a==b){same=100;}return same+choose(a)*10+choose(b);}export int | undefined absentU(){return null;}export int | null absentN(){return undefined;}export int defaulted(int value=7){return value;}`,
 exercise:`console.log(JSON.stringify([m.exercise(),m.absentU()===undefined,m.absentN()===null,m.defaulted(),m.defaulted(0),m.defaulted.length]));`,
 expected:[112,true,true,7,0,0]
},{
 id:'pinned-collections',matched:false,
 source:`export (int | undefined)[] values=[null,2];export void change((int | undefined)[] values){values[1]=null;values.push(null);}export (int | undefined)[] copy((int | null)[] source){return [...source];}`,
 exercise:`const a=m.values;m.change(a);const source=[null,3];console.log(JSON.stringify([a===m.values,a.length,a.every(x=>x===undefined),m.copy(source)[0]===undefined,source[0]===null]));`,
 expected:[true,3,true,true,true]
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:c.matched,migration:!!c.migration,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of c.matched?[['before',baseline,null],['after',binary,c.migration||!c.matched?'unified':null]]:[['after',binary,c.migration||!c.matched?'unified':null]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n${mode?`[language]\nabsence='${mode}'\n`:''}`);
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
const totals={unchanged_contract:groupTotals(false),opt_in_migration:groupTotals(true)};
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'two unchanged-contract controls, one opt-in absence comparison with a shared behavior oracle, and two new boundary capabilities; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
