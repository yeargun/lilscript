// Freeze representative runtime inputs and the three benchmark-contract
// fixtures. This is evidence preparation, never a production port build path.
import {readFileSync,writeFileSync,mkdirSync,copyFileSync,existsSync,readdirSync} from "node:fs";
import {resolve,join,dirname} from "node:path";
import {fileURLToPath} from "node:url";
import {execFileSync} from "node:child_process";
import {build} from "../contracts/node_modules/esbuild/lib/main.js";
import {fileIdentity,snapshotDependencyTree} from "../../finer/tools/artifact-evidence.mjs";

const here=dirname(fileURLToPath(import.meta.url));
const args=Object.fromEntries(Array.from({length:(process.argv.length-2)/2},(_,index)=>[process.argv[index*2+2].replace(/^--/,""),process.argv[index*2+3]]));
for(const key of ["compiler","codec","inspector","ports-report","ports-work","work"]) if(!args[key]) throw new Error(`--${key} required`);
const output=resolve(args.work);
if(existsSync(output) && readdirSync(output).length) throw new Error("prepare directory must be empty");
mkdirSync(output,{recursive:true});
const receipt={schema:1,inputs:{},builds:[],artifacts:[],toolchain:snapshotDependencyTree(resolve(here,"../contracts/node_modules"),{maxBytes:2*1024**3,maxEntries:100_000}).sha256};
const pin=path=>{path=resolve(path);receipt.inputs[path]=fileIdentity(path);return path;};
for(const path of [args.compiler,args.codec,args.inspector,args["ports-report"],fileURLToPath(import.meta.url),resolve(here,"../contracts/package-lock.json")]) pin(path);
const write=(name,value)=>{const path=join(output,name);writeFileSync(path,typeof value==="string"?value:JSON.stringify(value,null,2)+"\n");return path;};
const commands=[];
function compile(source,stem,codec) {
  pin(source);
  const config=write(`${stem}.${codec}.toml`,`[policy]\nversion=3\n[objective]\ncodecs=["${codec}"]\n[effort]\nlevel=13\n`);
  const artifact=join(output,`${stem}.${codec}.mjs`);
  const target=["library","micro"].includes(stem)?"js-module":"js";
  const argv=[source,"--target",target,"--config",config,"-o",artifact];
  execFileSync(resolve(args.compiler),argv,{stdio:"pipe",timeout:120000,env:{PATH:"/usr/bin:/bin",TZ:"UTC",LANG:"C",LC_ALL:"C"}});
  receipt.builds.push({compiler:resolve(args.compiler),args:argv,config:fileIdentity(config),artifact:{path:artifact,...fileIdentity(artifact)}});
  commands.push(argv);
  return {artifact,config};
}
function inventory(artifact) {
  const value=JSON.parse(execFileSync(resolve(args.inspector),[artifact],{encoding:"utf8"}));
  return write(`${artifact.split("/").at(-1)}.inventory.json`,value);
}
const ports=JSON.parse(readFileSync(args["ports-report"],"utf8"));
const workloads=[];
for(const [id,core,workload,iterations,expected,counters] of [
  ["markedlil","marked.raw.js","marked-workload.mjs",1024,
    {characters:1024*(95+35+25),last:"<h1>Hello</h1>\n<p>A <strong>small</strong> <a href=\"https://example.com\">link</a>.</p>\n"},
    {parse_calls:1024*3+1,input_documents:3}],
  ["zodlil","zod.core.js","zod-workload.mjs",2048,
    {sum:8*255*256/2,failures:2048,retained_rows:8},{parse_calls:2048,safe_parse_calls:2048}],
  ["posthoglil","posthog.raw.js","posthog-workload.mjs",8192,
    {characters:8192*4*36,canonical:"0189dcd5-5311-7d40-8db0-9496a2eef37b"},{parse_calls:8192*4}],
]) {
  const port=ports.ports.find(row=>row.port===id);
  if(!port || port.state!=="green") throw new Error(`port ${id} lacks a passing build/test receipt`);
  const directory=join(resolve(args["ports-work"]),"runs",id,id);
  const candidateSource=pin(join(directory,"dist",core));
  const recorded=port.artifacts.find(row=>row.path===`dist/${core}`);
  if(recorded?.sha256!==fileIdentity(candidateSource).sha256) throw new Error(`port artifact differs from receipt: ${id}`);
  const candidate=join(output,`${id}.candidate.mjs`);copyFileSync(candidateSource,candidate);
  let entry,options={};
  if(id==="markedlil") entry=join(port.source.path,"node_modules/marked/lib/marked.esm.js");
  if(id==="zodlil") entry=join(port.source.path,"node_modules/zod/index.js");
  if(id==="posthoglil") {entry=join(port.source.path,"official/entry.ts");options={alias:{"@":join(port.source.path,"vendor/posthog-js/packages/core/src")},tsconfigRaw:{compilerOptions:{}}};}
  const bundled=await build({entryPoints:[entry],bundle:true,write:false,metafile:true,format:"esm",target:"es2020",platform:"neutral",legalComments:"none",...options});
  for(const input of Object.keys(bundled.metafile.inputs)) pin(input);
  const reference=write(`${id}.reference.mjs`,bundled.outputFiles[0].text);
  write(`${id}.reference.metafile.json`,bundled.metafile);
  const observed=pin(join(here,workload));
  // Fixed documents and independently authored expected values are visible in
  // the workload source; neither implementation supplies the expected result.
  if(id==="markedlil") {
    const texts=["<h1>Hello</h1>\n<p>A <strong>small</strong> <a href=\"https://example.com\">link</a>.</p>\n","<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n","<p>plain <em>text</em></p>\n"];
    expected.characters=iterations*texts.reduce((sum,text)=>sum+text.length,0);
  }
  workloads.push({id,contract:{target:"ES2020 ESM",surface:id==="markedlil"?["parse"]:id==="zodlil"?["z.object","z.number","z.string","z.array","z.boolean"]:["parseUuid"],
    semantics:"Standard unmodified builtins; fixed deterministic input operations in the pinned workload; no API-wide parity claim",candidate_source_receipt:resolve(args["ports-report"])},
    workload:observed,iterations,warmup_runs:5,expected,counters,runtime_limits:{steady_cpu_us:.05},
    implementations:{reference:{entry:reference,inventory:inventory(reference)},candidate:{entry:candidate,inventory:inventory(candidate)}}});
  receipt.artifacts.push({id,reference:{path:reference,...fileIdentity(reference)},candidate:{path:candidate,...fileIdentity(candidate)},compiler:ports.compiler});
}
const micro=compile(join(here,"micro.lil"),"micro","brotli");
let calls=0;
// Independent arithmetic recurrence, performed at preparation time, never by
// importing either runtime implementation or reading the compiler's output.
for(let value=0;value<256;value++) {let result=value;for(let index=0;index<32;index++) result=((result+index)|0)^(index*4);calls+=result;}
workloads.push({id:"generic-allocation-calls-strings",contract:{target:"ES2020 ESM",exports:["allocation","calls","strings"],semantics:"signed int32; standard builtins"},
  workload:pin(join(here,"micro-workload.mjs")),iterations:65536,warmup_runs:5,
  expected:{allocations:256*(5*255*256/2+256*10),lengths:65536/4*19,calls:calls*256},counters:{calls:65536*3,iterations:65536},
  runtime_limits:{steady_cpu_us:.05},implementations:{reference:{entry:pin(join(here,"micro-reference.mjs")),inventory:inventory(join(here,"micro-reference.mjs"))},
    candidate:{entry:micro.artifact,inputs:[join(here,"micro.lil"),micro.config],inventory:inventory(micro.artifact)}}});
write("runtime.json",{schema:2,workloads});

const programs=[];
const fixtures=resolve(here,"../contracts/fixtures");
for(const [id,boundary,format] of [["library","open-library","esm"],["application","closed-application","script"],["consumer","consumer-bundle","script"]]) {
  const artifacts={};let config;
  for(const objective of ["raw","gzip","brotli"]) {const built=compile(join(fixtures,`${id}.lil`),id,objective);artifacts[objective]=built.artifact;config=built.config;}
  const entry=pin(join(fixtures,`${id}.${id==="application"?"js":"mjs"}`));
  programs.push({id,contract:{boundary,format,target:"es2020",exports:id==="library"?["transform"]:[],externals:[],reflected_properties:[],
    semantics:{arithmetic:"signed int32, explicitly coerced in JavaScript",builtins:"standard unmodified; console.log observed",reflection:"only declared exports"},
    equivalence:"Same bounded integer recurrence/function with int32 wraparound; consumer imports the same transform contract"},
    upstream:{entry,inputs:[pin(join(fixtures,"library.mjs"))],...(id==="application"?{minified:entry}:{})},
    candidate:{artifacts,inputs:[pin(join(fixtures,`${id}.lil`)),pin(join(fixtures,"library.lil")),...Object.keys(artifacts).map(codec=>join(output,`${id}.${codec}.toml`))],provenance:{compiler:resolve(args.compiler),config,receipt:join(output,"preparation.json")}},
    oracle:{entry:pin(join(fixtures,id==="library"?"library-oracle.mjs":"console-oracle.mjs")),expected:id==="library"?{exports:["transform"],values:[0,7,0,5,7]}:["4320"]}});
}
write("bars.json",{schema:1,toolchain:resolve(here,"../contracts"),codec:resolve(args.codec),inspector:resolve(args.inspector),programs});
for(const [path,identity] of Object.entries(receipt.inputs)) if(fileIdentity(path).sha256!==identity.sha256) throw new Error(`preparation input changed: ${path}`);
receipt.complete=true;
write("preparation.json",receipt);
console.log(JSON.stringify({output,workloads:workloads.map(row=>row.id),contracts:programs.map(row=>row.id)}));
