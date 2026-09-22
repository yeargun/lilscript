import {readFileSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {join, relative} from 'node:path';
import {pathToFileURL} from 'node:url';
import {brotliCompressSync, gzipSync, constants} from 'node:zlib';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root=process.argv[2]??'/tmp/motionlil-size-rootcause-20260911/repo',out=process.argv[3]??'/tmp/motionlil-size-rootcause-20260911';
const require=createRequire(join(root,'package.json'));
const {build}=require('esbuild'),{minify}=require('terser'),{parse}=require('acorn');
const {TraceMap,decodedMappings}=require('@jridgewell/trace-mapping');
const read=p=>readFileSync(p,'utf8');
const sizes=s=>{const b=Buffer.from(s);return {raw:b.length,gzip9:execFileSync('python3',['-c','import gzip,sys;sys.stdout.buffer.write(gzip.compress(sys.stdin.buffer.read(),compresslevel=9,mtime=0))'],{input:b}).length,brotli11:brotliCompressSync(b,{params:{[constants.BROTLI_PARAM_QUALITY]:11}}).length,sha256:createHash('sha256').update(b).digest('hex')}};
const options={module:true,compress:{passes:3},mangle:{toplevel:true,properties:{regex:/^_/,keep_quoted:true}},format:{comments:false}};
const compat=join(root,'src/.__diagnostic-compat.mjs');
writeFileSync(compat,read(join(root,'src/compat.mjs')).replaceAll('./.__compiled-index.mjs','./.__compiled-full.mjs'));
const facade=join(root,'src/.__diagnostic-full.mjs');
writeFileSync(facade,'export * from "./.__compiled-full.mjs";export * from "./.__diagnostic-compat.mjs";export {animate,animateMini} from "./.__diagnostic-compat.mjs";');
async function bundle(label,source,{retainMap=false}={}){
 const b=await build({stdin:{contents:source,resolveDir:join(root,'src'),sourcefile:'diagnostic-entry.mjs'},bundle:true,platform:'browser',format:'esm',target:'es2022',treeShaking:true,legalComments:'none',logLevel:'warning',write:false,outfile:join(out,label+'.js'),metafile:true,sourcemap:'external'});
 const code=b.outputFiles.find(f=>f.path.endsWith('.js')).text,map=b.outputFiles.find(f=>f.path.endsWith('.js.map')).text;
 const m=await minify(code,{...options,sourceMap:{content:map,asObject:true}});
 writeFileSync(join(out,label+'.js'),m.code+'\n');
 const result={...sizes(m.code+'\n')};
 if(retainMap){
  const tm=new TraceMap(m.map),lines=m.code.split('\n'),counts={};
  decodedMappings(tm).forEach((segs,line)=>segs.forEach((seg,i)=>{
   if(seg.length<4)return;
   const source=tm.sources[seg[1]],end=segs[i+1]?.[0]??lines[line].length;
   counts[source]=(counts[source]??0)+Buffer.byteLength(lines[line].slice(seg[0],end));
  }));
  result.minifiedSourceAttribution=Object.fromEntries(Object.entries(counts).sort((a,b)=>b[1]-a[1]));
  writeFileSync(join(out,label+'.map.json'),JSON.stringify(m.map));
  result.inputContributions=b.metafile.outputs[Object.keys(b.metafile.outputs).find(x=>x.endsWith('.js'))].inputs;
 }
 return result;
}
const pub=await import(pathToFileURL(join(root,'dist/index.js'))),full=await import(pathToFileURL(join(root,'dist/full.js'))),original=await import(pathToFileURL(join(root,'site/esm-comparison/original.js')));
const shared=Object.keys(pub).filter(k=>k in original).sort(),names=shared.join(',');
const results={scope:{defaultExports:Object.keys(pub).length,fullExports:Object.keys(full).length,originalExports:Object.keys(original).length,sharedDefaultExports:shared,portOnlyDefaultExports:Object.keys(pub).filter(k=>!(k in original))}};
results.full=await bundle('full-mapped','export * from "./.__diagnostic-full.mjs";',{retainMap:true});
results.coreOnly=await bundle('core-only','export * from "./.__compiled-full.mjs";');
results.sharedOriginal=await bundle('shared-original',`export {${names}} from "../site/esm-comparison/original.js";`);
results.sharedModularPort=await bundle('shared-modular-port',`export {${names}} from "../dist/index.js";`);
results.sharedMonolithicPort=await bundle('shared-monolithic-port',`export {${names}} from "./.__diagnostic-full.mjs";`);
function astStats(source){
 const ast=parse(source,{ecmaVersion:'latest',sourceType:'module'}),nodes={},ops={},names={},literals={},functions=[];
 function walk(n){if(!n||typeof n!=='object')return;if(n.type){nodes[n.type]=(nodes[n.type]??0)+1;if(n.operator)ops[n.operator]=(ops[n.operator]??0)+1;if(n.type==='MemberExpression'&&!n.computed&&n.property?.type==='Identifier')names[n.property.name]=(names[n.property.name]??0)+1;if(n.type==='Literal'&&typeof n.value==='string')literals[n.value]=(literals[n.value]??0)+1;}
  for(const [k,v] of Object.entries(n)){if(k==='start'||k==='end')continue;if(Array.isArray(v))v.forEach(walk);else if(v&&typeof v==='object')walk(v);}}
 walk(ast);
 for(const n of ast.body){if(n.type==='FunctionDeclaration')functions.push({name:n.id.name,raw:n.end-n.start,head:source.slice(n.start,Math.min(n.start+140,n.end))});}
 return {nodes,operators:ops,memberNames:Object.entries(names).sort((a,b)=>b[1]-a[1]).slice(0,35),stringLiterals:Object.entries(literals).sort((a,b)=>b[1]-a[1]).slice(0,25),largestFunctions:functions.sort((a,b)=>b.raw-a.raw).slice(0,20)};
}
results.ast={port:astStats(read(join(root,'dist/full.js'))),original:astStats(read(join(root,'site/esm-comparison/original.js')))};
writeFileSync(join(out,'size-attribution.json'),JSON.stringify(results,null,2)+'\n');
console.log(JSON.stringify({...results,ast:undefined,full:{...results.full,inputContributions:undefined}},null,2));
