import {test} from "node:test";
import assert from "node:assert/strict";
import {checkRuntimeLedger,runtimeIdentity} from "./runtime-ledger.mjs";
const report={complete:true,engine:{sha256:"engine"},environment:{TZ:"UTC"},inputs:{"/workload":"w","/reference":"a","/candidate":"b"},
  manifest:{workloads:[{id:"calls",workload:"/workload",iterations:10,warmup_runs:5,expected:42,counters:{calls:10},runtime_limits:{steady_cpu_us:.05}}]},
  workloads:[{id:"calls",contract:{exports:["run"]},implementations:{reference:{entry:"/reference"},candidate:{entry:"/candidate"}},judgments:{steady_cpu_us:{verdict:"inconclusive"}}}]};
test("runtime debts cannot cover another artifact, contract or disposition",()=>{
  const identity=runtimeIdentity(report,report.workloads[0],"steady_cpu_us");
  const ledger={schema:1,entries:[{id:"calls-noise",workload:"calls",metric:"steady_cpu_us",owner:"V1",reason:"controls overlap",evidence:"receipt.json",identity,disposition:"inconclusive"}]};
  const result=checkRuntimeLedger(report,ledger);assert.equal(result.passed,true);assert.equal(result.parity_established,false);
  for(const change of [value=>{value.inputs["/candidate"]="changed"},value=>{value.workloads[0].contract.exports.push("another")},value=>{value.workloads[0].judgments.steady_cpu_us.verdict="regression"}]) {
    const altered=structuredClone(report);change(altered);assert.equal(checkRuntimeLedger(altered,ledger).passed,false);
  }
  const improved=structuredClone(report);improved.workloads[0].judgments.steady_cpu_us.verdict="within-declared-limit";
  assert.deepEqual(checkRuntimeLedger(improved,ledger).stale,["calls-noise"]);
  assert.equal(checkRuntimeLedger(improved,{schema:1,entries:[]}).parity_established,true);
  assert.throws(()=>checkRuntimeLedger({...report,complete:false},ledger),/incomplete/);
  assert.throws(()=>checkRuntimeLedger(report,{schema:1,entries:[{...ledger.entries[0],owner:""}]}),/owner/);
});
