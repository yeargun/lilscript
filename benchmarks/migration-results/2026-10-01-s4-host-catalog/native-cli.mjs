import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const directory=dirname(fileURLToPath(import.meta.url));
const folder=resolve(directory,'artifacts/native-cli');mkdirSync(folder,{recursive:true});
const compiler='/home/azureuser/lilscript-work/bin/s4-host-catalog-1/lilscript';
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const run=(command,args)=>{const r=spawnSync(command,args,{encoding:'utf8',timeout:60000,maxBuffer:8*1024*1024});if(r.status!==0)throw Error(`${command}: ${r.stderr||r.error}`);return r.stdout;};
const entry=resolve(folder,'main.lil'),config=resolve(folder,'lilscript.toml');
writeFileSync(entry,'extern int compute(int n);print(compute(7));\n');
writeFileSync(config,"[host]\nnative_sources=['provider.c']\n[host.native]\ncompute='host_compute'\n");
writeFileSync(resolve(folder,'provider.c'),'#include "main.h"\nint32_t host_compute(int32_t n){return n*3;}\n');
run(compiler,[entry,'--config',config,'--target','c','-o',resolve(folder,'main.c')]);
const header=hash(resolve(folder,'main.h'));
// Keep executables outside the tracked evidence; source/header hashes and the
// observed behavior are the receipt, not a machine-dependent binary claim.
const executable='/tmp/lilscript-s4-host-catalog-cli-main';
run(compiler,[entry,'--config',config,'--target','native','-o',executable]);
const output=run(executable,[]);
if(output!=='21\n')throw Error(JSON.stringify({output}));
if(hash('/tmp/lilscript-s4-host-catalog-cli-main.h')!==header)throw Error('header differs between C and executable delivery');
writeFileSync(resolve(directory,'native-cli.json'),JSON.stringify({compiler:{path:compiler,sha256:hash(compiler)},scope:'CLI C/header delivery and separately compiled provider from a TOML-relative path; not native-complete qualification',sources:['main.lil','lilscript.toml','provider.c','main.c','main.h'].map(path=>({path:'artifacts/native-cli/'+path,sha256:hash(resolve(folder,path))})),output},null,2)+'\n');
console.log('native CLI provider and header passed');
