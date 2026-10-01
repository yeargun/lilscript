// Affected development inputs: matched open/closed boundaries, no held-out tuning.
import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url)), root=resolve(directory,'../../..');
const binaries={g1:'/home/azureuser/lilscript-work/bin/g1-complete-2/lilscript',g2:'/home/azureuser/lilscript-work/bin/g2-complete-1/lilscript'};
const codec='/home/azureuser/lilscript-work/bin/q1-complete-2/lilscript-codec';
const sha=x=>createHash('sha256').update(x).digest('hex');
const run=(command,args,input)=>{const result=spawnSync(command,args,{cwd:root,input,encoding:'utf8',timeout:120000,maxBuffer:64*1024*1024});if(result.status!==0)throw Error(`${command}: ${result.stderr||result.error}`);return result;};
const inputs=[-11,0,7,31];
const sources=[
 {id:'siblings',declarations:'class Base{int sharedValue=2;}'+Array.from({length:16},(_,i)=>`class C${i} extends Base{int privateCount=${i+1};int privateOffset=${i+2};int privateScale=${i+3};int privateLimit=${i+4};}`).join(''),
  compute:'int total=0;'+Array.from({length:16},(_,i)=>`C${i} c${i}=new C${i}();c${i}.privateCount+=n;total+=c${i}.sharedValue+c${i}.privateCount+c${i}.privateOffset+c${i}.privateScale+c${i}.privateLimit;`).join('')+'return total;',
  publication:'export Base expose(){return new C15();}',expected:inputs.map(n=>672+16*n),shape:{sharedValue:2,privateCount:16,privateOffset:17,privateScale:18,privateLimit:19},pristine:false},
 {id:'generic',declarations:'class Box<T>{T hiddenPayload;init(T value){this.hiddenPayload=value;}}',
  compute:'Box<int> b=new Box<int>(n);b.hiddenPayload+=7;return b.hiddenPayload*3;',
  publication:'export Box<int> expose(){return new Box<int>(23);}',expected:inputs.map(n=>(n+7)*3),shape:{hiddenPayload:23},pristine:false},
 {id:'observed',declarations:'class Box{int hiddenAmount;int hiddenOffset=7;init(int n){this.hiddenAmount=n;}}',
  compute:'Box b=new Box(n);if(b is Box){b.hiddenAmount+=b.hiddenOffset;return b.hiddenAmount*3;}return -1;',
  publication:'export Box expose(){return new Box(23);}',expected:inputs.map(n=>(n+7)*3),shape:{hiddenAmount:23,hiddenOffset:7},pristine:true},
];
const rows=[];
for(const item of sources)for(const boundary of ['closed','open']){
 const folder=resolve(directory,'artifacts',`${item.id}-${boundary}`);mkdirSync(folder,{recursive:true});
 const source=item.declarations+`${boundary==='open'?'export ':''}int compute(int n){${item.compute}}`+(boundary==='open'?item.publication:`extern int input();for(int i=0;i<4;i+=1){print(compute(input()));}`);
 const entry=resolve(folder,'main.lil');writeFileSync(entry,source+'\n');
 const oracle={inputs,expected:item.expected,...(boundary==='open'?{shape:item.shape}:{})};writeFileSync(resolve(folder,'oracle.json'),JSON.stringify(oracle)+'\n');
 const row={id:item.id,boundary,source_sha256:sha(source+'\n'),oracle_sha256:sha(JSON.stringify(oracle)+'\n'),lanes:{}};
 for(const lane of ['raw','gzip','brotli']){
  const config=`[optimization]\npreset='maximum'\ninlining=false\nscalar_replacement=false\n[javascript]\npriority='size-first'\nstrip_debug=false\nstrip_console_calls=false\nassume_pristine_builtins=${item.pristine}\n[objective]\ncodecs='${lane}'\n[effort]\nlevel=13\n[policy.tactics]\nproperty-mangling='on'\n`;
  const configPath=resolve(folder,lane+'.toml');writeFileSync(configPath,config);row.lanes[lane]={config_sha256:sha(config)};
  for(const [version,binary]of Object.entries(binaries)){
   const output=resolve(folder,`${version}-${lane}.js`),format=boundary==='open'?'esm':'iife',target=boundary==='open'?'js-module':'js';
   const result=run(binary,[entry,'--config',configPath,'--format',format,'--target',target,'--explain','json','-o',output]),code=readFileSync(output,'utf8');
   let oracleCode;
   if(boundary==='closed')oracleCode=`let i=0;globalThis.input=()=>${JSON.stringify(inputs)}[i++];${code}`;
   else oracleCode=`const m=await import('data:text/javascript,'+encodeURIComponent(${JSON.stringify(code)}));for(const n of ${JSON.stringify(inputs)})console.log(m.compute(n));const value=m.expose();const expected=${JSON.stringify(item.shape)};if(JSON.stringify(Object.keys(value).sort())!==JSON.stringify(Object.keys(expected).sort())||Object.keys(expected).some(k=>value[k]!==expected[k])||JSON.stringify(JSON.parse(JSON.stringify(value)))!==JSON.stringify(value))throw Error('public shape');`;
   const actual=run('node',['--input-type=module'],oracleCode).stdout;
   if(actual!==item.expected.join('\n')+'\n')throw Error(`${item.id}/${boundary}/${lane}/${version}: ${actual}`);
   const sizes=JSON.parse(run(codec,['--json',output]).stdout).artifacts[0],explain=JSON.parse(result.stderr.slice(result.stderr.indexOf('{\n'))),search=explain.search,t=search.terminal.objectives[0],metric={raw:'raw',gzip:'gzip9',brotli:'brotli11'}[lane];
   if(t.after!==sizes[metric])throw Error('scored/delivered mismatch');
   row.lanes[lane][version]={sha256:sha(code),raw:sizes.raw,gzip9:sizes.gzip9,brotli11:sizes.brotli11,work:{proposals:search.proposals,structures:search.structures,renders:search.renders,codec_probes:search.codec_probes,examined:t.examined,judged:t.judged}};
  }
 }
 rows.push(row);console.log(`checked ${item.id}/${boundary}`);
}
const totals=Object.fromEntries(['closed','open'].map(boundary=>[boundary,Object.fromEntries([['raw','raw'],['gzip','gzip9'],['brotli','brotli11']].map(([lane,key])=>{const cohort=rows.filter(r=>r.boundary===boundary),before=cohort.reduce((s,r)=>s+r.lanes[lane].g1[key],0),after=cohort.reduce((s,r)=>s+r.lanes[lane].g2[key],0);return[lane,{before,after,delta:after-before}];}))]));
const identity=path=>({path,sha256:sha(readFileSync(path))});
writeFileSync(resolve(directory,'comparison.json'),JSON.stringify({scope:'six affected open/closed development programs; effort13; scalar replacement/inlining disabled to retain affected layouts; independent numeric/shape oracles; not fleet or timing evidence',compilers:Object.fromEntries(Object.entries(binaries).map(([v,p])=>[v,identity(p)])),codec:identity(codec),totals,rows},null,2)+'\n');
const paths=['Cargo.toml','Cargo.lock',...readdirSync(resolve(root,'src'),{recursive:true}).filter(p=>p.endsWith('.rs')).map(p=>'src/'+p)].sort();
const source_files=paths.map(path=>({path,sha256:sha(readFileSync(resolve(root,path)))}));
writeFileSync(resolve(directory,'identity.json'),JSON.stringify({head:run('git',['rev-parse','HEAD']).stdout.trim(),compiler:identity(binaries.g2),source_files,source_sha256:sha(JSON.stringify(source_files)),source_digest_recipe:'SHA-256 of compact JSON source_files in sorted path order'},null,2)+'\n');
console.log(JSON.stringify(totals));
