import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {basename, dirname, relative, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {buildFoundation} from './compiler-delivery.mjs';
import {compilerPath, projectRoot} from '../tooling/compiler-path.mjs';

// A consumer's selected public surface is a typed source entry. The compiler
// owns linking, naming, wrappers and every JavaScript byte in the result.
export async function buildSelectedReactivity(exportNames, output) {
  const names = [...new Set(exportNames)].sort();
  const all = JSON.parse(readFileSync(resolve(projectRoot,'src/package/exports.json'),'utf8')).reactivity;
  if (!names.length || names.some(name=>!all.includes(name))) throw Error(`Unknown or empty @vue/reactivity export selection: ${names}`);
  const compiler = compilerPath();
  const scratch = resolve(projectRoot,'.tmp');
  mkdirSync(scratch,{recursive:true});
  const temporary = mkdtempSync(resolve(scratch,'selected-reactivity-'));
  const stage = resolve(temporary,'output');
  try {
    const specifier = path => './'+relative(temporary,resolve(projectRoot,path)).replaceAll('\\','/');
    const metadata = readFileSync(resolve(projectRoot,'src/package/reactivity.lil'),'utf8').split('\n')
      .filter(line=>names.some(name=>line.startsWith(`metadata(${name},`))).join('\n');
    writeFileSync(resolve(temporary,'entry.lil'),[
      `import {${names.join(',')}} from ${JSON.stringify(specifier('src/reactivity/index.lil'))};`,
      `import extern {metadata} from ${JSON.stringify(specifier('src/package/metadata.mjs'))};`,
      'extern void metadata(JsValue value,string name,int length);',metadata,
      `export {${names.join(',')}};`,''
    ].join('\n'));
    const filename=basename(output);
    if (!/^[a-zA-Z0-9_.-]+\.(?:js|mjs)$/.test(filename)) throw Error('Invalid selected entry filename');
    const config=resolve(temporary,'lilscript.toml');
    writeFileSync(config,`[policy]\nversion=3\n[optimization]\npreset="maximum"\n[javascript]\nassume_pristine_builtins=false\nassume_pure_property_reads=false\n[objective]\ncodecs="brotli"\n[effort]\nlevel=15\n[defines]\nDEV=false\nTEST=false\n[delivery]\nmode="split"\ndirectory="."\nhost_modules="embed"\nentry_names=${JSON.stringify(filename)}\nchunk_names=${JSON.stringify(filename+'.chunks/[name]-[index].js')}\n[delivery.entries]\nselected="entry.lil"\n`);
    const args=['--config',config,'--target','js-module','--mode',process.env.LILSCRIPT_BUILD_MODE??'production','--out-dir',stage,'--logical-work',process.env.LILSCRIPT_LOGICAL_WORK??'40000000000'];
    const compiled=spawnSync(compiler,args,{cwd:projectRoot,encoding:'utf8',maxBuffer:16*1024*1024});
    if(compiled.status!==0)throw Error(compiled.stderr||compiled.stdout||'Selected compilation failed');
    const manifest=JSON.parse(readFileSync(resolve(stage,'lilscript.manifest.json'),'utf8'));
    if(![3,4,5].includes(manifest.version))throw Error('Selected build requires compiler receipts');
    for(const group of manifest.outputs)for(const file of group.files){
      const bytes=readFileSync(resolve(stage,file.file));
      if(createHash('sha256').update(bytes).digest('hex')!==file.sha256)throw Error(`Selected artifact changed: ${file.file}`);
      const target=resolve(dirname(output),file.file);
      mkdirSync(dirname(target),{recursive:true});writeFileSync(target,bytes);
    }
    const code=readFileSync(output,'utf8');
    writeFileSync(output+'.manifest.json',JSON.stringify(manifest,null,2)+'\n');
    return {code,exportNames:names,sourcePath:resolve(projectRoot,'src/reactivity/index.lil'),sourceKind:'complete-reactivity',manifest};
  } finally {rmSync(temporary,{recursive:true,force:true});}
}

if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) await buildFoundation();
