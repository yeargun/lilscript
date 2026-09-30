import assert from "node:assert/strict";
import {test} from "node:test";
import {spawnSync} from "node:child_process";
import {mkdtempSync,writeFileSync,readFileSync,existsSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join,resolve} from "node:path";

const binary=resolve(process.env.LILSCRIPT_ORACLE??"target/release/lilscript-oracle");
assert.ok(existsSync(binary),"build lilscript-oracle or set LILSCRIPT_ORACLE");
test("independent oracle generation is atomic with respect to existing disagreement",()=>{
  const root=mkdtempSync(join(tmpdir(),"lilscript-oracle-"));
  try {
    const source=join(root,"case.lil"),missing=join(root,"missing.lil"),report=join(root,"bad.json");
    writeFileSync(source,"print(2 + 3);\n");writeFileSync(join(root,"case.out"),"wrong\n");
    writeFileSync(missing,"print(9);\n");
    const result=spawnSync(binary,[source,missing,"--write-missing","--json",report],{encoding:"utf8"});
    assert.notEqual(result.status,0);
    assert.equal(readFileSync(join(root,"case.out"),"utf8"),"wrong\n");
    assert.equal(existsSync(join(root,"missing.out")),false);
    const receipt=JSON.parse(readFileSync(report,"utf8"));assert.equal(receipt.complete,false);
    assert.equal(receipt.cases.find(row=>row.case==="case").independent_output,"5\n");
    const fresh=spawnSync(binary,[missing,"--write-missing","--json",join(root,"good.json")],{encoding:"utf8"});
    assert.equal(fresh.status,0,fresh.stderr);assert.equal(readFileSync(join(root,"missing.out"),"utf8"),"9\n");
  } finally {rmSync(root,{recursive:true,force:true});}
});
test("host-dependent cases are explicit coverage gaps and cannot replace prior coverage",()=>{
  const root=mkdtempSync(join(tmpdir(),"lilscript-oracle-host-"));
  try {
    const source=join(root,"case.lil"),baseline=join(root,"baseline.json"),report=join(root,"host.json");
    writeFileSync(source,"print(5);\n");writeFileSync(join(root,"case.out"),"5\n");
    const first=spawnSync(binary,[source,"--json",baseline],{encoding:"utf8"});assert.equal(first.status,0,first.stderr);
    writeFileSync(join(root,"case.host.js"),"globalThis.hostObservation = 1;\n");
    const second=spawnSync(binary,[source,"--json",report,"--baseline",baseline],{encoding:"utf8"});
    assert.notEqual(second.status,0);
    const receipt=JSON.parse(readFileSync(report,"utf8"));assert.deepEqual(receipt.lost_coverage,["case"]);
    assert.equal(receipt.cases[0].status,"uncovered");assert.match(receipt.cases[0].reason,/host-model/);
    assert.equal(receipt.cases[0].host_inputs.length,1);
  } finally {rmSync(root,{recursive:true,force:true});}
});
