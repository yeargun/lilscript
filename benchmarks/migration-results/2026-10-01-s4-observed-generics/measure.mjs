import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-observed-generics-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-sealed-modules-1/lilscript';
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
 id:'observed-generic',matched:false,
 source:`class Box<T>{T value;int count;init(T value,int count=2,T... rest){this.value=value;this.count=count;for(int i=0;i<rest.length;i++){this.value=rest[i];}}T get(){return this.value;}T pick(T fallback,T... rest){if(rest.length>0){return rest[rest.length-1];}return fallback;}V echo<V>(V value){return value;}}export constructor Box;class Child<U> extends Box<U>{init(U value,U... rest){super(value,3,...rest);}}export constructor Child;class Empty<T>{int count=7;}export constructor Empty;export int run(){Box<int> b=new Box<int>(5,4,7,9);Child<int> c=new Child<int>(2,8);Empty<string> e=new Empty<string>();return b.get()+b.count+c.get()+c.count+e.count+b.echo(10)+b.pick(1,2,3);}`,
 exercise:`const a=Symbol('a'),b=Symbol('b'),box=new m.Box(a),child=new m.Child(a,b),empty=new m.Empty();console.log(JSON.stringify([m.run(),box.get()===a,box.pick(a,b)===b,box.echo(a)===a,child.get()===b,child instanceof m.Box,child.constructor===m.Child,box.constructor===m.Box,box.count,child.count,empty.count,m.Box.length,m.Child.length,m.Empty.length,box.pick.length]));`,
 expected:[44,true,true,true,true,true,true,true,2,3,7,1,1,0,1]
},{
 id:'fixed-struct-generic',matched:false,
 source:`struct Point{int x;}export Point copy<T>(Point p,T value){p.x+=1;return p;}class Box<T>{T value;init(T v){this.value=v;}Point change(Point p){p.x+=2;return p;}}export constructor Box;class Constructed{int value;init(Point p){this.value=p.x;}}export JsValue constructorValue(){return Constructed;}export int run(){Point p=Point{4};Box<int> b=new Box<int>(3);Point q=b.change(p);Point r=copy(p,true);return p.x*100+q.x*10+r.x;}`,
 exercise:`const p={x:8},box=new m.Box(1),C=m.constructorValue();console.log(JSON.stringify([m.run(),m.copy(p,Symbol('v')),box.change(p),p,new C(p).value,new C(p).constructor===C,m.copy.length,box.change.length]));`,
 expected:[465,{x:9},{x:10},{x:8},8,true,2,1]

}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:c.matched,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of c.matched?[['before',baseline,null],['after',binary,null]]:[['after',binary,null]]){
   const config=resolve(folder,`${label}-${lane}.toml`);
   writeFileSync(config,`[optimization]\npreset='maximum'\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n${mode?`[language]\nfield_initialization='${mode}'\n`:''}`);
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
const totals=Object.fromEntries(Object.entries(metrics).map(([lane,key])=>{
 const matched=rows.filter(r=>r.matched),before=matched.reduce((n,r)=>n+r.lanes[lane].before[key],0),after=matched.reduce((n,r)=>n+r.lanes[lane].after[key],0);
 return[lane,{before,after,delta:after-before}];
}));
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'one matched class boundary and new observed/fixed-struct generic capability oracles; no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
