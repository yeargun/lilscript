#!/usr/bin/env node
// Executable benchmark contracts, independently minimized by each objective.
// No artifact is a bar until the common oracle accepts it. Size similarity is
// never used as a substitute for matched semantics or behavior.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {mkdirSync, readFileSync, writeFileSync, existsSync, readdirSync, realpathSync} from "node:fs";
import {dirname, join, resolve, isAbsolute} from "node:path";
import {fileURLToPath} from "node:url";
import {brotliCompressSync, gzipSync, constants} from "node:zlib";
import {fileIdentity, fingerprint, snapshotDependencyTree} from "../finer/tools/artifact-evidence.mjs";
import {SIZE_VERDICT_POLICY, sizeVerdict} from "./lib/size-verdict.mjs";

const scripts = dirname(fileURLToPath(import.meta.url));
export const RECIPE_IDS = ["terser", "swc", "oxc", "rolldown", "esbuild", "closure-advanced", "upstream"];
const OBJECTIVES = ["raw", "gzip", "brotli"];
const env = {PATH:"/usr/bin:/bin", TZ:"UTC", LANG:"C", LC_ALL:"C"};

export function validateContract(contract) {
  assert.ok(contract && ["open-library","closed-application","consumer-bundle"].includes(contract.boundary), "declare boundary");
  assert.ok(["esm","script"].includes(contract.format), "format must be esm or script");
  assert.equal(contract.target,"es2020","this recipe grid targets ES2020");
  for (const key of ["exports","externals","reflected_properties"]) assert.ok(Array.isArray(contract[key]), `declare ${key}`);
  assert.ok(contract.semantics && typeof contract.semantics === "object" && Object.keys(contract.semantics).length, "declare matched language guarantees and host assumptions");
  assert.ok(typeof contract.equivalence === "string" && contract.equivalence.length, "explain upstream/candidate semantic equivalence");
  assert.equal(new Set(contract.exports).size,contract.exports.length,"duplicate exports");
  for (const name of contract.exports) assert.match(name,/^(?:default|[$A-Z_a-z][$\w]*)$/u,"export must be a binding name");
}

export function minima(rows) {
  return Object.fromEntries(OBJECTIVES.map(objective => {
    const eligible = rows.filter(row => row.eligible && row.kind === "competitor");
    eligible.sort((a,b) => a.sizes[objective]-b.sizes[objective] || a.id.localeCompare(b.id));
    return [objective,eligible.length ? {id:eligible[0].id,bytes:eligible[0].sizes[objective],sha256:eligible[0].artifact.sha256} : null];
  }));
}

export function runBars({manifestPath,lockPath,work,reportPath,writeLock=false,timeout=120}) {
  for (const path of [reportPath]) assert.ok(!existsSync(path),"report exists; choose a new path");
  assert.ok(!existsSync(work) || readdirSync(work).length === 0,"work directory must be empty");
  const manifest = JSON.parse(readFileSync(manifestPath,"utf8"));
  assert.equal(manifest.schema,1);
  assert.ok(Array.isArray(manifest.programs) && manifest.programs.length,"declare programs");
  const base = dirname(manifestPath);
  const absolute = path => realpathSync(isAbsolute(path) ? path : resolve(base,path));
  const files = new Map();
  const pin = path => {path=absolute(path);files.set(path,fileIdentity(path));return path;};
  pin(manifestPath); pin(process.execPath);
  for (const name of ["bars.mjs","bars-worker.mjs","bars-recipe.mjs"]) pin(join(scripts,name));
  pin(join(scripts,"lib/size-verdict.mjs"));
  pin(join(scripts,"../finer/tools/artifact-evidence.mjs"));
  const codec = pin(manifest.codec);
  const inspector = manifest.inspector ? pin(manifest.inspector) : null;
  const toolchain = absolute(manifest.toolchain);
  pin(join(toolchain,"package.json")); pin(join(toolchain,"package-lock.json"));
  const dependencies = snapshotDependencyTree(join(toolchain,"node_modules"), {maxBytes:2*1024**3,maxEntries:100_000});
  const seen = new Set();
  const programs = manifest.programs.map(program => {
    validateContract(program.contract);
    assert.ok(typeof program.id === "string" && /^[a-zA-Z0-9_-]+$/.test(program.id) && !seen.has(program.id),"unique path-safe program id required");seen.add(program.id);
    assert.ok(Object.hasOwn(program.oracle,"expected"),"common oracle needs explicit expected result");
    const row = structuredClone(program);
    row.upstream.entry = pin(row.upstream.entry);
    row.oracle.entry = pin(row.oracle.entry);
    for (const path of [...(row.upstream.inputs??[]),...(row.oracle.inputs??[]),...(row.candidate.inputs??[])]) pin(path);
    assert.deepEqual(Object.keys(row.candidate.artifacts).sort(),[...OBJECTIVES].sort(),"declare our artifact for every objective");
    for (const objective of OBJECTIVES) row.candidate.artifacts[objective]=pin(row.candidate.artifacts[objective]);
    assert.ok(row.candidate.provenance?.compiler && row.candidate.provenance?.config,"candidate requires compiler and configuration provenance");
    const compiler=pin(row.candidate.provenance.compiler);pin(row.candidate.provenance.config);
    assert.ok(row.candidate.provenance.receipt,"candidate requires a compiler build receipt");
    const buildReceipt=JSON.parse(readFileSync(pin(row.candidate.provenance.receipt),"utf8"));
    assert.equal(buildReceipt.complete,true,"candidate preparation is incomplete");
    assert.equal(buildReceipt.inputs?.[compiler]?.sha256,fileIdentity(compiler).sha256,"candidate receipt names another compiler");
    for(const objective of OBJECTIVES) {
      const artifact=row.candidate.artifacts[objective];
      const build=buildReceipt.builds?.find(build=>build.artifact?.path===artifact && build.compiler===compiler);
      assert.ok(build,`missing actual compiler invocation for ${objective}`);
      assert.equal(build.artifact.sha256,fileIdentity(artifact).sha256,`candidate ${objective} differs from compiler build receipt`);
      const index=build.args.indexOf("--config");
      assert.ok(index>=0 && build.args[index+1],"compiler invocation lacks explicit config");
      assert.equal(fileIdentity(pin(build.args[index+1])).sha256,build.config.sha256,"compiler configuration changed after build");
    }
    if (row.externs) row.externs=pin(row.externs);
    if (row.upstream.minified) row.upstream.minified=pin(row.upstream.minified);
    return row;
  });
  const lock = {schema:1, files:Object.fromEntries([...files].sort(([a],[b])=>a.localeCompare(b))),
    dependencies:{root:dependencies.root,sha256:dependencies.sha256,files:dependencies.files},
    node:{version:process.version,versions:process.versions}};
  if (writeLock) {
    assert.ok(!existsSync(lockPath),"lock exists; choose a new path instead of silently updating identities");
    mkdirSync(dirname(lockPath),{recursive:true});writeFileSync(lockPath,JSON.stringify(lock,null,2)+"\n");
  } else assert.deepEqual(JSON.parse(readFileSync(lockPath,"utf8")),lock,"benchmark input/tool content differs from lock");
  mkdirSync(work,{recursive:true});
  const report={schema:1,kind:"qualified-benchmark-contracts",complete:false,lock_sha256:fingerprint(lock),programs:[],environment:env,
    size_verdict_policy:SIZE_VERDICT_POLICY,
    limitations:["Behavior is established over the explicit oracle, not all possible inputs.","Language/host equivalence is declared and reviewed; a tool cannot prove those source contracts automatically.","CPU for native subprocess tools is unavailable here; wall cost includes the complete child process."]};
  const invoke = (script,request,stem) => {
    const path=join(work,`${stem}.request.json`);writeFileSync(path,JSON.stringify(request)+"\n");
    const start=process.hrtime.bigint();
    try {
      const result=execFileSync(process.execPath,[join(scripts,script),path],{encoding:"utf8",env,timeout:timeout*1000,maxBuffer:16*1024**2});
      writeFileSync(join(work,`${stem}.stdout`),result);
      return {...JSON.parse(result),process_wall_ns:Number(process.hrtime.bigint()-start)};
    } catch(error) {
      writeFileSync(join(work,`${stem}.stderr`),String(error.stderr??error));
      throw error;
    }
  };
  const score = artifact => {
    const report=JSON.parse(execFileSync(codec,["--json",artifact],{encoding:"utf8",env,timeout:timeout*1000}));
    const canonical=report.artifacts[0]; const bytes=readFileSync(artifact);
    return {sizes:{raw:canonical.raw,gzip:canonical.gzip9,brotli:canonical.brotli11},codecs:report.codecs,
      robustness:{brotli5:brotliCompressSync(bytes,{params:{[constants.BROTLI_PARAM_QUALITY]:5,[constants.BROTLI_PARAM_LGWIN]:22}}).length,
        gzip6:gzipSync(bytes,{level:6}).length,node_zlib:process.versions.zlib,node_brotli:process.versions.brotli},
      inventory:inspector ? JSON.parse(execFileSync(inspector,[artifact],{encoding:"utf8",env,timeout:timeout*1000})) : null};
  };
  try {
    for (const program of programs) {
      const row={id:program.id,contract:program.contract,artifacts:[],refused:[]};report.programs.push(row);
      let entry=program.upstream.entry;
      if (program.contract.format === "esm" && program.contract.boundary === "open-library") {
        // Entry construction precedes bundling, optimization and byte scoring.
        entry=join(work,`${program.id}.entry.mjs`);
        writeFileSync(entry,`export {${program.contract.exports.join(",")}} from ${JSON.stringify(program.upstream.entry)};\n`);
      }
      const common=join(work,`${program.id}.bundle.${program.contract.format==="esm"?"mjs":"js"}`);
      const baseRequest={toolchain,entry,input:common,format:program.contract.format,externals:program.contract.externals,externs:program.externs,timeout_ms:timeout*1000};
      row.bundle=invoke("bars-recipe.mjs",{...baseRequest,recipe:"bundle",output:common},`${program.id}-bundle`);
      const verify=(path,id)=>invoke("bars-worker.mjs",{oracle:program.oracle.entry,expected:program.oracle.expected,artifact:path,format:program.contract.format},`${program.id}-${id}-oracle`);
      // A broken input baseline invalidates the contract, not merely one recipe.
      verify(common,"baseline");
      for (const objective of OBJECTIVES) {
        const path=program.candidate.artifacts[objective];
        verify(path,`ours-${objective}`);
        row.artifacts.push({id:`ours-${objective}`,kind:"candidate",objective,eligible:true,artifact:{path,...fileIdentity(path)},...score(path)});
      }
      for (const recipe of RECIPE_IDS) {
        if (recipe==="closure-advanced" && program.contract.format==="esm") {
          row.refused.push({id:recipe,status:"inapplicable",reason:"This pinned Closure CLI recipe does not preserve an open ESM export boundary. Closed script consumers are measured separately."});continue;
        }
        if (recipe==="upstream" && !program.upstream.minified) {
          row.refused.push({id:recipe,status:"inapplicable",reason:"No upstream minified artifact is declared for this exact boundary."});continue;
        }
        if (recipe==="closure-advanced" && program.contract.boundary==="open-library" && !program.externs) {
          row.refused.push({id:recipe,status:"inapplicable",reason:"An open script API requires declared Closure externs."});continue;
        }
        const output=join(work,`${program.id}.${recipe}.${program.contract.format==="esm"?"mjs":"js"}`);
        try {
          const cost=invoke("bars-recipe.mjs",{...baseRequest,recipe,input:recipe==="upstream" ? program.upstream.minified : common,output},`${program.id}-${recipe}`);
          verify(output,recipe);
          row.artifacts.push({id:recipe,kind:"competitor",eligible:true,artifact:{path:output,...fileIdentity(output)},cost,...score(output)});
        } catch(error) {row.refused.push({id:recipe,status:"failed",reason:String(error.message).slice(0,4000)});}
      }
      row.minima=minima(row.artifacts);
      assert.ok(OBJECTIVES.every(objective=>row.minima[objective]),"no eligible competitor for one or more objectives");
      row.deltas=Object.fromEntries(OBJECTIVES.map(objective=>[objective,
        row.artifacts.find(artifact=>artifact.objective===objective).sizes[objective]-row.minima[objective].bytes]));
      row.size_verdicts=Object.fromEntries(OBJECTIVES.map(objective=>[objective,
        sizeVerdict(row.artifacts.find(artifact=>artifact.objective===objective).sizes[objective],row.minima[objective].bytes)]));
    }
    for (const [path,identity] of files) assert.deepEqual(fileIdentity(path),identity,`input changed during benchmark: ${path}`);
    assert.equal(snapshotDependencyTree(join(toolchain,"node_modules"),{maxBytes:2*1024**3,maxEntries:100_000}).sha256,dependencies.sha256,"installed tool content changed");
    report.complete=true;
  } catch(error) {report.error=String(error.message);throw error;}
  finally {mkdirSync(dirname(reportPath),{recursive:true});writeFileSync(reportPath,JSON.stringify(report,null,2)+"\n");}
  return report;
}

if (process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const options={};
  for(let index=2;index<process.argv.length;index++) {
    const key=process.argv[index];
    if(key==="--write-lock") options.writeLock=true;
    else if(["--manifest","--lock","--work","--json","--timeout"].includes(key)) options[key.slice(2)]=process.argv[++index];
    else throw new Error(`unknown option ${key}`);
  }
  for(const name of ["manifest","lock","work","json"]) assert.ok(options[name],`--${name} required`);
  const report=runBars({manifestPath:resolve(options.manifest),lockPath:resolve(options.lock),work:resolve(options.work),reportPath:resolve(options.json),writeLock:options.writeLock,timeout:Number(options.timeout??120)});
  process.stdout.write(JSON.stringify({complete:report.complete,programs:report.programs.map(row=>({id:row.id,deltas:row.deltas,refused:row.refused.map(item=>item.id)}))})+"\n");
}
