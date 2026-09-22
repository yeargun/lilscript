// Assemble the comparison graph from the freshly built upstream Git checkout.
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const base=resolve(process.argv[2]), name=process.argv[3];
const config=JSON.parse(readFileSync(join(base,'jobs.json'),'utf8'))[name];
const root=join(base,'upstream',name), port=join(base,'ports',name), out=join(base,'results',name);
const {build,transform}=await import(pathToFileURL(join(base,'toolchain/node_modules/esbuild/lib/main.js')));
const {minify}=await import(pathToFileURL(join(base,'toolchain/node_modules/terser/main.js')));
let options={absWorkingDir:root,bundle:true,format:'esm',platform:'browser',target:'esnext',write:false,outdir:out,legalComments:'none',external:config.external??[],conditions:['browser','import','production'],define:{'process.env.NODE_ENV':'"production"'},loader:{'.ttf':'file'},logLevel:'error'};
const absolute=join(root,config.entry);
if(name==='markedlil'){
 const contents=readFileSync(join(port,'scripts/official-parse-entry.mjs'),'utf8').replaceAll('../official/marked-18.0.10/src/',join(root,'src')+'/');
 options.stdin={contents,resolveDir:root,sourcefile:'matched-parse-entry.mjs'};
}else if(name==='posthoglil'){
 // The checked-in kernel adapter imports the original algorithms. The native
 // source-repository build is recorded independently for the complete SDK.
 options.entryPoints=[join(port,'official/entry.ts')];
 options.alias={'@':join(root,'packages/core/src')};
 options.plugins=[{name:'pinned-posthog-source',setup(b){b.onResolve({filter:/vendor\/posthog-js\//},args=>({path:args.path.replace(/^.*?vendor\/posthog-js\//,root+'/')}));}}];
}else if(name==='playcanvaslil'){
 // This library's public surface is the four-module shader processing core.
 const contents=readFileSync(join(port,'benchmarks/open-world.js'),'utf8').replaceAll('../upstream/engine/',root+'/');
 options.stdin={contents,resolveDir:root,sourcefile:'shader-core-entry.mjs'};
 const {default:strip}=await import(pathToFileURL(join(port,'node_modules/@rollup/plugin-strip/dist/es/index.js')));
 const {parse}=await import(pathToFileURL(join(port,'node_modules/acorn/dist/acorn.mjs')));
 const buildScript=readFileSync(join(port,'scripts/build.mjs'),'utf8');
 const functions=JSON.parse(buildScript.match(/const stripFunctions = (\[[\s\S]*?\]);/)[1].replace(/,\s*\]/,']'));
 const plugin=strip({functions,debugger:false,sourceMap:false});
 options.plugins=[{name:'upstream-release-strip',setup(b){b.onLoad({filter:/\.js$/},({path})=>{const code=readFileSync(path,'utf8');const transformed=plugin.transform.call({parse:code=>parse(code,{ecmaVersion:'latest',sourceType:'module'})},code,path);return{contents:transformed?.code??code,loader:'js'}})}}];
}else{
 options.stdin={contents:`export * from ${JSON.stringify(absolute)};${config.defaultExport?' export {default} from '+JSON.stringify(absolute)+';':''}`,resolveDir:root,sourcefile:'public-esm-entry.mjs'};
}
if(name==='playcanvaslil') Object.assign(options,{platform:'neutral',target:'es2022',minifySyntax:true,minifyWhitespace:true,minifyIdentifiers:false});
const minifier=name==='playcanvaslil'?{module:true,ecma:2022,compress:{arrows:false,passes:3},mangle:false,format:{comments:false}}:{module:true,compress:{passes:3},mangle:true,format:{comments:false}};
const samples=[];let bundled,compressed,metafile;
for(let i=0;i<3;i++){
 const started=performance.now();const result=await build({...options,metafile:true});
 const file=result.outputFiles.find(x=>x.path.endsWith('.js'));if(!file)throw new Error('No ESM output');bundled=file.text;metafile=result.metafile;
 const linked=performance.now();compressed=(await minify(bundled,minifier)).code+'\n';
 writeFileSync(join(out,'original.esm.js'),compressed);samples.push({bundleSeconds:(linked-started)/1000,wallSeconds:(performance.now()-started)/1000});
 for(const f of result.outputFiles.filter(x=>!x.path.endsWith('.js')))writeFileSync(f.path,f.contents);
}
writeFileSync(join(out,'original.unminified.js'),bundled);
writeFileSync(join(out,'original.terser-nomangle.js'),(await minify(bundled,{module:true,compress:{passes:3},mangle:false,format:{comments:false}})).code+'\n');
writeFileSync(join(out,'original.esbuild.js'),(await transform(bundled,{format:'esm',target:'esnext',minify:true,legalComments:'none'})).code);
try{
 const vite=await import(pathToFileURL(join(port,'node_modules/vite/dist/node/index.js')));
 if(vite.minify)for(const mangle of [true,false]){const value=await vite.minify('original.js',bundled,{module:true,compress:true,mangle,codegen:{removeWhitespace:true,legalComments:'none'},sourcemap:false});if(value.errors?.length)throw new Error('Oxc rejected source ESM');writeFileSync(join(out,'original.oxc-'+(mangle?'mangle':'nomangle')+'.js'),value.code+'\n');}
}catch(e){if(e.code!=='ERR_MODULE_NOT_FOUND')throw e;}
const paths=['original.esm.js','original.unminified.js','original.terser-nomangle.js','original.esbuild.js','original.oxc-mangle.js','original.oxc-nomangle.js'].map(f=>join(out,f)).filter(existsSync);
const measured=JSON.parse(execFileSync(join(base,'tools/lilscript-codec'),['--json',...paths],{encoding:'utf8',maxBuffer:8*1024*1024}));
const artifacts=measured.artifacts.map(x=>({...x,path:x.path.slice(out.length+1),sha256:createHash('sha256').update(readFileSync(x.path)).digest('hex')}));
writeFileSync(join(out,'esm.json'),JSON.stringify({source:config.source,entry:config.entry,scope:config.esmScope,external:options.external,samples,medianSeconds:[...samples].sort((a,b)=>a.wallSeconds-b.wallSeconds)[1].wallSeconds,toolchain:{esbuild:'0.28.1',terser:'5.51.2'},artifacts,inputs:Object.keys(metafile.inputs)},null,2)+'\n');
