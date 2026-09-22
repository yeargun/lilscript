import {readFileSync,writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
import {brotliCompressSync,gzipSync,constants} from 'node:zlib';
const root=process.argv[2]??'/tmp/motionlil-size-rootcause-20260911/repo',out=process.argv[3]??'/tmp/motionlil-size-rootcause-20260911';
const require=createRequire(root+'/package.json'),{build}=require('esbuild'),{minify}=require('terser');
const results={};
for(const level of [3,9]){
 const compat=`.__diagnostic-compat${level}.mjs`,compiled=`.__compiler-level${level}.mjs`;
 writeFileSync(root+'/src/'+compat,readFileSync(root+'/src/compat.mjs','utf8').replaceAll('./.__compiled-index.mjs','./'+compiled));
 const source=`export * from './${compiled}';export * from './${compat}';export {animate,animateMini} from './${compat}';`;
 const b=await build({stdin:{contents:source,resolveDir:root+'/src',sourcefile:'entry.mjs'},bundle:true,platform:'browser',format:'esm',target:'es2022',treeShaking:true,legalComments:'none',write:false});
 const m=await minify(b.outputFiles[0].text,{module:true,compress:{passes:3},mangle:{toplevel:true,properties:{regex:/^_/,keep_quoted:true}},format:{comments:false}});
 const bytes=Buffer.from(m.code+'\n');writeFileSync(out+`/full-level${level}.js`,bytes);
 results[level]={raw:bytes.length,gzip9:execFileSync('python3',['-c','import gzip,sys;sys.stdout.buffer.write(gzip.compress(sys.stdin.buffer.read(),compresslevel=9,mtime=0))'],{input:bytes}).length,brotli11:brotliCompressSync(bytes,{params:{[constants.BROTLI_PARAM_QUALITY]:11}}).length};
}
writeFileSync(out+'/optimization-levels.json',JSON.stringify(results,null,2)+'\n');console.log(results);
