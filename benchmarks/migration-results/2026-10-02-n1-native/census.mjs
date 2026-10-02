// One target-checking census; no C compiler/runtime or JavaScript search.
import {discoverCases,composeConfig,LANES,maskFor} from '../../../scripts/cases.mjs';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';
const compiler=resolve(process.argv[2]), dir=resolve(process.argv[3]), destination=resolve(process.argv[4]);
mkdirSync(dir,{recursive:true});
const lane=LANES.find(row=>row.id==='formation-only/raw/c'),rows=[];
for(const [index,item] of discoverCases().entries()) {
 const config=join(dir,`${index}.toml`);
 writeFileSync(config,composeConfig(lane,[],item.toml?readFileSync(item.toml,'utf8'):null));
 const result=spawnSync(compiler,[item.source,'--config',config,'--target','native','--check'],{encoding:'utf8',timeout:20000});
 const diagnostic=result.stderr.trim();
 rows.push({id:item.id,input_digest:item.digest,masks:maskFor(item,'c'),status:result.status===0?'admitted':result.signal?'crash-or-timeout':'refused',exit:result.status,source_qualified:/--> .*:\d+:\d+/.test(diagnostic),diagnostic:result.status===0?null:diagnostic});
}
const counts={};for(const row of rows)counts[row.status]=(counts[row.status]||0)+1;
const missing=rows.filter(row=>row.status!=='admitted'&&!row.source_qualified);
writeFileSync(destination,JSON.stringify({schema:1,scope:'Target checking with common semantic normalization; no C rendering, toolchain, execution or optional JavaScript search; case controls retained',compiler:{path:compiler,sha256:createHash('sha256').update(readFileSync(compiler)).digest('hex')},counts,rows},null,2)+'\n');
console.log(JSON.stringify({counts,without_source:missing.map(({id,diagnostic})=>({id,diagnostic:diagnostic.slice(0,450)}))}));
if(rows.some(row=>row.status==='crash-or-timeout')||missing.some(row=>row.diagnostic.includes('compiler (native)')))process.exitCode=1;
