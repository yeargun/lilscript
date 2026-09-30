#!/usr/bin/env node
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {fingerprint} from "../finer/tools/artifact-evidence.mjs";

// A carried result is tied to exact programs, engine, workload and limits. It
// cannot exempt an entire port, another artifact, or a newly changed contract.
export function runtimeIdentity(report,row,metric) {
  const workload=report.manifest.workloads.find(item=>item.id===row.id);
  return fingerprint({engine:report.engine.sha256,environment:report.environment,
    contract:row.contract,inputs:report.inputs,workload:report.inputs[row.workload ?? workload.workload],
    expected:workload.expected,counters:workload.counters,iterations:workload.iterations,warmup_runs:workload.warmup_runs,
    reference:report.inputs[row.implementations.reference.entry],candidate:report.inputs[row.implementations.candidate.entry],
    metric,tolerance:workload.runtime_limits[metric]});
}

export function checkRuntimeLedger(report,ledger) {
  assert.equal(report.complete,true,"incomplete runtime evidence");
  assert.equal(ledger.schema,1);
  assert.ok(Array.isArray(ledger.entries));
  const ids=new Set();
  for(const entry of ledger.entries) {
    for(const field of ["id","workload","metric","owner","reason","evidence","identity","disposition"])
      assert.ok(typeof entry[field]==="string" && entry[field].length,`runtime ledger needs ${field}`);
    assert.match(entry.identity,/^[a-f0-9]{64}$/);
    assert.ok(["regression","inconclusive"].includes(entry.disposition));
    assert.ok(!ids.has(entry.id),"duplicate runtime ledger id");ids.add(entry.id);
  }
  const used=new Set(),unaccounted=[];
  for(const row of report.workloads) {
    assert.ok(Object.keys(row.judgments??{}).length,`no declared limits for ${row.id}`);
    for(const [metric,judgment] of Object.entries(row.judgments)) {
      if(judgment.verdict==="within-declared-limit") continue;
      const identity=runtimeIdentity(report,row,metric);
      const matching=ledger.entries.find(entry=>entry.workload===row.id && entry.metric===metric &&
        entry.identity===identity && entry.disposition===judgment.verdict);
      if(matching) used.add(matching.id);
      else unaccounted.push({workload:row.id,metric,disposition:judgment.verdict,identity});
    }
  }
  const stale=ledger.entries.filter(entry=>!used.has(entry.id)).map(entry=>entry.id);
  return {passed:unaccounted.length===0 && stale.length===0,unaccounted,stale,
    carried:[...used],parity_established:used.size===0 && unaccounted.length===0};
}

if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const [reportPath,ledgerPath]=process.argv.slice(2);
  assert.ok(reportPath && ledgerPath,"usage: node scripts/runtime-ledger.mjs REPORT LEDGER");
  const result=checkRuntimeLedger(JSON.parse(readFileSync(reportPath,"utf8")),JSON.parse(readFileSync(ledgerPath,"utf8")));
  console.log(JSON.stringify(result));
  if(!result.passed) process.exitCode=1;
}
