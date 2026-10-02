// A bounded runtime-ownership qualification batch, not the library/fleet suite.
// node tests/native/qualify-ownership.mjs /absolute/lilscript /absolute/output
import {spawn} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync,copyFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const compiler=resolve(process.argv[2]),work=resolve(process.argv[3]);
const fixture=process.argv.find(arg=>arg.startsWith('--fixture='))?.slice('--fixture='.length)||'tests/native/ownership';
mkdirSync(work,{recursive:true});
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const rows=[];
async function run(name,command,args,expected) {
 const started=performance.now();
 const result=await new Promise(resolve=>{
  const child=spawn(command,args,{detached:process.platform!=='win32',env:{...process.env,ASAN_OPTIONS:'detect_leaks=1:abort_on_error=1',UBSAN_OPTIONS:'halt_on_error=1'}});
  let stdout='',stderr='',error,killTimer;
  const kill=signal=>{try{if(process.platform==='win32')child.kill(signal);else process.kill(-child.pid,signal);}catch{}};
  const timer=setTimeout(()=>{error='qualification timed out after 60000 ms';kill('SIGTERM');killTimer=setTimeout(()=>kill('SIGKILL'),1000);},60000);
  child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
  child.stdout.on('data',data=>stdout+=data);child.stderr.on('data',data=>stderr+=data);
  child.on('error',value=>error=String(value));
  child.on('close',(status,signal)=>{clearTimeout(timer);clearTimeout(killTimer);resolve({stdout,stderr,error,status,signal});});
 });
 writeFileSync(join(work,`${name}.log`),(result.stdout||'')+(result.stderr||'')+(result.error?String(result.error):''));
 const row={name,command,args,exit:result.status,signal:result.signal,elapsed_ms:performance.now()-started,passed:!result.error&&result.status===0&&(expected===undefined||result.stdout===expected)};
 rows.push(row);console.log(`${name}: ${row.passed?'passed':'FAILED'}`);
 return row.passed;
}
const compilers=[['gcc','/usr/bin/cc',[]],['clang',process.env.LILSCRIPT_NATIVE_CLANG||'clang',[]],['clang-sanitize',process.env.LILSCRIPT_NATIVE_CLANG||'clang',['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']]];
for(const [name,cc,extra] of (process.argv.includes("--generated-only")?[]:compilers)) {
 const out=join(work,`runtime-${name}`);
 if(await run(`runtime-${name}-build`,cc,['-std=c11','-O2','-Wall','-Wextra','-Werror',...extra,'tests/native/runtime-ownership.c','-lm','-o',out]))await run(`runtime-${name}-run`,out,[],'runtime ownership: passed\n');
}
const c=join(work,'ownership.c');
if(await run('source',compiler,[`${fixture}.lil`,'--config',`${fixture}.toml`,'--target','c','-o',c])) {
 const expected=readFileSync(`${fixture}.out`,'utf8');
 const host=join(work,'host.c');copyFileSync(`${fixture}.host.c`,host);
 for(const [name,cc,extra] of compilers) {
  const out=join(work,`ownership-${name}`);
  if(await run(`ownership-${name}-build`,cc,['-std=c11','-O2','-Wall','-Wextra','-Werror','-DLS_NATIVE_QUALIFICATION',...extra,c,host,'-lm','-o',out]))await run(`ownership-${name}-run`,out,[],expected);
 }
}
writeFileSync(join(work,'qualification.json'),JSON.stringify({schema:1,compiler:{path:compiler,sha256:hash(compiler)},fixtures:['tests/native/runtime-ownership.c',`${fixture}.lil`,`${fixture}.host.c`,`${fixture}.toml`,`${fixture}.out`].map(path=>({path,sha256:hash(path)})),rows},null,2)+'\n');
if(rows.some(row=>!row.passed))process.exitCode=1;
