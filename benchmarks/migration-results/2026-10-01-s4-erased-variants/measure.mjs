import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)),root=resolve(directory,'../../..');
const binary='/home/azureuser/lilscript-work/bin/s4-erased-variants-1/lilscript';
const baseline='/home/azureuser/lilscript-work/bin/s4-variants-1/lilscript';
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const identity=path=>({path,sha256:sha(readFileSync(path))});
const run=(command,args,input)=>{const r=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:300000,maxBuffer:64*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r;};
const metrics={raw:'raw',gzip:'gzip9',brotli:'brotli11'};
const cases=[{
 id:'static-inheritance',matched:true,
 source:`class Base{int n;init(int n){this.n=n;}int read(int extra=2){return this.n+extra;}}class Child extends Base{init(int n){super(n);}}export int run(int n){Base value=new Child(n);return value.read();}`,
 exercise:`console.log(JSON.stringify([m.run(3),m.run(-7)]));`,expected:[5,-5]
},{
 id:'payload-dispatch',matched:true,
 source:`class Text{string text;init(string x){this.text=x;}}class Count{int count;init(int x){this.count=x;}}sealed class Base{virtual int read(int n=3){return n+1;}}class Child extends Base{override int read(int n=4){return n*2;}}Text|Count choose(bool text){if(text){return new Text("hello");}return new Count(9);}export int payload(bool text){auto f=match(choose(text)){Text(item)=>(()=>item.text.length),Count(item)=>(()=>item.count)};return f();}export int dispatch(bool child){Base b=new Base();if(child){b=new Child();}return b.read();}`,
 exercise:`console.log(JSON.stringify([m.payload(true),m.payload(false),m.dispatch(false),m.dispatch(true)]));`,expected:[5,9,4,8]
},{
 id:'generic-virtual',matched:false,
 source:`sealed class Base<T>{T stored;init(T value){this.stored=value;}virtual T pick(T value,bool stored=false){if(stored){return this.stored;}return value;}virtual U echo<U>(U value){return value;}}class Child<S> extends Base<S>{init(S value){super(value);}override S pick(S value,bool stored=true){if(stored){return this.stored;}return value;}override V echo<V>(V value){return value;}}export string run(bool child){Base<string> value=new Base<string>("base");if(child){value=new Child<string>("child");}return value.pick("given");}export int generic(){Base<string> value=new Child<string>("x");return value.echo(7);}`,
 exercise:`console.log(JSON.stringify([m.run(false),m.run(true),m.generic()]));`,expected:['given','child',7]
}];
const rows=[];
for(const c of cases){
 const folder=resolve(directory,'artifacts',c.id);mkdirSync(folder,{recursive:true});
 const entry=resolve(folder,'main.lil');writeFileSync(entry,c.source);
 writeFileSync(resolve(folder,'oracle.json'),JSON.stringify({exercise:c.exercise,expected:c.expected},null,2)+'\n');
 const row={id:c.id,matched:c.matched,migration:!!c.migration,source_sha256:sha(c.source),lanes:{}};
 for(const [lane,metric] of Object.entries(metrics)){
  const outputs={};
  for(const [label,compiler,mode] of c.matched?[['before',baseline,null],['after',binary,null]]:[['after',binary,null]]){
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
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'static-inheritance and captured-payload controls plus erased generic dispatch capability; independent behavior oracles, no fleet or timing claim',compiler:identity(binary),baseline:identity(baseline),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binary),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
