import assert from "node:assert/strict";
import {test} from "node:test";
import {execFileSync} from "node:child_process";
import {mkdtempSync,writeFileSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {fileURLToPath} from "node:url";
import {validateContract,minima,RECIPE_IDS} from "./bars.mjs";

const contract={boundary:"open-library",format:"esm",target:"es2020",exports:["twice"],externals:[],reflected_properties:[],
  semantics:{arithmetic:"signed int32",builtins:"standard"},equivalence:"both functions return the doubled signed integer"};
test("boundary contracts require explicit semantics and a declared surface",()=>{
  validateContract(contract);
  for(const key of ["boundary","exports","externals","reflected_properties","semantics","equivalence"]) {
    const invalid={...contract};delete invalid[key];assert.throws(()=>validateContract(invalid));
  }
  assert.throws(()=>validateContract({...contract,exports:["twice","twice"]}));
  assert.ok(RECIPE_IDS.includes("oxc") && RECIPE_IDS.includes("rolldown") && RECIPE_IDS.includes("closure-advanced"));
});
test("only common-oracle passing competitors set each independent bar",()=>{
  const row=(id,raw,gzip,brotli,eligible=true,kind="competitor")=>({id,eligible,kind,sizes:{raw,gzip,brotli},artifact:{sha256:id}});
  const winners=minima([row("raw",1,8,9),row("gzip",8,1,8),row("brotli",9,9,1),row("broken",0,0,0,false),row("ours",0,0,0,true,"candidate")]);
  assert.deepEqual(Object.fromEntries(Object.entries(winners).map(([key,value])=>[key,value.id])),{raw:"raw",gzip:"gzip",brotli:"brotli"});
  assert.deepEqual(minima([row("broken",0,0,0,false)]),{raw:null,gzip:null,brotli:null});
});
test("the same fresh-process observer refuses a wrong candidate",()=>{
  const root=mkdtempSync(join(tmpdir(),"lilscript-bars-"));
  try {
    const oracle=join(root,"oracle.mjs"),artifact=join(root,"artifact.mjs"),request=join(root,"request.json");
    writeFileSync(oracle,"export function observe(api){return [api.twice(3),Object.keys(api)]}");
    writeFileSync(request,JSON.stringify({oracle,artifact,format:"esm",expected:[6,["twice"]]}));
    writeFileSync(artifact,"export const twice = n => n * 2");
    const worker=fileURLToPath(new URL("./bars-worker.mjs",import.meta.url));
    const result=JSON.parse(execFileSync(process.execPath,[worker,request],{encoding:"utf8"}));
    assert.equal(result.passed,true);
    writeFileSync(artifact,"export const twice = n => n * 3");
    assert.throws(()=>execFileSync(process.execPath,[worker,request],{stdio:"pipe"}),/Command failed/);
  } finally {rmSync(root,{recursive:true,force:true});}
});
