import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
const root=resolve(process.argv[2]);
const {fromSource}=await import(pathToFileURL(join(root,'scripts/lib/official.mjs')));
const samples=[];
const work=mkdtempSync(join(tmpdir(),'page-audit-katex-source-'));
try {
for(let index=0;index<3;index++) {
  const start=performance.now();
  const source=await fromSource({work});
  writeFileSync(join(root,'.page-audit/upstream/official.source-terser.js'),source.code);
  samples.push((performance.now()-start)/1000);
}
} finally { rmSync(work,{recursive:true,force:true}); }
const resultPath=join(root,'.page-audit/result.json');
const result=JSON.parse(readFileSync(resultPath,'utf8'));
result.upstreamSourceBuild={samples,medianSeconds:[...samples].sort((a,b)=>a-b)[1],scope:'Pinned KaTeX Flow source tree: Babel type stripping, esbuild bundling and Terser passes=3; three sequential samples, dependencies already installed.'};
writeFileSync(resultPath,JSON.stringify(result,null,2)+'\n');
console.log(result.upstreamSourceBuild);
