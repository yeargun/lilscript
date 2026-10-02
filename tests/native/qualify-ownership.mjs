// A bounded runtime-ownership qualification batch, not the library/fleet suite.
// node tests/native/qualify-ownership.mjs /absolute/lilscript /absolute/output
import {spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync,copyFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const compiler=resolve(process.argv[2]),work=resolve(process.argv[3]);
mkdirSync(work,{recursive:true});
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const rows=[];
function run(name,command,args,expected) {
 const result=spawnSync(command,args,{encoding:'utf8',timeout:60000,env:{...process.env,ASAN_OPTIONS:'detect_leaks=1:abort_on_error=1',UBSAN_OPTIONS:'halt_on_error=1'}});
 writeFileSync(join(work,`${name}.log`),(result.stdout||'')+(result.stderr||'')+(result.error?String(result.error):''));
 const row={name,command,args,exit:result.status,signal:result.signal,passed:result.status===0&&(expected===undefined||result.stdout===expected)};
 rows.push(row);console.log(`${name}: ${row.passed?'passed':'FAILED'}`);
 return row.passed;
}
const compilers=[['gcc','/usr/bin/cc',[]],['clang',process.env.LILSCRIPT_NATIVE_CLANG||'clang',[]],['clang-sanitize',process.env.LILSCRIPT_NATIVE_CLANG||'clang',['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']]];
for(const [name,cc,extra] of (process.argv.includes("--generated-only")?[]:compilers)) {
 const out=join(work,`runtime-${name}`);
 if(run(`runtime-${name}-build`,cc,['-std=c11','-O2','-Wall','-Wextra','-Werror',...extra,'tests/native/runtime-ownership.c','-lm','-o',out]))run(`runtime-${name}-run`,out,[],'runtime ownership: passed\n');
}
const c=join(work,'ownership.c');
if(run('source',compiler,['tests/native/ownership.lil','--config','tests/native/ownership.toml','--target','c','-o',c])) {
 const expected=readFileSync('tests/native/ownership.out','utf8');
 const host=join(work,'host.c');copyFileSync('tests/native/ownership.host.c',host);
 for(const [name,cc,extra] of compilers) {
  const out=join(work,`ownership-${name}`);
  if(run(`ownership-${name}-build`,cc,['-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',...extra,c,host,'-lm','-o',out]))run(`ownership-${name}-run`,out,[],expected);
 }
}
writeFileSync(join(work,'qualification.json'),JSON.stringify({schema:1,compiler:{path:compiler,sha256:hash(compiler)},fixtures:['tests/native/runtime-ownership.c','tests/native/ownership.lil','tests/native/ownership.host.c','tests/native/ownership.toml','tests/native/ownership.out'].map(path=>({path,sha256:hash(path)})),rows},null,2)+'\n');
if(rows.some(row=>!row.passed))process.exitCode=1;
