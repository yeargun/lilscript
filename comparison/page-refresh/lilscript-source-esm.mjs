import {readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const base=resolve(process.argv[2]),port=join(base,'ports/zodlil'),out=join(base,'results/zodlil');
const {build}=await import(pathToFileURL(join(base,'toolchain/node_modules/esbuild/lib/main.js')));
const samples=[];let metafile;
for(let i=0;i<3;i++){
 const start=performance.now();
 const result=await build({entryPoints:[join(port,'dist/index.js')],bundle:true,format:'esm',platform:'browser',target:'esnext',minify:true,legalComments:'none',outfile:join(out,'lilscript.esm.js'),metafile:true,plugins:[{name:'source-locales-esm',setup(b){b.onLoad({filter:/\/dist\/compat\.js$/},({path})=>({contents:readFileSync(path,'utf8').replace('import { createRequire } from "node:module"','import * as originalLocales from '+JSON.stringify(join(base,'upstream/zodlil/packages/zod/v4/locales/index.js'))).replace('const require = createRequire(import.meta.url)','').replace('require("zod/v4/locales")','originalLocales'),loader:'js'}))}}]});
 samples.push({wallSeconds:(performance.now()-start)/1000});metafile=result.metafile;
}
const file=join(out,'lilscript.esm.js'),scored=JSON.parse(execFileSync(join(base,'tools/lilscript-codec'),['--json',file],{encoding:'utf8'})).artifacts[0];
writeFileSync(join(out,'lilscript-esm.json'),JSON.stringify({...scored,path:'lilscript.esm.js',sha256:createHash('sha256').update(readFileSync(file)).digest('hex'),samples,assembly:'Complete public dist/index.js graph, including compatibility, async, JSON Schema and all original locale modules. esbuild 0.28.1 bundle/minify. The Node createRequire locale import is expressed as the equivalent ESM namespace import from the pinned source-built upstream locale graph.',inputs:Object.keys(metafile.inputs)},null,2)+'\n');
