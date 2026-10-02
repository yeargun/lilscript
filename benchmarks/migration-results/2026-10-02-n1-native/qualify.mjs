// Focused public CLI qualification. No fleet/library suite is run here.
// node qualify.mjs /absolute/pinned/lilscript /absolute/output-directory
import {execFileSync,spawnSync} from 'node:child_process';
import {mkdirSync,readFileSync,writeFileSync,existsSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const compiler=resolve(process.argv[2]), directory=resolve(process.argv[3]);
mkdirSync(directory,{recursive:true});
const hash=data=>createHash('sha256').update(data).digest('hex');
const put=(name,text)=>{const path=join(directory,name);writeFileSync(path,text);return path;};
const rows=[];
function run(args,{expected=0,env=process.env}={}){
 const result=spawnSync(compiler,args,{encoding:'utf8',env,timeout:60000});
 assert.equal(result.status,expected,`${args.join(' ')}\n${result.stderr}`);
 return result;
}
const source=put('counter.lil','int counter=1;int next(){counter+=1;return counter;}print(next());print(next());');
for(const permission of ['off','on']){
 const config=put(`${permission}.toml`, `effort.level=0\n[policy.tactics]\ndead-code-elimination='${permission}'\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'\n[target.native]\nobjective='balanced'\n`);
 const c=join(directory,`${permission}.c`),executable=join(directory,`${permission}.exe`);
 const report=JSON.parse(run([source,'--target','c','--config',config,'-o',c,'--explain','json']).stderr);
 run([c,'--target','native','--link-c','--config',config,'-o',executable]);
 assert.equal(execFileSync(executable,{encoding:'utf8'}),'2\n3\n');
 assert.equal(readFileSync(c,'utf8').includes('ls_native_unbound'),permission==='off');
 assert.equal(report.native_delivery.initialization_guards_removed>0,permission==='on');
 const receipt=JSON.parse(readFileSync(`${executable}.native.json`,'utf8'));
 assert.equal(receipt.output.sha256,hash(readFileSync(executable)));
 assert.equal(receipt.c_sha256,hash(readFileSync(c)));
 assert.ok(receipt.flags.includes('-O2'));
 rows.push({name:`global-guards-${permission}`,source_sha256:hash(readFileSync(source)),c_bytes:readFileSync(c).length,c_sha256:hash(readFileSync(c)),removed:report.native_delivery.initialization_guards_removed,baseline_work:report.resources.baseline_work,native_ns:report.phases_ns.native_ns,toolchain:receipt});
}
// Checks neither execute a missing C compiler nor write the requested path.
const checkConfig=put('check.toml',"effort.level=0\n[target.native]\ncompiler='/nonexistent/native-check-compiler'\n");
const absent=join(directory,'check-must-not-write');
run([source,'--target','native','--check','--config',checkConfig,'-o',absent]);
assert.ok(!existsSync(absent));
rows.push({name:'target-check-without-compiler',passed:true});
const dependency=put('bad-value.lil','struct Box{int[] values;}\nexport void run(){Box b=Box{[1,2]};print(b.values[0]);}');
const entry=put('bad-entry.lil','import {run} from "./bad-value.lil";run();');
const refused=run([entry,'--target','native','--check','--config',checkConfig],{expected:1});
assert.ok(refused.stderr.includes('bad-value.lil'));assert.ok(refused.stderr.includes('native managed struct field'));
rows.push({name:'dependency-diagnostic',source_sha256:hash(readFileSync(dependency)),diagnostic:refused.stderr});
// Compile a provider with the exact newly staged header and configured source.
const provider=put('provider.lil','extern int compute(int n);print(compute(7));');
put('host.c','#include "provider.h"\nint32_t host_compute(int32_t n){return n*3;}\n');
const providerConfig=put('provider.toml',"effort.level=0\n[host]\nnative_sources=['host.c']\n[host.native]\ncompute='host_compute'\n[target.native]\nobjective='balanced'\n");
const output=join(directory,'output','provider');
run([provider,'--target','native','--config',providerConfig,'-o',output]);
assert.equal(execFileSync(output,{encoding:'utf8'}),'21\n');
const providerReceipt=JSON.parse(readFileSync(`${output}.native.json`,'utf8'));
assert.equal(providerReceipt.header_sha256,hash(readFileSync(`${output}.h`)));
assert.equal(providerReceipt.host_inputs.length,1);
rows.push({name:'provider-header-and-relative-source',toolchain:providerReceipt});
// Early callback during initialization must retain the runtime guard.
const reentry=put('reentry.lil','extern int install(func()->int f);int later=install(read);int read(){return later;}print(read());');
const reentryConfig=put('reentry.toml',"effort.level=0\n[policy.tactics]\ndead-code-elimination='on'\ninlining='off'\n[host.native]\ninstall='host_install'\n");
const reentryC=join(directory,'reentry.c');
const reentryReport=JSON.parse(run([reentry,'--target','c','--config',reentryConfig,'-o',reentryC,'--explain','json']).stderr);
assert.ok(readFileSync(reentryC,'utf8').includes('ls_native_unbound'));
assert.equal(reentryReport.native_delivery.initialization_guards_removed,0);
rows.push({name:'reentry-guard-retained',c_sha256:hash(readFileSync(reentryC)),native_delivery:reentryReport.native_delivery});
// The actual run() entry point must reach all-objective/native delivery.
const all=put('all.lil','print(5);');
const allConfig=put('all.toml',"effort.level=0\n[objective]\ncodecs=['raw','gzip','brotli']\n[delivery]\nentry_names='main.mjs'\n[target.native]\nobjective='size'\n");
const allOut=join(directory,'all-output');
run([all,'--target','all','--config',allConfig,'--out-dir',allOut]);
for(const codec of ['raw','gzip','brotli'])assert.equal(execFileSync('node',[join(allOut,codec,'main.mjs')],{encoding:'utf8'}),'5\n');
assert.equal(execFileSync(join(allOut,'native','all'),{encoding:'utf8'}),'5\n');
rows.push({name:'all-objectives-and-native-cli',toolchain:JSON.parse(readFileSync(join(allOut,'native','all.native.json'),'utf8'))});
const report={schema:1,compiler:{path:compiler,sha256:hash(readFileSync(compiler))},rows};
writeFileSync(join(directory,'qualification.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify({passed:rows.length,compiler:report.compiler,c_bytes:rows.filter(row=>row.c_bytes).map(({name,c_bytes,removed})=>({name,c_bytes,removed}))}));
